//! Key vocabulary shared by the island and the shell.
//!
//! Split out of `lib.rs` when the file hit the 800-line hard cap. The boundary is
//! real, not a line-count accident: this module is the only place that knows a
//! virtual-key code or reads modifier state, while `lib.rs` stays about the control.

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};

/// Virtual-key codes the shell needs to talk about.
///
/// Plain `u16` constants on purpose: `app` must not name a win32 type, so this
/// crate keeps the platform vocabulary on its side of the boundary.
pub const TAB: u16 = 0x09;
pub const ESCAPE: u16 = 0x1B;
pub const D: u16 = 0x44;
pub const T: u16 = 0x54;
pub const Z: u16 = 0x5A;
/// `VK_OEM_PERIOD` — the `.` next to `/` on a US layout. `Ctrl+.` is the shipped
/// tools-panel shortcut, and it is *not* `VK_DECIMAL` (numpad).
pub const OEM_PERIOD: u16 = 0xBE;

/// A key press reduced to what a shortcut table needs.
///
/// `WM_KEYDOWN` does not carry the modifier state and the shell cannot ask winit
/// either: while the island has focus the keystroke is delivered here and never
/// reaches the egui window, so this is the only place that can see it. Reading
/// `GetKeyState` at message time is what makes `Ctrl+D` distinguishable from `D`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyPress {
    pub vk: u16,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

/// Snapshot the modifier state at message time.
///
/// `GetKeyState` reads the queue state for the current thread, which is the only
/// correct source here: `lparam` says nothing about Shift/Ctrl, and by the time a
/// `WM_CHAR` arrives the modifiers may already be released.
pub(crate) fn modifiers(vk: u16) -> KeyPress {
    unsafe {
        KeyPress {
            vk,
            ctrl: GetKeyState(VK_CONTROL as i32) < 0,
            shift: GetKeyState(VK_SHIFT as i32) < 0,
            alt: GetKeyState(VK_MENU as i32) < 0,
        }
    }
}
