//! Whole-window opacity (parity Q28/Q29), including the ghost blend.
//!
//! The shipped note fades with CSS `opacity` on its root element: title strip, body
//! text and caret all become translucent together, and the desktop shows through.
//! `SetLayeredWindowAttributes(…, LWA_ALPHA)` is the win32 answer to the same
//! question — DWM blends the window's redirection surface as **one bitmap**, so the
//! RichEdit child fades with its parent without the control knowing anything about
//! opacity.
//!
//! ## Two blends, not one number
//!
//! `LWA_ALPHA` multiplies *every* pixel, background included, so a "20% note" is a
//! 20% panel with 20% text — the desktop shows through both. At 0% the note is gone,
//! and a note nobody can see is a note nobody can find again. Ghost mode therefore
//! uses a second blend:
//!
//! ```text
//! background (== key colour)  →  alpha 0, and click-through
//! everything else (text, strip, glyphs) → alpha 51 (20%)
//! ```
//!
//! That is `LWA_COLORKEY | LWA_ALPHA`: pixels matching the key vanish, the rest are
//! blended by the alpha byte. Measured 2026-11-25 with a layered parent and a
//! `RICHEDIT50W` child: the child's own background is keyed out too (its pixels
//! equal the desktop **exactly**), a non-key pixel lands on
//! `0.2 * colour + 0.8 * desktop`, and `WindowFromPoint` answers the note on the
//! non-key pixel / the window behind on a keyed one.
//!
//! The key is the note's *panel* colour, which is already the body background on
//! both sides of the window boundary: egui paints it and `EM_SETBKGNDCOLOR` fills
//! the RichEdit with it. So ghost mode needs no repaint in a different colour — the
//! only thing that changes is which pixels the compositor throws away.
//!
//! ## Why the ghost keeps 20%
//!
//! At 0% the window is click-through *as a whole* (`WindowFromPoint` over the note
//! answers the window behind it), so ghost mode would be a one-way door: the note
//! cannot be clicked, cannot take focus, and `Ctrl+Shift+H` (an island shortcut)
//! cannot reach it. Keeping the *content* at 20% also answers the actual complaint
//! that started this: a fully transparent note cannot be located after focus moves
//! elsewhere. The strip stays fully visible at 20% and stays hit-testable, so the
//! note can always be clicked back.
//!
//! ## What the key colour must not be
//!
//! If the strip colour ever equals the panel colour, the whole window becomes keyed
//! and therefore unclickable. The caller detects that case and falls back to a flat
//! 20% blend ([`Blend::Flat`]) rather than shipping a note that cannot be recovered.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetLayeredWindowAttributes, GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW,
    SetWindowPos, GWL_EXSTYLE, LWA_ALPHA, LWA_COLORKEY, SWP_FRAMECHANGED, SWP_NOMOVE,
    SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_LAYERED,
};

/// Opacity of everything that is *not* keyed out in ghost mode: 20%.
///
/// Not 1. A one-byte ghost is invisible, which is the state this batch exists to
/// fix — after focus leaves, there is nothing on screen to aim at. 20% keeps the
/// text and the strip locatable while the body itself is gone.
pub const GHOST_CONTENT_ALPHA: u8 = 51;

/// One compositing state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// The whole surface, background included, at `alpha`.
    Flat(u8),
    /// `key`-coloured pixels disappear and stop hit-testing; the rest stay at
    /// [`GHOST_CONTENT_ALPHA`].
    Ghost { key: u32 },
}

impl Blend {
    /// The byte the compositor will use.
    pub fn alpha(self) -> u8 {
        match self {
            Blend::Flat(alpha) => alpha,
            Blend::Ghost { .. } => GHOST_CONTENT_ALPHA,
        }
    }

    /// The colour key, when this blend uses one.
    pub fn key(self) -> Option<u32> {
        match self {
            Blend::Flat(_) => None,
            Blend::Ghost { key } => Some(key),
        }
    }

    /// Compact form for the log line.
    pub fn describe(self) -> String {
        match self {
            Blend::Flat(alpha) => format!("flat alpha={alpha}"),
            Blend::Ghost { key } => {
                format!("ghost key={key:06X} alpha={GHOST_CONTENT_ALPHA}")
            }
        }
    }
}

