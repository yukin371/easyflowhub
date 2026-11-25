//! Throwaway diagnostic: which scroll path works on a *parked* (hidden) control?
//!
//! Three candidates, in increasing order of intrusiveness:
//!   1. `EM_SCROLLCARET` (what "caret follows typing" uses)
//!   2. `EM_LINESCROLL` (explicit scroll, no caret involvement)
//!   3. `EM_SCROLLCARET` again, after making the window visible
//!
//! Run with: cargo run -p hub-editor --example scroll_probe

use std::ptr;

use hub_core::island::PixelRect;
use hub_editor::{Editor, Theme};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, RegisterClassW, MSG, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("ScrollProbeHost");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(proc_),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);

        // Not WS_VISIBLE: this is the parked case the bench runs in.
        let parent = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("host").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            100,
            100,
            640,
            1200,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );

        let mut client = std::mem::zeroed();
        GetClientRect(parent, &mut client);
        println!(
            "host client={}x{}",
            client.right - client.left,
            client.bottom - client.top
        );

        let theme = Theme {
            background: 0x0021_252D,
            foreground: 0x00E8_EEF7,
            font_face: "Microsoft YaHei UI".to_string(),
            font_pt: 16.5,
            line_spacing: 1.0,
            padding: (0, 0),
            caret_width: 0,
        };
        let rect = PixelRect {
            x: 0,
            y: 0,
            width: 600,
            height: 1100,
        };
        let editor = Editor::create(parent as isize, rect, &theme).expect("create editor");

        let text: String = "天地玄黄宇宙洪荒日月盈昃辰宿列张寒来暑往秋收冬藏"
            .chars()
            .cycle()
            .take(10_000)
            .collect();
        editor.set_text(&text);

        println!("chars={}", editor.char_count());
        println!("lines={}", editor.line_count());
        println!("[hidden] after set_text first_visible={}", editor.first_visible_line());

        editor.caret_to_end();
        println!("[hidden] caret={:?}", editor.caret());
        editor.scroll_caret();
        println!(
            "[hidden] EM_SCROLLCARET  first_visible={} pos={:?}",
            editor.first_visible_line(),
            editor.scroll_pos()
        );

        editor.lines_scroll(100);
        println!(
            "[hidden] EM_LINESCROLL(l-100 top) first_visible={} pos={:?}",
            editor.first_visible_line(),
            editor.scroll_pos()
        );

        // Now the same path after giving the control focus. NO ShowWindow here:
        // launching a visible window is not something this diagnostic needs, and
        // whether the control is on screen is reported instead of forced.
        editor.focus();
        editor.caret_to_end();
        editor.scroll_caret();
        println!(
            "[focus]  EM_SCROLLCARET  first_visible={} pos={:?}",
            editor.first_visible_line(),
            editor.scroll_pos()
        );

        editor.set_text(&text);
        editor.caret_to_end();
        editor.scroll_caret();
        println!(
            "[focus]  after reload   first_visible={} pos={:?}",
            editor.first_visible_line(),
            editor.scroll_pos()
        );

        let mut message: MSG = std::mem::zeroed();
        while windows_sys::Win32::UI::WindowsAndMessaging::PeekMessageW(
            &mut message,
            ptr::null_mut(),
            0,
            0,
            1,
        ) != 0
        {}
    }
}
