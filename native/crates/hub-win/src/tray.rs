//! The notification-area icon: `Shell_NotifyIconW` on a hidden message window.
//!
//! ## Why this is not the controller window
//!
//! The controller window is created by winit and parked 20 000 px off-screen. The
//! tray could report to it -- one `SetWindowSubclass` and the callback lands in the
//! app -- but then the tray's menu would be owned by a window whose lifecycle
//! belongs to another crate, and "the menu did not dismiss" would be debugged in
//! three places. A private message window costs one `RegisterClass` and keeps the
//! ownership of every handle in this module.
//!
//! ## Two Win32 details that look optional and are not
//!
//! * `SetForegroundWindow` before `TrackPopupMenu`, then a posted `WM_NULL` after.
//!   Without them the menu stays on screen after the user clicks elsewhere: the
//!   popup is a separate window and only dismisses while its owner is foreground
//!   (the classic "tray menu sticks" bug, present since Windows 95).
//! * The icon is deleted with `NIM_DELETE` before the process exits. `Drop` covers
//!   the ordinary path, but the controller leaves through `std::process::exit`
//!   (which runs no destructors), so [`Tray::remove`] is public and called there.
//!   Skipping it leaves a ghost icon in the notification area until the user hovers
//!   over it -- a dead icon that looks like a crashed process.

use std::ffi::c_void;
use std::sync::Arc;

use windows_sys::Win32::Foundation::{GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyMenu, DestroyWindow, GetCursorPos, GetWindowLongPtrW, LoadIconW,
    PostMessageW, RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, TrackPopupMenu,
    GWLP_USERDATA, HICON, HMENU, IDI_APPLICATION, MF_SEPARATOR, MF_STRING,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND, WM_LBUTTONUP, WM_NULL,
    WM_RBUTTONUP, WNDCLASSW, WS_POPUP,
};

/// What the user picked from the tray menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    NewNote,
    TodoCard,
    CloseAll,
    Quit,
}

pub type CommandSink = Arc<dyn Fn(TrayCommand) + Send + Sync>;

/// Menu ids. Deliberately small and stable: they are what `WM_COMMAND` carries, and
/// the same table answers both the menu and a script driving `WM_COMMAND` directly.
const MENU_NEW: usize = 1;
const MENU_CLOSE_ALL: usize = 2;
const MENU_QUIT: usize = 3;
/// Appended rather than slotted in after `MENU_NEW`: the ids are what `WM_COMMAND`
/// carries, and a script that pokes the tray must keep meaning the same thing.
const MENU_TODO_CARD: usize = 4;

const CLASS_NAME: &str = "EasyFlowHubTray";
const TRAY_ID: u32 = 1;
/// Tray callbacks arrive as `WM_APP + n`; the value only has to be distinct from
/// everything winit and the controls use.
const TRAY_MESSAGE: u32 = WM_APP + 1;
/// `ERROR_CLASS_ALREADY_EXISTS` — a second `install` in one process is not a bug.
const CLASS_ALREADY_EXISTS: u32 = 1410;

/// A live notification-area icon.
pub struct Tray {
    /// Raw handles as `isize`, the same shape `window.rs` uses, so this struct stays
    /// plain data that can sit in a struct the event loop owns.
    hwnd: isize,
    icon: isize,
    /// Whether `icon` came from `CreateIconFromResourceEx` (ours to destroy) or from
    /// `LoadIconW` (a shared system icon that must not be destroyed).
    icon_owned: bool,
    source: &'static str,
    visible: bool,
}

impl Tray {
    /// Register the icon. `ico` is the raw `.ico` file, parsed here so the asset
    /// stays in `app` (which owns brand assets) and this crate owns only the handle.
    ///
    /// A missing or unreadable icon is not fatal: the tray still works with the
    /// generic application icon, and [`Tray::icon_source`] reports which one is in
    /// use so "the icon is the wrong one" is answerable from the log.
    pub fn install(tooltip: &str, ico: Option<&[u8]>, on_command: CommandSink) -> Result<Self, String> {
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            return Err("GetModuleHandleW returned 0".to_string());
        }

