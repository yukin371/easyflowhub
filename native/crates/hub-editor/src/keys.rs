//! Key vocabulary shared by the island and the shell.
//!
//! Split out of `lib.rs` when the file hit the 800-line hard cap. The boundary is
//! real, not a line-count accident: this module is the only place that knows a
//! virtual-key code or reads modifier state, while `lib.rs` stays about the control.

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Input::Ime::ImmGetVirtualKey;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, MapVirtualKeyW, VK_CONTROL, VK_MENU, VK_SHIFT, MAPVK_VSC_TO_VK_EX,
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

/// Undo the IME's rewrite of the virtual-key code.
///
/// With a Chinese IME active, `WM_KEYDOWN` can arrive with `wParam` set to
/// `VK_PROCESSKEY` (0xE5) instead of the key the user pressed: the IME claims the
/// combo before the control sees it. Measured 2026-11-25 — `Ctrl+.` after Sogou was
/// installed arrived as `vk=0xE5`, so the tools shortcut looked "not implemented".
///
/// Two ways back, in order:
/// 1. `ImmGetVirtualKey` — the documented answer, but it returns 0xE5 itself on this
///    RichEdit, so it is not enough on its own. TSF-based IMEs do not go through
///    IMM32, which is why.
/// 2. The scan code in `lParam` bits 16–23, mapped back with the thread's layout.
///    This is what actually works here; the IME rewrites `wParam` and leaves
///    `lParam` alone. Synthetic input leaves the scan code at 0, so this branch is
///    only exercised by a real keyboard.
///
/// Consuming the callback's key afterwards also keeps the IME from acting twice: if
/// the shell answers the shortcut, the message is not passed on and Sogou does *not*
/// also toggle punctuation.
pub(crate) fn original_key(hwnd: HWND, vk: u16, lparam: LPARAM) -> u16 {
    const VK_PROCESSKEY: u16 = 0xE5;
    if vk != VK_PROCESSKEY {
        return vk;
    }
    let original = unsafe { ImmGetVirtualKey(hwnd) } as u16;
    if original != 0 && original != VK_PROCESSKEY {
        return original;
    }

    let scan = ((lparam as usize >> 16) & 0xFF) as u32;
    if scan == 0 {
        return vk;
    }
    let mapped = unsafe { MapVirtualKeyW(scan, MAPVK_VSC_TO_VK_EX) } as u16;
    if mapped == 0 {
        vk
    } else {
        mapped
    }
}
