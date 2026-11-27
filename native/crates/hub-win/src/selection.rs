//! Ask the focused application to copy its selection, then put the clipboard back.
//!
//! ## Why a keystroke and not a query
//!
//! Windows has no "give me the selected text" call. UI Automation's
//! `TextPattern.GetSelection` exists, but it only answers for applications that
//! implement it — which is not the set of applications people select text in — and
//! it blocks on the provider's thread. Every shipped clipboard translator that has
//! to work *everywhere* does what a person would: `Ctrl+C`, read, paste back.
//!
//! ## The three things that make it safe
//!
//! 1. **It happens before the panel takes focus.** `SendInput` goes to whatever
//!    owns the foreground, and the panel is shown before it is focused precisely so
//!    that this window exists. Focus first and the copy would ask the panel for its
//!    selection, which is nothing.
//! 2. **The clipboard is restored from a snapshot, not from memory.** What was
//!    there is read before the copy and written back after; [`Prior::Other`] is
//!    reported instead of being pretended away (see `clipboard`).
//! 3. **Every wait is bounded.** The budget covers the whole round trip, and the
//!    caller runs this between showing the window and focusing it, so an unbounded
//!    wait is a panel that appears and ignores the keyboard.
//!
//! ## Why polling rather than a fixed sleep
//!
//! The Tauri build slept 45 ms and then read the clipboard. That is both too long
//! (most applications answer in 5–15 ms, and the sleep is on the reveal path) and
//! too short (a heavy application under load can miss it). Watching the clipboard's
//! sequence number answers *when* instead of guessing: it returns as soon as the
//! copy lands, and the budget is what it costs when nothing was selected.

use std::mem::size_of;
use std::time::{Duration, Instant};

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_C,
    VK_CONTROL, VK_MENU, VK_SHIFT, VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow, SetForegroundWindow};

use crate::clipboard::{self, Prior};

/// How long the copy is given to land.
///
/// This is on the reveal path: the panel is on screen but does not answer the
/// keyboard until the capture returns, so the number is a user-visible cost paid on
/// every summon that has no selection -- which is most of them. The shipped Tauri
/// build slept 45 ms for the same job, and that is the bar here rather than a
/// number that feels generous: 45 ms of a 60 ms budget is the most this feature is
/// allowed to cost. Early exit is what makes the *positive* case cheap; nothing
/// makes the negative case cheaper, because "no copy arrived" can only be learned
/// by waiting.
pub const CAPTURE_BUDGET: Duration = Duration::from_millis(45);

/// How long to wait after `SetForegroundWindow` before pasting.
///
/// A target application that has just been activated has not necessarily finished
/// processing the activation, and a `Ctrl+V` that arrives first goes nowhere. The
/// Tauri build used 60 ms; it is paid only on the replace path, never on reveal.
const PASTE_SETTLE: Duration = Duration::from_millis(60);

/// How long the hotkey's own modifiers are given to be released.
///
/// `WM_HOTKEY` is delivered while the combination is physically held, and Windows
/// posts synthetic key-ups for those modifiers to the foreground window so it does
/// not see them stuck. A `Ctrl+C` injected in that same instant can be ordered
/// *behind* those key-ups, and the target then receives a bare `c`.
///
/// That is not a missed copy. It is a character typed into whatever had the focus,
/// over the selection, which is how a note came back holding one letter and the log
/// said `reason=no_copy` -- a text insertion moves no clipboard sequence number, so
/// the two symptoms never look related. The wait is short because the keys are
/// usually released a few milliseconds after the press; 12 ms is spent out of the
/// same budget the copy uses, not on top of it.
const MODIFIER_RELEASE: Duration = Duration::from_millis(12);

/// What a capture attempt produced.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Text, the window it came from, and what the round trip cost.
    Captured(Capture),
    /// Nothing usable, with the reason a log line can carry.
    Miss {
        reason: &'static str,
        elapsed: Duration,
        /// How much of the budget went on waiting for the hotkey's modifiers. Zero
        /// on the paths that never get that far.
        held: Duration,
    },
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub text: String,
    /// The window that had the focus when the copy was sent. The only thing that
    /// can receive a replacement later.
    pub target: isize,
    /// What the clipboard held before: `text`, `other`, `empty`.
    pub prior: Prior,
    /// Whether the clipboard is back to `prior` afterwards. `false` is a fact to
    /// log, not an error to hide.
    /// Whether the clipboard is back to `prior` afterwards. `false` is a fact to
    /// log, not an error to hide.
    pub restored: bool,
    /// The chord that opened the panel is normally still held, and injecting the
    /// copy behind its synthetic key-ups types a bare letter into the target (see
    /// [`MODIFIER_RELEASE`]). This is the one thing that can go *wrong* silently
    /// rather than merely fail, so it is waited out.
    pub held: Duration,
    pub elapsed: Duration,
}

impl Outcome {
    /// One line for the log, whichever way it went.
    pub fn describe(&self) -> String {
        match self {
            Self::Captured(capture) => format!(
                "chars={} ms={:.1} prior={} restored={} held_ms={}",
                capture.text.chars().count(),
                capture.elapsed.as_secs_f64() * 1000.0,
                capture.prior.describe(),
                capture.restored,
                capture.held.as_secs_f64() * 1000.0
            ),
            Self::Miss {
                reason,
                elapsed,
                held,
            } => format!(
                "chars=0 reason={reason} ms={:.1} held_ms={:.1}",
                elapsed.as_secs_f64() * 1000.0,
                held.as_secs_f64() * 1000.0
            ),
        }
    }
}

