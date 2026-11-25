//! Window placement helpers.
//!
//! Owned by the platform layer rather than by egui viewport commands on purpose:
//! egui talks in points, win32 in physical pixels, and the spike has to prove the
//! panel lands on an exact physical rectangle. Converting back and forth at reveal
//! time is a rounding bug waiting to happen — the "half size for one frame" symptom
//! in §5.1.1 is exactly that class of bug.
//!
//! ## Why "parked" instead of "hidden"
//!
//! `ShowWindow(SW_HIDE)` looks like the obvious way to close the panel, and it is
//! what the Tauri build does. It also stops `WM_PAINT` delivery: winit then has no
//! redraw events to drive eframe's loop, and any `request_repaint` scheduled while
//! hidden is simply never serviced. Measured consequence: the first bench cycle
//! worked and every following one stalled.
//!
//! So the panel is kept *visible but off-screen* when closed. It stays redrawable,
//! never appears in the taskbar (`with_taskbar(false)`), and can be moved back with
//! a single `SetWindowPos` — which is all a reveal has to do anyway.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
};

/// Far enough that no virtual desktop (including a 4-monitor 4K wall) reaches it.
///
/// Measured on Win11 26300: asking for -30000 lands at -20000, so the OS clamps
/// the coordinate. The intent (off every monitor) survives the clamp, but the
/// number that ends up in `GetWindowRect` is -20000, not this constant -- do not
/// assert on the constant when checking a parked window.
const PARKED_X: i32 = -30_000;
const PARKED_Y: i32 = -30_000;

/// Move, resize and focus in one call — the reveal path.
///
/// The window is already visible (parked), so this only has to move it. Focus is
/// granted explicitly afterwards: the panel must accept typing the instant it
/// appears, and `SetForegroundWindow` after the move is the ordering that works
/// when the foreground lock is held by the previously active window.
pub fn move_on_screen(hwnd: isize, x: i32, y: i32, width: i32, height: i32, topmost: bool) {
    let hwnd = hwnd as HWND;
    let insert_after = if topmost { HWND_TOPMOST } else { std::ptr::null_mut() };
    let flags = if topmost {
        SWP_SHOWWINDOW
    } else {
        SWP_NOZORDER | SWP_SHOWWINDOW
    };

    unsafe {
        SetWindowPos(hwnd, insert_after, x, y, width, height, flags);
    }
}

/// Give the panel keyboard focus.
///
/// Split from the move because it is the expensive half: `SetForegroundWindow` has to
/// negotiate the foreground lock with the shell and is routinely denied or delayed
/// when the calling thread is not already foreground. Measuring it separately is the
/// difference between "the panel takes 50 ms to appear" and "appearing is instant,
/// taking focus is not".
pub fn focus(hwnd: isize) -> bool {
    unsafe { SetForegroundWindow(hwnd as HWND) != 0 }
}

/// Set size/position without showing or focusing — startup only.
pub fn place_hidden(hwnd: isize, x: i32, y: i32, width: i32, height: i32) {
    let hwnd = hwnd as HWND;
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Make the window visible without stealing focus, so the warm frames can paint.
pub fn show_without_activation(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_SHOWNOACTIVATE);
    }
}

/// Close the panel: move it off-screen, give the keyboard back, keep it redrawable.
///
/// Moving the window is not closing it as far as the keyboard is concerned. The
/// island keeps keyboard focus while it sits off-screen, so after Esc the panel is
/// invisible but still the focus owner: typing goes into a window nobody can see.
/// Measured 2026-11-25 with `scripts/esc_poke.rs` -- `focus_is_editor=true` after
/// park, and a byte landed in the document. That is worse than a focus leak: the
/// user believes the panel is closed and keeps typing into their other window.
///
/// `SetFocus(null)` is the smallest fix that does not touch the reveal path. It
/// does **not** hand the foreground to the previously active window: doing that
/// would make the next `SetForegroundWindow` negotiate the foreground lock, which
/// changes what the latency benchmark measures (today's cycles start with this
/// process already foreground, an optimistic condition worth recording separately
/// rather than silently changing mid-benchmark). Releasing focus is enough to stop
/// the keystrokes; the foreground question belongs to Phase 1, where reveals from
/// another application are the normal case.
///
/// `SW_HIDE` remains wrong here -- see the module comment: hiding stops
/// `WM_PAINT` delivery and stalls the next bench cycle.
///
/// Returns the window that held focus before the call (`0` if none), so callers
/// can log what was released instead of asserting a fix by inspection.
pub fn park(hwnd: isize) -> isize {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};

    let hwnd = hwnd as HWND;
    let previous = unsafe { GetFocus() };
    unsafe {
        SetFocus(std::ptr::null_mut());
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            PARKED_X,
            PARKED_Y,
            0,
            0,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE,
        );
    }
    previous as isize
}

/// Keyboard focus of the calling thread, as a raw handle (`0` = none).
///
/// Only meaningful on the UI thread: `GetFocus` answers for the caller's message
/// queue, so a cross-thread check reads this thread's focus, not the panel's.
/// Used to prove that parking really gave the keyboard back -- a zero here after
/// `park` is the difference between "closed" and "invisible but still typing".
pub fn current_focus() -> isize {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetFocus;

    unsafe { GetFocus() as isize }
}

/// Locate this process's window by title.
///
/// eframe 0.31 does not expose the `HWND` through `App`, and the spike needs raw
/// win32 for three things eframe cannot express: physical-pixel placement, DWM
/// chrome, and off-screen parking. `FindWindowW` over a fixed title is the least
/// invasive bridge; Phase 1 will plumb the handle out of winit instead and delete
/// this function (recorded in `native/README.md`).
pub fn find_by_title(title: &str) -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
    if hwnd.is_null() {
        None
    } else {
        Some(hwnd as isize)
    }
}

/// A top-level window owned by this process.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub title: String,
    pub visible: bool,
}

/// Enumerate this process's top-level windows.
///
/// This is the evidence for "exactly one window, and it is not on any monitor at
/// startup": counting from outside via `MainWindowHandle` misses windows that are
/// not focusable, which is precisely the state that has to be proven.
pub fn top_level_windows() -> Vec<WindowInfo> {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };

    unsafe extern "system" fn collect(hwnd: *mut core::ffi::c_void, data: isize) -> i32 {
        let out = &mut *(data as *mut Vec<WindowInfo>);
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid != unsafe { GetCurrentProcessId() } {
            return 1;
        }

        let len = unsafe { GetWindowTextLengthW(hwnd) };
        let mut buffer = vec![0u16; len.max(0) as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        let title = String::from_utf16_lossy(&buffer[..copied.max(0) as usize]);

        out.push(WindowInfo {
            title,
            visible: unsafe { IsWindowVisible(hwnd) } != 0,
        });
        1
    }

    let mut windows: Vec<WindowInfo> = Vec::new();
    unsafe {
        EnumWindows(Some(collect), &mut windows as *mut Vec<WindowInfo> as isize);
    }
    windows
}
