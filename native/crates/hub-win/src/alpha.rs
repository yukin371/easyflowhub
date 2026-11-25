//! Whole-window opacity (parity Q28/Q29).
//!
//! The shipped note fades with CSS `opacity` on its root element: title strip, body
//! text and caret all become translucent together, and the desktop shows through.
//! `SetLayeredWindowAttributes(…, LWA_ALPHA)` is the win32 answer to the same
//! question — DWM blends the window's redirection surface as **one bitmap**, so the
//! RichEdit child fades with its parent without the control knowing anything about
//! opacity.
//!
//! ## Why the alternatives were rejected
//!
//! | Candidate | Why not |
//! |---|---|
//! | `UpdateLayeredWindow` (per-pixel alpha) | The caller then owns every frame's bits, which on a GL window means reading the swap chain back and re-uploading it; it also disables the child window's own painting |
//! | Alpha on the island alone | The island covers the body but not the strip, so the note would fade in two pieces — visible as a hard seam under the title bar |
//! | An egui veil drawn over everything | A child window paints over its parent, so the veil would sit *under* the text it is supposed to fade |
//!
//! ## Hit testing: why the floor is 1, not 0
//!
//! Partial alpha does not change hit testing — a 40% note is clicked exactly like an
//! opaque one. **Zero does**: at `LWA_ALPHA = 0` the window is click-through, and
//! `WindowFromPoint` over the note answers whatever is behind it (measured
//! 2026-11-25; at 1 it answers the note again). The shipped note never loses pointer
//! events, because CSS `opacity: 0` still receives them — that is how a user scrolls
//! a ghost back into view.
//!
//! So [`GHOST_ALPHA`] is 1: visually gone, still reachable. The alternative designs
//! were worse. Click-through plus a global "un-ghost" hotkey adds a key the shipped
//! build does not have; refusing to reach 0% breaks Q28 outright; leaving the note
//! unreachable turns ghost mode into a way to lose the text (notes are not persisted
//! yet in this batch).

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetLayeredWindowAttributes, GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW,
    SetWindowPos, GWL_EXSTYLE, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_NOZORDER, WS_EX_LAYERED,
};

/// The lowest alpha a note is allowed to use, which is **not** 0.
///
/// Measured 2026-11-25 on Win11 26300: with `LWA_ALPHA = 0` the whole window is
/// click-through — `WindowFromPoint` over the note answers the window *behind* it —
/// and it becomes hit-testable again at 1. That turns GHOST into a one-way door:
/// the note cannot be clicked, so it cannot take focus, so `Ctrl+Shift+H` (an
/// island shortcut) cannot reach it, and the text inside is only reachable by
/// closing every note.
///
/// The shipped note gets hit-testing at `opacity: 0` for free from CSS: a
/// zero-opacity element still receives pointer events, which is how a user scrolls
/// a ghost back up. One byte out of 255 is that same affordance here: the composite
/// differs from the desktop by 0.4% of one channel, which no screenshot in this
/// repo can resolve, while the window stays reachable.
pub const GHOST_ALPHA: u8 = 1;

/// Opacity `0..=1` as the byte `SetLayeredWindowAttributes` wants.
///
/// Rounding, not truncation: `0.7 * 255` is 178.5, and dropping the half would make
/// a 20% step land at 50 instead of 51 — the label would say `20%` while the window
/// is at 19.6%. Non-finite input falls back to fully opaque: a note nobody can see
/// is worse than one at the wrong alpha.
///
/// The floor at the bottom is [`GHOST_ALPHA`], not 0 — see its comment. Visually
/// 0% still means gone; the window is there so the user can get it back.
pub fn from_opacity(opacity: f32) -> u8 {
    if !opacity.is_finite() {
        return 255;
    }
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    alpha.max(GHOST_ALPHA)
}

/// The inverse, for logs and for reading a state back out of the OS.
pub fn to_opacity(alpha: u8) -> f32 {
    alpha as f32 / 255.0
}

/// Apply `alpha` to the whole window, making it layered first if it is not.
///
/// Returns whether the OS accepted the attribute. The `WS_EX_LAYERED` bit is added
/// by `SetWindowLongPtrW` and only takes effect after the `SWP_FRAMECHANGED` pass —
/// without it the window is left *hidden* until the next attribute call, which
/// reads as "the note vanished while the log said ok". Setting the bit is idempotent,
/// so an unchanged value still costs two cheap calls.
pub fn set(hwnd: isize, alpha: u8) -> bool {
    let hwnd = hwnd as HWND;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let want = style | WS_EX_LAYERED;
        if style != want {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want as isize);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA) != 0
    }
}

/// Read back what the compositor is actually using.
///
/// `None` means "not layered" (or the window is gone). This exists so the log can
/// record the value the OS kept rather than the value we asked for: a silent
/// downgrade would otherwise look identical to a successful call.
pub fn get(hwnd: isize) -> Option<u8> {
    let mut alpha: u8 = 255;
    let mut key = 0u32;
    let mut flags = 0u32;
    let ok = unsafe {
        GetLayeredWindowAttributes(hwnd as HWND, &mut key, &mut alpha, &mut flags)
    };
    if ok == 0 {
        None
    } else {
        Some(alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_percent_steps_land_on_exact_bytes() {
        // 0.2 * 255 = 51 exactly; the point of this test is the 0.4 and 0.6 steps,
        // which are 102.00000000000001 and 153.0 in binary.
        assert_eq!(from_opacity(0.2), 51);
        assert_eq!(from_opacity(0.4), 102);
        assert_eq!(from_opacity(0.6), 153);
        assert_eq!(from_opacity(0.8), 204);
        assert_eq!(from_opacity(1.0), 255);
    }

    #[test]
    fn ghost_keeps_one_byte_so_the_window_can_be_clicked_back() {
        // 0 would be click-through (measured), which makes GHOST unrecoverable with
        // the mouse. 1 is invisible (0.4% of one channel) and still hit-testable.
        assert_eq!(from_opacity(0.0), GHOST_ALPHA);
        assert_eq!(GHOST_ALPHA, 1);
        // The strip label must still read GHOST at that byte: `opacity_label` cuts
        // off below 0.5%, and 1/255 is 0.39%.
        assert_eq!(to_opacity(GHOST_ALPHA) <= 0.005, true);
    }

    #[test]
    fn out_of_range_and_nan_fall_back_to_visible() {
        assert_eq!(from_opacity(2.5), 255, "clamping up must stay at opaque");
        assert_eq!(
            from_opacity(-1.0),
            GHOST_ALPHA,
            "clamping down must reach ghost, not below it"
        );
        assert_eq!(from_opacity(f32::NAN), 255);
        assert_eq!(from_opacity(f32::INFINITY), 255);
    }

    #[test]
    fn a_label_step_survives_the_round_trip() {
        // The strip prints `(opacity * 100).round()`, and the window uses the byte.
        // Five steps down must read 0% at the same time the window reaches 0.
        for step in 0..=5 {
            let opacity = 1.0 - step as f32 * 0.2;
            let byte = from_opacity(opacity);
            assert_eq!(
                (to_opacity(byte) * 100.0).round() as i32,
                (opacity * 100.0).round() as i32,
                "step {step} disagrees between label and compositor"
            );
        }
    }
}
