//! Monitor layout snapshot.
//!
//! `EnumDisplayMonitors` is the call that makes the reveal path slow: on a
//! mixed-DPI desktop it walks every adapter and each transition costs a
//! `WM_DPICHANGED` round of work. §5.1.1 calls this out as one of the three
//! latency traps, so the snapshot is taken at startup and only refreshed when the
//! OS says the layout changed (`WM_DISPLAYCHANGE` / `WM_SETTINGCHANGE`).
//!
//! The snapshot is plain data (`MonitorRect`), which is also why the clamping
//! itself can live in `hub-core` and be unit-tested.

use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, MONITORINFOEXW};

/// `MONITORINFOF_PRIMARY` from `wingdi.h`; not re-exported by windows-sys 0.61.
const MONITORINFOF_PRIMARY: u32 = 1;
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// One monitor, in physical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorRect {
    pub name: String,
    /// Full bounds `(left, top, right, bottom)`.
    pub rect: (i32, i32, i32, i32),
    /// Work area — the desktop minus taskbar and appbars.
    pub work: (i32, i32, i32, i32),
    /// Effective DPI / 96. 1.0 at 100%, 1.5 at 150%.
    pub scale: f32,
    pub primary: bool,
}

impl MonitorRect {
    pub fn work_width(&self) -> i32 {
        self.work.2 - self.work.0
    }

    pub fn work_height(&self) -> i32 {
        self.work.3 - self.work.1
    }
}

/// Immutable snapshot; rebuild with `refresh()` when the OS reports a change.
#[derive(Debug, Clone, Default)]
pub struct MonitorCache {
    monitors: Vec<MonitorRect>,
}

impl MonitorCache {
    pub fn refresh() -> Self {
        let mut collected: Vec<MonitorRect> = Vec::new();
        // SAFETY: `collect` only writes into `collected` through the `LPARAM`
        // pointer we control for the duration of the call.
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(collect_monitor),
                &mut collected as *mut Vec<MonitorRect> as isize,
            );
        }
        if collected.is_empty() {
            // Degenerate case (no monitor reported): synthesise a 1920x1080 at
            // 100% so the panel still has somewhere to go instead of panicking.
            collected.push(MonitorRect {
                name: "fallback".to_string(),
                rect: (0, 0, 1920, 1080),
                work: (0, 0, 1920, 1040),
                scale: 1.0,
                primary: true,
            });
        }
        Self { monitors: collected }
    }

    pub fn as_slice(&self) -> &[MonitorRect] {
        &self.monitors
    }

    pub fn len(&self) -> usize {
        self.monitors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.monitors.is_empty()
    }

    pub fn primary(&self) -> &MonitorRect {
        self.monitors
            .iter()
            .find(|m| m.primary)
            .unwrap_or(&self.monitors[0])
    }

    /// Monitor containing `(x, y)`, with the same fallback order Windows uses:
    /// nearest work area, then primary.
    pub fn at(&self, x: i32, y: i32) -> &MonitorRect {
        let hit = self
            .monitors
            .iter()
            .find(|m| {
                x >= m.rect.0 && x < m.rect.2 && y >= m.rect.1 && y < m.rect.3
            });
        hit.unwrap_or_else(|| self.primary())
    }
}

unsafe extern "system" fn collect_monitor(
    handle: *mut core::ffi::c_void,
    _hdc: *mut core::ffi::c_void,
    _clip: *mut RECT,
    data: isize,
) -> i32 {
    let collected = &mut *(data as *mut Vec<MonitorRect>);

    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let ok = GetMonitorInfoW(handle, &mut info.monitorInfo);
    if ok == 0 {
        return 1; // keep enumerating
    }

    let mut dpi_x = 96u32;
    let mut dpi_y = 96u32;
    let hr = GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    let scale = if hr >= 0 && dpi_x > 0 {
        dpi_x as f32 / 96.0
    } else {
        1.0
    };

    let name_len = info
        .szDevice
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(info.szDevice.len());
    let name = String::from_utf16_lossy(&info.szDevice[..name_len]);

    let r = info.monitorInfo.rcMonitor;
    let w = info.monitorInfo.rcWork;
    collected.push(MonitorRect {
        name,
        rect: (r.left, r.top, r.right, r.bottom),
        work: (w.left, w.top, w.right, w.bottom),
        scale,
        primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    });

    1
}

/// Cursor position in physical pixels.
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok == 0 {
        None
    } else {
        Some((point.x, point.y))
    }
}