/// Opacity `0..=1` as the byte `SetLayeredWindowAttributes` wants.
///
/// Rounding, not truncation: `0.7 * 255` is 178.5, and dropping the half would make
/// a 20% step land at 50 instead of 51 — the label would say `20%` while the window
/// is at 19.6%. Non-finite input falls back to fully opaque: a note nobody can see
/// is worse than one at the wrong alpha.
///
/// `0.0` maps to `0`, which as a [`Blend::Flat`] is click-through. That is not a
/// hazard in practice because the caller routes `0` to [`Blend::Ghost`] instead —
/// see the module comment — but the mapping stays honest rather than silently
/// lifting the value, so a future caller cannot believe it got "0% and clickable".
pub fn from_opacity(opacity: f32) -> u8 {
    if !opacity.is_finite() {
        return 255;
    }
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The inverse, for logs and for reading a state back out of the OS.
pub fn to_opacity(alpha: u8) -> f32 {
    alpha as f32 / 255.0
}

/// The blend for ghost mode, given the note's two background colours.
///
/// `panel` is the body colour on both sides of the child-window boundary (egui paints
/// it, `EM_SETBKGNDCOLOR` fills the RichEdit with it), which is why it works as the
/// key without a repaint. `strip` is the title strip: it has to stay visible, and it
/// is also the only reliably clickable handle left when the body is keyed out.
///
/// If the two are equal the whole window would be keyed and therefore unclickable, so
/// that case degrades to a flat 20% — locatable and clickable, just without removing
/// the body. The returned note is for the log, empty in the normal case.
pub fn ghost(panel: u32, strip: u32) -> (Blend, &'static str) {
    if strip == panel {
        return (Blend::Flat(GHOST_CONTENT_ALPHA), "key=off reason=strip-equals-panel");
    }
    (Blend::Ghost { key: panel }, "")
}

/// Apply a blend to the whole window, making it layered first if it is not.
///
/// Returns whether the OS accepted the attribute. The `WS_EX_LAYERED` bit is added
/// by `SetWindowLongPtrW` and only takes effect after the `SWP_FRAMECHANGED` pass —
/// without it the window is left *hidden* until the next attribute call, which
/// reads as "the note vanished while the log said ok". Setting the bit is idempotent,
/// so an unchanged value still costs two cheap calls.
///
/// Switching from [`Blend::Ghost`] back to [`Blend::Flat`] must clear the key, which
/// is why the flat branch passes `0` and `LWA_ALPHA` only: `SetLayeredWindowAttributes`
/// replaces the previous attribute wholesale, and a stale key would keep keying the
/// panel out of a 100% note.
pub fn apply(hwnd: isize, blend: Blend) -> bool {
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
        match blend {
            Blend::Flat(alpha) => SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA) != 0,
            Blend::Ghost { key } => {
                SetLayeredWindowAttributes(hwnd, key, GHOST_CONTENT_ALPHA, LWA_COLORKEY | LWA_ALPHA)
                    != 0
            }
        }
    }
}

/// Read back what the compositor is actually using: `alpha`, plus the key when one
/// is in force.
///
/// `None` means "not layered" (or the window is gone). This exists so the log can
/// record the value the OS kept rather than the value we asked for: a silent
/// downgrade would otherwise look identical to a successful call.
pub fn get(hwnd: isize) -> Option<(u8, Option<u32>)> {
    let mut alpha: u8 = 255;
    let mut key = 0u32;
    let mut flags = 0u32;
    let ok = unsafe {
        GetLayeredWindowAttributes(hwnd as HWND, &mut key, &mut alpha, &mut flags)
    };
    if ok == 0 {
        return None;
    }
    if flags & LWA_COLORKEY != 0 {
        Some((alpha, Some(key)))
    } else {
        Some((alpha, None))
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
    fn ghost_content_is_twenty_percent_and_a_key() {
        let blend = Blend::Ghost { key: 0x00_22_15_0D };
        assert_eq!(blend.alpha(), 51);
        assert_eq!(blend.alpha(), GHOST_CONTENT_ALPHA);
        assert_eq!(blend.key(), Some(0x00_22_15_0D));
        // 51/255 is 20.0%, so the strip label and the pixels agree.
        assert_eq!((to_opacity(blend.alpha()) * 100.0).round() as i32, 20);
    }

    #[test]
    fn a_flat_blend_never_carries_a_key() {
        // The regression this guards: leaving LWA_COLORKEY set while going back to
        // 100% would key the panel out of an opaque note, i.e. punch a hole in it.
        let blend = Blend::Flat(255);
        assert_eq!(blend.key(), None);
        assert_eq!(blend.alpha(), 255);
    }

    #[test]
    fn ghost_keys_the_panel_and_keeps_the_strip_visible() {
        let panel = 0x00_22_15_0D;
        let strip = 0x00_44_2E_1F;
        let (blend, note) = ghost(panel, strip);
        assert_eq!(blend.key(), Some(panel));
        assert_eq!(blend.alpha(), 51);
        assert_eq!(note, "");
    }

    #[test]
    fn a_strip_the_same_colour_as_the_panel_falls_back_to_flat() {
        // Keying both would leave nothing clickable: the note would be visible as
        // 20% text with no way back except the tray's "close all".
        let same = 0x00_22_15_0D;
        let (blend, note) = ghost(same, same);
        assert_eq!(blend.key(), None);
        assert_eq!(blend.alpha(), GHOST_CONTENT_ALPHA);
        assert_eq!(note, "key=off reason=strip-equals-panel");
    }

    #[test]
    fn zero_maps_to_zero_and_the_caller_routes_it_to_ghost() {
        // Honest mapping: `Flat(0)` really is click-through, so the note layer must
        // send 0 to `Blend::Ghost`. Pinned here so the two stay in sync.
        assert_eq!(from_opacity(0.0), 0);
        assert_eq!(Blend::Ghost { key: 1 }.alpha(), GHOST_CONTENT_ALPHA);
    }

    #[test]
    fn out_of_range_and_nan_fall_back_to_visible() {
        assert_eq!(from_opacity(2.5), 255, "clamping up must stay at opaque");
        assert_eq!(from_opacity(-1.0), 0);
        assert_eq!(from_opacity(f32::NAN), 255);
        assert_eq!(from_opacity(f32::INFINITY), 255);
    }

    #[test]
    fn a_label_step_survives_the_round_trip() {
        // The strip prints `(opacity * 100).round()`, and the window uses the byte.
        // Five steps down must read 0% at the same time the window reaches ghost.
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
