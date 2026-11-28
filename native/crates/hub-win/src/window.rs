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
    IsIconic, SetForegroundWindow, SetWindowPos, ShowWindow, HWND_NOTOPMOST, HWND_TOPMOST,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_MINIMIZE, SW_RESTORE,
    SW_SHOWNOACTIVATE,
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

/// Move the window without resizing, re-ordering or activating it.
///
/// The drag in `app::note::drag` calls this once per frame while the left button
/// is held. `SWP_NOACTIVATE` matters as much here as in [`park`]: a window that
/// activates on every frame of a drag would fight the focus it is trying to keep.
pub fn move_to(hwnd: isize, x: i32, y: i32) -> bool {
    unsafe {
        SetWindowPos(
            hwnd as HWND,
            std::ptr::null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        ) != 0
    }
}

/// Move **and** resize in one call, without activating the window.
///
/// The resize in `app::note::edge` needs both halves every frame: grabbing the
/// left edge moves the origin and shrinks the width at the same time, and two
/// calls would show one of them for a frame. `SWP_NOZORDER` keeps a pinned note on
/// top while its edge is dragged; `SWP_NOACTIVATE` keeps the keyboard where it is.
///
/// Coordinates and size are physical pixels, which is the space the cursor is in.
pub fn set_frame(hwnd: isize, x: i32, y: i32, width: i32, height: i32) -> bool {
    unsafe {
        SetWindowPos(
            hwnd as HWND,
            std::ptr::null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        ) != 0
    }
}

/// The whole window rectangle, in physical screen pixels: `(x, y, w, h)`.
///
/// The resize reads this rather than the client size: an edge drag is defined on
/// the window's outer edges, and this note is undecorated, so the two rectangles
/// differ by nothing today -- reading the window one keeps the arithmetic honest
/// if it ever gains a frame.
pub fn window_rect(hwnd: isize) -> (i32, i32, i32, i32) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetWindowRect(hwnd as HWND, &mut rect) };
    if ok == 0 {
        return (0, 0, 0, 0);
    }
    (
        rect.left,
        rect.top,
        (rect.right - rect.left).max(0),
        (rect.bottom - rect.top).max(0),
    )
}

/// Whether the left mouse button is physically down, right now.
///
/// `GetAsyncKeyState`, not `GetKeyState`: the latter answers from the last message
/// the thread retrieved, and the resize runs in the frame loop, which is driven by
/// `request_repaint` rather than by mouse messages -- during an edge drag the
/// messages go to the island's control, not to this thread's queue in a way the
/// frame can see.
pub fn left_button_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

    unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 }
}

/// The cursor in physical screen pixels, and nothing else.
///
/// The drag reads this instead of egui's pointer position because egui reports
/// *client* coordinates and the client moves with the window: every frame after
/// the first would report the same delta, which is zero, and the window would
/// never follow.
pub fn cursor_position() -> (i32, i32) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut point) };
    (point.x, point.y)
}

/// Where the client area starts on screen, in physical pixels.
///
/// This is the window's origin for a borderless shell (client and window rects are
/// the same rectangle), and it is what a drag has to preserve: the offset between
/// the cursor and this point must not change while the button is held.
pub fn client_origin(hwnd: isize) -> (i32, i32) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ClientToScreen;

    let mut point = POINT { x: 0, y: 0 };
    unsafe { ClientToScreen(hwnd as HWND, &mut point) };
    (point.x, point.y)
}

