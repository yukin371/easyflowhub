//! Process DPI awareness.
//!
//! This has to happen **before** the first monitor query or cursor read, otherwise
//! Windows virtualises coordinates for the whole process: on a 3840x2160 @150%
//! screen, `GetMonitorInfoW` reports 2560x1440 and `GetCursorPos` returns a scaled
//! point. The panel would then be placed at 2/3 of its intended position — a bug
//! that only shows up on scaled displays, i.e. exactly the target environment.
//!
//! `PER_MONITOR_AWARE_V2` is the same mode winit requests for the window; making the
//! process aware first means our cached layout and winit's `pixels_per_point` agree.

use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

/// Returns true when the process is now per-monitor v2 aware.
///
/// A `false` result is not fatal: it means the awareness was already set (by a
/// manifest or by winit), which is the normal case in a packaged build.
pub fn make_per_monitor_aware() -> bool {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) != 0 }
}

/// Effective scale of the primary monitor, for logging before any window exists.
pub fn system_dpi() -> u32 {
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
    unsafe { GetDpiForSystem() }
}
