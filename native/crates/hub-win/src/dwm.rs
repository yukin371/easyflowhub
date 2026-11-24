//! DWM chrome: dark frame, rounded corners, optional Mica.
//!
//! Everything here is best-effort and must never fail the app: on Windows 10 the
//! backdrop attribute simply returns an error, and on a machine where DWM is off
//! (remote session, composition disabled) the panel still has to appear. The
//! return value is the list of attributes that actually took effect, which the
//! startup log records so "it looked flat on that machine" has an answer.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

// Values from `dwmapi.h`; kept as literals so the spike does not depend on which
// SDK version the installed `windows-sys` happens to expose.
const DWMWA_USE_IMMERSIVE_DARK_MODE: i32 = 20;
const DWMWA_WINDOW_CORNER_PREFERENCE: i32 = 33;
const DWMWA_SYSTEMBACKDROP_TYPE: i32 = 38;

const DWMWCP_ROUND: i32 = 2;
const DWMSBT_NONE: i32 = 1;
const DWMSBT_MAINWINDOW: i32 = 2; // Mica

fn set_i32(hwnd: HWND, attribute: i32, value: i32) -> bool {
    let bytes = value.to_ne_bytes();
    let hr = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute as u32,
            bytes.as_ptr() as *const core::ffi::c_void,
            bytes.len() as u32,
        )
    };
    hr >= 0
}

/// Apply the shell chrome and report what stuck.
pub fn apply(hwnd: isize, lite: bool) -> Vec<&'static str> {
    let hwnd = hwnd as HWND;
    let mut applied = Vec::new();

    // Dark frame regardless of profile: the panel is a dark surface even in the
    // light theme (it is an action panel over other windows, not a document).
    if set_i32(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, 1) {
        applied.push("dark");
    }

    if !lite {
        if set_i32(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND) {
            applied.push("round");
        }
        if set_i32(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW) {
            applied.push("mica");
        }
    } else {
        // `lite` asks for the cheapest path: square, no backdrop, no extra
        // composition step. Explicitly writing NONE also overrides a theme default.
        set_i32(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, 1);
        set_i32(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_NONE);
    }

    applied
}