/// Copy the focused application's selection, restoring the clipboard afterwards.
///
/// `panel` is this process's own window: if it is somehow the foreground — a
/// second press while the panel is up — asking it to copy would be a round trip
/// that can only fail, and the log says which case it was rather than "no
/// selection".
pub fn capture(panel: isize) -> Outcome {
    let at = Instant::now();
    let zero = Duration::ZERO;

    let target = unsafe { GetForegroundWindow() } as isize;
    if target == 0 {
        return Outcome::Miss {
            reason: "no_foreground",
            elapsed: at.elapsed(),
            held: zero,
        };
    }
    if target == panel {
        return Outcome::Miss {
            reason: "self",
            elapsed: at.elapsed(),
            held: zero,
        };
    }

    let before = clipboard::sequence();
    let prior = clipboard::snapshot();
    let held = wait_for_modifiers();
    let miss = |reason: &'static str| Outcome::Miss {
        reason,
        elapsed: at.elapsed(),
        held,
    };
    if !send_ctrl(VK_C) {
        // Nothing was copied, so the clipboard is untouched: restoring here would
        // be writing to a clipboard we never changed.
        return miss("send_failed");
    }

    let mut landed = false;
    while at.elapsed() < CAPTURE_BUDGET {
        if clipboard::sequence() != before {
            landed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if !landed {
        // Deliberately no restore: the sequence never moved, so the clipboard is
        // still the user's. Clearing it here is what the Tauri build did when the
        // previous clipboard held no text, and it destroys a screenshot someone
        // took between the key press and this line.
        return miss("no_copy");
    }

    let text = clipboard::text();
    let restored = clipboard::restore(&prior);
    match text {
        Some(text) => Outcome::Captured(Capture {
            text,
            target,
            prior,
            restored,
            held,
            elapsed: at.elapsed(),
        }),
        // A copy that produced no text: an image, a file. The clipboard has been
        // restored, so the honest answer is "nothing to translate".
        None => miss("no_text"),
    }
}

/// Wait, briefly, for the hotkey's modifiers to be released.
///
/// Returns how long that took, which is 0 when the user already lifted their
/// fingers. Bounded by [`MODIFIER_RELEASE`] and deliberately not extended when the
/// keys stay down: a user who holds `Ctrl` while summoning the panel still gets a
/// capture attempt, and the log records that the wait ran out.
fn wait_for_modifiers() -> Duration {
    let at = Instant::now();
    while at.elapsed() < MODIFIER_RELEASE {
        let down = [VK_CONTROL, VK_MENU, VK_SHIFT]
            .iter()
            .any(|vk| unsafe { GetAsyncKeyState(*vk as i32) } as u16 & 0x8000 != 0);
        if !down {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    at.elapsed()
}

/// Write `text` over the selection in `target`, as if the user pasted it.
///
/// Explicit by design: nothing calls this on its own. The result surface offers
/// it, because writing into somebody else's document is the one action here that
/// cannot be undone by pressing `Esc`.
///
/// # This must not run on a thread that pumps `target`'s messages
///
/// The paste is a keystroke, so the target processes it on its own message loop
/// *after* this function sends it. The clipboard is put back 60 ms later, and the
/// two only work out if the target has had a chance to run in between. In this
/// shell the note windows share the panel's thread, so calling this from the UI
/// thread waited -- with the loop blocked -- until the clipboard had already been
/// restored, and the note pasted the *previous* clipboard contents. The symptom was
/// a replace that reported success and put the wrong text in the document.
pub fn replace(target: isize, text: &str) -> Result<(), String> {
    if target == 0 || unsafe { IsWindow(target as *mut std::ffi::c_void) } == 0 {
        return Err("原始选区窗口已关闭，请重新选中文本后重试。".to_string());
    }
    let prior = clipboard::snapshot();
    // Same hazard as the capture, with a worse outcome: a `v` injected while a
    // modifier from somewhere else is still down would be *typed* into the document
    // instead of pasting, on top of the text this was supposed to replace.
    let _ = wait_for_modifiers();
    if !clipboard::set_text(text) {
        return Err("无法写入剪贴板，替换选区失败。".to_string());
    }
    if unsafe { SetForegroundWindow(target as *mut std::ffi::c_void) } == 0 {
        let _ = clipboard::restore(&prior);
        return Err("无法恢复原始窗口焦点，替换选区失败。".to_string());
    }
    std::thread::sleep(PASTE_SETTLE);
    let sent = send_ctrl(VK_V);
    std::thread::sleep(PASTE_SETTLE);
    // Restore after the paste, not before: the target reads the clipboard when it
    // processes the keystroke, which is a message away, not an instant.
    let _ = clipboard::restore(&prior);
    if sent {
        Ok(())
    } else {
        Err("系统未接受粘贴输入，替换选区失败。".to_string())
    }
}

/// Press and release `key` with `Ctrl` held.
fn send_ctrl(key: u16) -> bool {
    let press = |code: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: code,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [
        press(VK_CONTROL, false),
        press(key, false),
        press(key, true),
        press(VK_CONTROL, true),
    ];
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    sent == inputs.len() as u32
}