        let hwnd = unsafe { create_window(instance) }?;
        let boxed = Box::into_raw(Box::new(on_command));
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, boxed as isize);
        }

        let loaded = load_icon(ico);
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: TRAY_MESSAGE,
            hIcon: loaded.icon,
            ..Default::default()
        };
        copy_wide(&mut data.szTip, tooltip);

        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) } != 0;
        if !added {
            // Free what was allocated before failing, or the window and the sink leak
            // for the lifetime of the process with no icon to show for it.
            unsafe {
                drop(Box::from_raw(boxed));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DestroyWindow(hwnd);
            }
            return Err("Shell_NotifyIconW(NIM_ADD) returned FALSE".to_string());
        }

        Ok(Self {
            hwnd: hwnd as isize,
            icon: loaded.icon as isize,
            icon_owned: loaded.owned,
            source: loaded.source,
            visible: true,
        })
    }

    /// Raw window handle, for the log and for driving the tray from a script.
    pub fn hwnd(&self) -> isize {
        self.hwnd
    }

    /// Which icon got used: `ico` when the embedded asset was accepted, `system`
    /// when it fell back to `IDI_APPLICATION`.
    pub fn icon_source(&self) -> &'static str {
        self.source
    }

    /// Remove the icon from the notification area. Idempotent, and public because the
    /// controller exits through `std::process::exit` and cannot rely on `Drop`.
    pub fn remove(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        let data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd as HWND,
            uID: TRAY_ID,
            ..Default::default()
        };
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &data);
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.remove();
        unsafe {
            if self.icon_owned {
                DestroyIcon(self.icon as HICON);
            }
            let sink = GetWindowLongPtrW(self.hwnd as HWND, GWLP_USERDATA);
            if sink != 0 {
                drop(Box::from_raw(sink as *mut CommandSink));
                SetWindowLongPtrW(self.hwnd as HWND, GWLP_USERDATA, 0);
            }
            DestroyWindow(self.hwnd as HWND);
        }
    }
}

unsafe fn create_window(instance: HINSTANCE) -> Result<HWND, String> {
    let class: Vec<u16> = CLASS_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    let wndclass = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: std::ptr::null_mut::<c_void>(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class.as_ptr(),
    };
    let atom = unsafe { RegisterClassW(&wndclass) };
    if atom == 0 {
        let code = unsafe { GetLastError() };
        if code != CLASS_ALREADY_EXISTS {
            return Err(format!("RegisterClassW failed (win32 error {code})"));
        }
    }

    let title: Vec<u16> = "easyflowhub-tray".encode_utf16().chain(std::iter::once(0)).collect();
    // `WS_POPUP` and no `WS_VISIBLE`: a hidden owner window is what every tray
    // implementation uses, and it keeps the window out of the taskbar and Alt+Tab.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        let code = unsafe { GetLastError() };
        return Err(format!("CreateWindowExW failed (win32 error {code})"));
    }
    Ok(hwnd)
}

unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        TRAY_MESSAGE => {
            match (lparam as u32) & 0xFFFF {
                WM_LBUTTONUP | WM_RBUTTONUP => unsafe { show_menu(hwnd) },
                // Double clicks and mouse moves are delivered too; the menu is the
                // only gesture, so everything else is dropped on purpose.
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as usize;
            if let Some(command) = command_for(id) {
                unsafe { dispatch(hwnd, command) };
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// The id → command table. `WM_COMMAND` and the menu are two views of the same list,
/// which is why the menu is not built from string literals in two places.
fn command_for(id: usize) -> Option<TrayCommand> {
    match id {
        MENU_NEW => Some(TrayCommand::NewNote),
        MENU_TODO_CARD => Some(TrayCommand::TodoCard),
        MENU_CLOSE_ALL => Some(TrayCommand::CloseAll),
        MENU_QUIT => Some(TrayCommand::Quit),
        _ => None,
    }
}

unsafe fn dispatch(hwnd: HWND, command: TrayCommand) {
    let sink = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    if sink == 0 {
        return;
    }
    let sink = unsafe { &*(sink as *const CommandSink) };
    sink(command);
}

unsafe fn show_menu(hwnd: HWND) {
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return;
    }

    // The shipped tray menu is 打开管理中心 / 新建快速笔记 / 退出. There is no manager
    // window until Phase 2, so that slot holds 关闭全部笔记 instead: once the tray
    // keeps the process alive, "close everything" is the one command that has no
    // mouse path anywhere else (Esc and Ctrl+D close one window at a time).
    unsafe {
        append(menu, MF_STRING, MENU_NEW, "新建快速笔记\tCtrl+Alt+N");
        append(menu, MF_STRING, MENU_TODO_CARD, "待办卡片");
        append(menu, MF_STRING, MENU_CLOSE_ALL, "关闭全部笔记\tCtrl+Alt+D");
        append(menu, MF_SEPARATOR, 0, "");
        append(menu, MF_STRING, MENU_QUIT, "退出");
    }

    let mut point = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } == 0 {
        unsafe { DestroyMenu(menu) };
        return;
    }

    unsafe {
        // See the module comment: foreground before, `WM_NULL` after.
        SetForegroundWindow(hwnd);
        TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }
}

unsafe fn append(menu: HMENU, flags: u32, id: usize, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        AppendMenuW(menu, flags, id, wide.as_ptr());
    }
}