/// Turn always-on-top on or off, touching only the z-order bit.
///
/// `ViewportCommand::WindowLevel` is the obvious route and it **hid the window**.
/// winit applies a level change by rebuilding the whole window style from its
/// cached `WindowFlags`, and this window is created `with_visible(false)` and then
/// shown by [`move_on_screen`] — all of it outside winit's model. So the first
/// level change wrote "not visible" (and dropped the no-activate style) and the
/// note disappeared; measured 2026-11-26 with `IsWindowVisible` plus a screen grab
/// that came back as the desktop. `SetWindowPos` with `HWND_TOPMOST` /
/// `HWND_NOTOPMOST` moves the window in the z-order and leaves the rest of the
/// style alone, which is the only part of the request that was wanted.
///
/// `SWP_NOACTIVATE`: unpinning must not pull the keyboard away from whatever the
/// user is typing in. Returns whether the OS accepted the call, so the caller can
/// log a failure instead of painting a highlight over a window that did not move.
pub fn set_topmost(hwnd: isize, on: bool) -> bool {
    let insert_after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
    unsafe {
        SetWindowPos(
            hwnd as HWND,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        ) != 0
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

/// Make the window a child of the desktop, or a top-level window again.
///
/// This is what the product calls "固定到桌面" (`toggle_todo_card_pin` in the Tauri
/// build does the same `GWLP_HWNDPARENT` write). A child of `Progman` sits on the
/// wallpaper: it goes behind normal windows, it survives "show desktop", and it
/// cannot be covered by the card's own always-on-top level fighting the shell.
///
/// `SWP_FRAMECHANGED` after the write is not decoration: the window keeps its old
/// non-client area until it is told to recompute, which shows up as a card that
/// does not repaint where it was moved to.
///
/// Returns `false` when pinning was asked for and the desktop window is missing --
/// a session with no shell is the one case where the request cannot be honoured,
/// and the caller must say so rather than paint a filled pin over a free window.
pub fn attach_to_desktop(hwnd: isize, on: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetWindowLongPtrW, GWLP_HWNDPARENT, SWP_FRAMECHANGED,
    };

    let parent = if on {
        let name: Vec<u16> = "Progman".encode_utf16().chain(std::iter::once(0)).collect();
        let progman = unsafe { FindWindowW(std::ptr::null(), name.as_ptr()) };
        if progman.is_null() {
            return false;
        }
        progman as isize
    } else {
        0
    };
    unsafe {
        SetWindowLongPtrW(hwnd as HWND, GWLP_HWNDPARENT, parent);
        SetWindowPos(
            hwnd as HWND,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
    true
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

/// The window the user is typing into, system-wide (`0` = none).
///
/// Unlike [`current_focus`] this answers for the whole desktop and is safe to call
/// from any thread. The panel uses it to decide that a click went somewhere else:
/// "the keyboard moved" is an OS fact, and asking egui whether *it* thinks it is
/// focused makes the answer depend on when its next event happened to arrive.
pub fn foreground() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    unsafe { GetForegroundWindow() as isize }
}

/// The control that has the keyboard **inside** `hwnd`'s thread (`0` = none).
///
/// `current_focus` only answers for the calling thread, which is useless here:
/// the launcher wants to know which edit box a *note* had focused, from the hotkey
/// thread. `GetGUIThreadInfo` is the cross-thread form of that question.
///
/// This matters because the panel and the notes share one thread. Parking the
/// panel calls `SetFocus(NULL)` to give the keyboard up, and that clears the
/// thread's focus window -- which was the note's editor. Putting the window back in
/// front does not bring the editor's focus back; Windows had already handed it
/// away. Remembering the child is what makes "Esc returns you to your note"
/// mean the caret, not just the z-order.
pub fn focused_child(hwnd: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO};

    if hwnd == 0 {
        return 0;
    }
    let thread = unsafe { GetWindowThreadProcessId(hwnd as HWND, std::ptr::null_mut()) };
    if thread == 0 {
        return 0;
    }
    let mut info: GUITHREADINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(thread, &mut info) } == 0 {
        return 0;
    }
    info.hwndFocus as isize
}

/// Give `hwnd` the keyboard, if it lives on the calling thread.
///
/// `SetFocus` is a thread-local call by design: a process cannot move another
/// process's focus, and that is fine -- a foreign application restores its own
/// focus when it is activated. The caller is the launcher's UI thread, which is
/// the thread the notes run on, so the case that needs this is exactly the case
/// where it works.
pub fn set_focus(hwnd: isize) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;

    hwnd != 0 && unsafe { SetFocus(hwnd as HWND) != std::ptr::null_mut() }
}

/// Collapse a note into the taskbar (`Ctrl+Alt+H`).
///
/// `SW_MINIMIZE` rather than `SW_HIDE`: hiding a note would stop `WM_PAINT` for it
/// (see the module comment), and the same trick that keeps the parked panel drawable
/// is what makes "hide all" impossible to implement without freezing the editors.
/// A minimized window is still a window: the island keeps its handle, the text stays
/// in the control, and restoring is one `ShowWindow`.
pub fn minimize(hwnd: isize) -> bool {
    unsafe { ShowWindow(hwnd as HWND, SW_MINIMIZE) != 0 }
}

/// Bring a note back. `SW_RESTORE` both un-minimizes and activates; the activation is
/// incidental -- callers decide separately which note gets the keyboard.
pub fn restore(hwnd: isize) -> bool {
    unsafe { ShowWindow(hwnd as HWND, SW_RESTORE) != 0 }
}

/// Whether the window is collapsed, as opposed to merely covered.
///
/// Used to pick the direction of `Ctrl+Alt+H`: the shipped toggle minimizes when any
/// note is *active*, which `IsIconic` answers without guessing from visibility (our
/// windows are always visible, they are just off-screen or behind something).
pub fn is_minimized(hwnd: isize) -> bool {
    unsafe { IsIconic(hwnd as HWND) != 0 }
}

/// Client area size in physical pixels.
///
/// The host needs this instead of trusting the UI framework's idea of its own
/// size: on the very first frame eframe 0.31 reports a placeholder screen rect
/// (10000 px), and an island created from that is a 15000 px child window that
/// the OS then has to clip. Asking Windows is authoritative and costs one
/// `GetClientRect` on the frame where the size actually changed.
pub fn client_size(hwnd: isize) -> (i32, i32) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetClientRect(hwnd as HWND, &mut rect) };
    if ok == 0 {
        return (0, 0);
    }
    ((rect.right - rect.left).max(0), (rect.bottom - rect.top).max(0))
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
