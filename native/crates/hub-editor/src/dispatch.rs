//! The messages the shell claims before RichEdit sees them.
//!
//! Split out of `lib.rs` when that file hit the 800-line hard cap, but the boundary
//! is real: `lib.rs` manages the control (create, theme, sizes, text), while this
//! module is the **message path** — what the island swallows so that the shell can
//! behave like the shipped note instead of like a text box.
//!
//! Three messages are claimed, each for a different reason:
//!
//! | Message | Default behaviour | Why the shell takes it |
//! |---|---|---|
//! | `WM_KEYDOWN` `VK_ESCAPE` | beeps, does nothing | Esc closes the note (Q12) |
//! | `WM_KEYDOWN` `VK_TAB` | moves focus out of the island | a note is a scratch pad; Tab must insert a tab |
//! | `WM_KEYDOWN` other | RichEdit's own editing | the shortcut table decides (Ctrl+D/Z/T/Shift+H) |
//! | `WM_MOUSEWHEEL` +Ctrl | **zooms the text** | `Ctrl+wheel` is window opacity (Q28) |
//!
//! Anything not claimed goes to `DefSubclassProc`, which is the point: reimplementing
//! editing would trade the platform's caret, selection and IME behaviour for nothing.

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_TAB};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageW, WM_CHAR, WM_KEYDOWN, WM_MOUSEWHEEL,
};

use crate::keys;

/// `LOWORD(wParam)` bit of `WM_MOUSEWHEEL` meaning "Ctrl was held".
///
/// windows-sys exports `MK_*` under `Win32_System_SystemServices`, and pulling that
/// feature in for one bit is not worth it. The value is `MK_CONTROL` from
/// `winuser.h` and does not change.
const MK_CONTROL: u32 = 0x0008;

/// The callbacks the shell installs, owned by [`crate::Editor`].
pub(crate) struct Callbacks {
    pub(crate) on_escape: Option<Box<dyn Fn() + Send + Sync>>,
    /// Returns `true` when the shell consumed the key. A `false` falls through to
    /// `DefSubclassProc`, which is what keeps `Ctrl+Z` inside RichEdit's own undo
    /// instead of reimplementing it one level up.
    pub(crate) on_key: Option<Box<dyn Fn(keys::KeyPress) -> bool + Send + Sync>>,
    /// `Ctrl+wheel`, in wheel delta units (positive = away from the user). Returns
    /// `true` when the shell consumed the notch.
    ///
    /// The shell needs this on this side of the boundary because RichEdit owns the
    /// wheel: `Ctrl+wheel` is its text **zoom** gesture, so unless the message is
    /// intercepted here, scrolling the opacity would also resize the body text.
    pub(crate) on_wheel: Option<Box<dyn Fn(i32) -> bool + Send + Sync>>,
}

/// Handle one message. `Some` means "claimed: do not pass it on".
///
/// # Safety
///
/// `callbacks` must point at the box installed with `SetWindowSubclass` and stay
/// alive for the call, which is what `Editor` guarantees for the life of the
/// control.
pub(crate) unsafe fn claim(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    callbacks: &Callbacks,
) -> Option<LRESULT> {
    if message == WM_KEYDOWN {
        match wparam as u16 {
            // Esc: RichEdit does nothing with it but would beep; the shell needs
            // to see it so the panel closes instead of the editor swallowing it.
            VK_ESCAPE => {
                if let Some(callback) = callbacks.on_escape.as_ref() {
                    callback();
                }
                return Some(0);
            }
            // Tab: insert a tab instead of moving focus out of the panel.
            VK_TAB => {
                unsafe { SendMessageW(hwnd, WM_CHAR, 0x09, 1) };
                return Some(0);
            }
            // Everything else goes through the shell's shortcut table. Esc stays
            // above it because closing is not a table entry: it must work even if
            // the table is empty or the callback was never installed.
            other => {
                if let Some(callback) = callbacks.on_key.as_ref() {
                    let press = keys::modifiers(keys::original_key(hwnd, other, lparam));
                    if callback(press) {
                        return Some(0);
                    }
                }
            }
        }
        return None;
    }

    // `Ctrl+wheel` is RichEdit's zoom gesture, and the shell wants it for window
    // opacity. Claimed before `DefSubclassProc` so one wheel notch cannot do both:
    // the shipped note adjusts *only* the opacity, and a body that also grows a
    // point per notch is a different product.
    if message == WM_MOUSEWHEEL {
        let keys_down = (wparam as usize & 0xFFFF) as u32;
        if keys_down & MK_CONTROL != 0 {
            if let Some(callback) = callbacks.on_wheel.as_ref() {
                // HIWORD is the signed delta, in multiples of WHEEL_DELTA (120).
                let delta = ((wparam as usize >> 16) & 0xFFFF) as u16 as i16 as i32;
                if callback(delta) {
                    return Some(0);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ctrl_wheel_is_claimed_and_the_delta_is_signed() {
        // No window is created here: the wheel branch never touches `hwnd` unless a
        // callback is installed, so a `null` handle is enough to pin the arithmetic.
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<i32>::new()));
        let sink = seen.clone();
        let callbacks = Callbacks {
            on_escape: None,
            on_key: None,
            on_wheel: Some(Box::new(move |delta| {
                sink.lock().unwrap().push(delta);
                true
            })),
        };

        let hwnd = std::ptr::null_mut();
        // HIWORD = 120 (one notch up) with MK_CONTROL in LOWORD.
        let up: usize = 120usize << 16 | MK_CONTROL as usize;
        let claimed = unsafe { claim(hwnd, WM_MOUSEWHEEL, up, 0, &callbacks) };
        assert_eq!(claimed, Some(0), "Ctrl+wheel must not reach RichEdit");
        // HIWORD = -120, as two's complement in the upper word.
        let down: usize = 0xFF88usize << 16 | MK_CONTROL as usize;
        let claimed = unsafe { claim(hwnd, WM_MOUSEWHEEL, down, 0, &callbacks) };
        assert_eq!(claimed, Some(0));

        assert_eq!(*seen.lock().unwrap(), vec![120, -120]);

        // Without Ctrl the wheel is a scroll, not an opacity change.
        let plain: usize = 120usize << 16;
        let claimed = unsafe { claim(hwnd, WM_MOUSEWHEEL, plain, 0, &callbacks) };
        assert_eq!(claimed, None);
        assert_eq!(seen.lock().unwrap().len(), 2, "the plain wheel was not reported");
    }
}
