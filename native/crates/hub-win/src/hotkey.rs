//! `RegisterHotKey` on a dedicated thread.
//!
//! Three decisions here matter for latency, and the first two were paid for in the
//! Tauri build:
//!
//! 1. **The hotkey owns its own message loop.** A hotkey registered with a null
//!    window handle is delivered to the registering *thread's* queue. Running it on
//!    the UI thread means every reveal competes with painting, and any blocking
//!    work in the callback (the old build captured the selection inline and could
//!    block on a clipboard owned by a screenshot tool) freezes the window. Here the
//!    thread runs the caller's closure — which only reads the cursor, hits the
//!    cached monitor layout and posts a message — then returns to `GetMessageW`.
//! 2. **Debounce happens before the closure runs.** QQ/WeChat install low-level
//!    keyboard hooks for the same combinations, so one physical press can arrive as
//!    two or three messages; without this the panel opens and closes immediately.
//! 3. **`MOD_NOREPEAT`.** Holding the key for 300 ms must not toggle a dozen times.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_QUIT,
};

/// Arbitrary but stable id; `RegisterHotKey` only needs it unique per thread.
const HOTKEY_ID: i32 = 0xE1;

const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const MOD_NOREPEAT: u32 = 0x4000;

pub type PressHandler = Arc<dyn Fn() + Send + Sync>;

/// A registered global hotkey and the thread serving it.
pub struct Hotkey {
    thread_id: u32,
}

impl Hotkey {
    /// Parse `combo`, register it and start pumping messages.
    ///
    /// `on_press` runs on the hotkey thread, immediately after the debounce check.
    /// Returns the `RegisterHotKey` failure when the combination is already owned by
    /// another process — Windows awards these first come, first served, so the
    /// caller is expected to surface it rather than retry silently.
    pub fn spawn(combo: &str, debounce: Duration, on_press: PressHandler) -> Result<Self, String> {
        let (modifiers, vk) = parse_combo(combo)?;
        let label = combo.to_string();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32, String>>();

        thread::Builder::new()
            .name("hub-hotkey".to_string())
            .spawn(move || {
                let flags = modifiers | MOD_NOREPEAT;
                let registered =
                    unsafe { RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, flags, vk) };
                if registered == 0 {
                    let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                    let _ = ready_tx.send(Err(format!(
                        "{label}: RegisterHotKey failed (win32 error {code}); another program already owns it"
                    )));
                    return;
                }

                let thread_id = unsafe { GetCurrentThreadId() };
                let _ = ready_tx.send(Ok(thread_id));

                let mut last_press: Option<Instant> = None;
                let mut msg = MSG::default();
                loop {
                    let got = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
                    if got <= 0 || msg.message == WM_QUIT {
                        break;
                    }
                    if msg.message != WM_HOTKEY || msg.wParam as i32 != HOTKEY_ID {
                        continue;
                    }

                    let now = Instant::now();
                    if let Some(previous) = last_press {
                        if now.duration_since(previous) < debounce {
                            continue;
                        }
                    }
                    last_press = Some(now);
                    on_press();
                }

                unsafe {
                    UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
                }
            })
            .map_err(|error| format!("failed to spawn hotkey thread: {error}"))?;

        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self { thread_id }),
            Ok(Err(message)) => Err(message),
            Err(_) => Err("hotkey thread died before registering".to_string()),
        }
    }

    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    /// Post the same `WM_HOTKEY` the OS would, so a 200-cycle run needs no hands.
    ///
    /// This is what makes the measurement repeatable: it walks the real path
    /// (debounce → closure → queue → reveal) and only skips driver and dispatch,
    /// which no implementation can influence.
    pub fn post_test_press(&self) -> bool {
        let posted = unsafe { PostThreadMessageW(self.thread_id, WM_HOTKEY, HOTKEY_ID as usize, 0) };
        posted != 0
    }

    /// Post `count` presses `gap` apart, each a separate `WM_HOTKEY`.
    ///
    /// Used to prove the debounce: a screenshot tool echoing the same combination
    /// produces exactly this pattern, and it must yield one reveal, not `count`.
    pub fn post_presses(self: &std::sync::Arc<Self>, count: usize, gap: Duration) {
        let thread_id = self.thread_id;
        thread::spawn(move || {
            for index in 0..count {
                if index > 0 {
                    thread::sleep(gap);
                }
                unsafe {
                    PostThreadMessageW(thread_id, WM_HOTKEY, HOTKEY_ID as usize, 0);
                }
            }
        });
    }

    /// Ask the message loop to exit (used when the spike shuts down on its own).
    pub fn shutdown(&self) {
        unsafe {
            PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
        }
    }
}

/// `"Ctrl+Shift+Space"` → `(MOD_CONTROL | MOD_SHIFT, VK_SPACE)`.
fn parse_combo(combo: &str) -> Result<(u32, u32), String> {
    let mut modifiers = 0u32;
    let mut key: Option<u32> = None;

    for part in combo.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "shift" => modifiers |= MOD_SHIFT,
            "alt" => modifiers |= MOD_ALT,
            "win" | "super" => modifiers |= MOD_WIN,
            other => {
                if key.is_some() {
                    return Err(format!("{combo}: more than one non-modifier key"));
                }
                key = Some(
                    virtual_key(other)
                        .ok_or_else(|| format!("{combo}: unsupported key '{other}'"))?,
                );
            }
        }
    }

    let key = key.ok_or_else(|| format!("{combo}: no non-modifier key"))?;
    Ok((modifiers, key))
}

fn virtual_key(name: &str) -> Option<u32> {
    let upper = name.to_ascii_uppercase();
    if upper.len() == 1 {
        let byte = upper.as_bytes()[0];
        if byte.is_ascii_alphanumeric() {
            return Some(byte as u32);
        }
    }
    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(number) = rest.parse::<u32>() {
            if (1..=24).contains(&number) {
                return Some(0x70 + number - 1);
            }
        }
    }
    match upper.as_str() {
        "SPACE" => Some(0x20),
        "ENTER" | "RETURN" => Some(0x0D),
        "TAB" => Some(0x09),
        "ESC" | "ESCAPE" => Some(0x1B),
        "BACKSPACE" => Some(0x08),
        "GRAVE" | "`" => Some(0xC0),
        "SEMICOLON" => Some(0xBA),
        "QUOTE" => Some(0xDE),
        "COMMA" => Some(0xBC),
        "PERIOD" => Some(0xBE),
        "SLASH" => Some(0xBF),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_documented_combination() {
        assert_eq!(
            parse_combo("Ctrl+Shift+Space").unwrap(),
            (MOD_CONTROL | MOD_SHIFT, 0x20)
        );
    }

    #[test]
    fn parses_function_and_letter_keys() {
        assert_eq!(
            parse_combo("Ctrl+Alt+K").unwrap(),
            (MOD_CONTROL | MOD_ALT, b'K' as u32)
        );
        assert_eq!(
            parse_combo("Ctrl+Shift+F10").unwrap(),
            (MOD_CONTROL | MOD_SHIFT, 0x79)
        );
    }

    #[test]
    fn rejects_a_combination_without_a_key() {
        assert!(parse_combo("Ctrl+Shift").is_err());
    }

    #[test]
    fn rejects_two_non_modifier_keys() {
        assert!(parse_combo("Ctrl+A+B").is_err());
    }

    #[test]
    fn rejects_unknown_key_names() {
        assert!(parse_combo("Ctrl+Nope").is_err());
    }
}