struct LoadedIcon {
    icon: HICON,
    owned: bool,
    source: &'static str,
}

/// Turn the embedded `.ico` into an `HICON`, or fall back to the system icon.
///
/// The asset is the one the Tauri build shipped, and its entries are PNG framed
/// (Vista style). `CreateIconFromResourceEx` accepts that on Vista+, but it is not
/// documented as such for every frame size, so the fallback is real code rather than
/// an `unwrap`: [`Tray::icon_source`] reports which path ran.
fn load_icon(ico: Option<&[u8]>) -> LoadedIcon {
    if let Some(bytes) = ico {
        if let Some(icon) = icon_from_ico(bytes) {
            return LoadedIcon {
                icon,
                owned: true,
                source: "ico",
            };
        }
    }
    let system = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    LoadedIcon {
        icon: system,
        owned: false,
        source: "system",
    }
}

/// Pick the 32×32 frame out of an `.ico` and wrap it in an `HICON`.
fn icon_from_ico(bytes: &[u8]) -> Option<HICON> {
    if bytes.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let mut frame: Option<(usize, usize)> = None;

    for index in 0..count {
        let at = 6 + index * 16;
        let entry = bytes.get(at..at + 16)?;
        let width = if entry[0] == 0 { 256 } else { entry[0] as usize };
        let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        // The notification area renders at 16 or 20 logical pixels; 32 is the frame
        // Windows scales down for both, and it is what is asked for below.
        if width == 32 {
            frame = Some((offset, size));
        }
    }

    let (offset, size) = frame?;
    let data = bytes.get(offset..offset.checked_add(size)?)?;
    let handle = unsafe { CreateIconFromResourceEx(data.as_ptr(), size as u32, 1, 0x0003_0000, 32, 32, 0) };
    if handle.is_null() {
        None
    } else {
        Some(handle)
    }
}

fn copy_wide(target: &mut [u16], text: &str) {
    let mut index = 0;
    for unit in text.encode_utf16() {
        if index + 1 >= target.len() {
            break;
        }
        target[index] = unit;
        index += 1;
    }
    target[index] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_id_maps_to_exactly_one_command() {
        assert_eq!(command_for(MENU_NEW), Some(TrayCommand::NewNote));
        assert_eq!(command_for(MENU_TODO_CARD), Some(TrayCommand::TodoCard));
        assert_eq!(command_for(MENU_CLOSE_ALL), Some(TrayCommand::CloseAll));
        assert_eq!(command_for(MENU_QUIT), Some(TrayCommand::Quit));
        assert_eq!(
            command_for(0),
            None,
            "id 0 is what a cancelled menu sends; it must not be a command"
        );
        assert_eq!(command_for(99), None);
    }

    #[test]
    fn a_truncated_ico_is_rejected_instead_of_read_out_of_bounds() {
        assert!(icon_from_ico(&[]).is_none());
        assert!(icon_from_ico(&[0, 0, 1, 0, 5, 0]).is_none(), "entry table missing");
    }

    #[test]
    fn the_tooltip_is_always_terminated() {
        let mut buffer = [0xFFFFu16; 128];
        copy_wide(&mut buffer, "易流笔记");
        assert_eq!(buffer[4], 0);
        assert_eq!(buffer[5], 0xFFFF, "only the used prefix and one terminator are touched");

        let mut tiny = [0xFFFFu16; 4];
        copy_wide(&mut tiny, "abcdef");
        assert_eq!(tiny[3], 0, "a tip longer than the buffer still ends in a NUL");
    }
}
