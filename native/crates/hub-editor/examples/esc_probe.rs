//! Throwaway diagnostic: does Esc reach the callback once per press, and where
//! does a parked control still send the keystrokes that follow?
//!
//! The report was "Esc had to be pressed three times before the panel closed".
//! Two very different faults produce that symptom:
//!
//!   1. delivery: the press never reaches the subclass proc (focus is on the
//!      parent, or the IME ate it because a candidate window was open);
//!   2. polling: the press arrives, the flag is set, but nothing ever asks
//!      egui to run a frame, so the flag sits unread until an unrelated mouse
//!      message happens to wake the loop.
//!
//! This probe settles (1) by sending N presses synchronously and counting the
//! callbacks. `SendMessageW` runs the window proc on the caller's thread, so the
//! count here is exactly what the control delivers -- a smaller number in the
//! real app cannot be blamed on the control.
//!
//! It also checks what happens *after* a park, because that is the other half of
//! the same code path: the panel is moved off-screen but stays visible and
//! foreground, so the question is whether the next keystroke lands in the
//! invisible editor.
//!
//! Run with: cargo run -p hub-editor --example esc_probe

use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use hub_core::island::PixelRect;
use hub_editor::{Editor, Theme};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, VK_ESCAPE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, RegisterClassW, SetWindowPos, MSG,
    SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
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
        let class = wide("EscProbeHost");
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

        let theme = Theme {
            background: 0x0021_252D,
            foreground: 0x00E8_EEF7,
            font_face: "Microsoft YaHei UI",
            font_pt: 16.5,
        };
        let rect = PixelRect {
            x: 0,
            y: 0,
            width: 600,
            height: 1100,
        };
        let mut editor = Editor::create(parent as isize, rect, &theme).expect("create editor");

        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        editor.on_escape(Box::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }));

        editor.focus();
        println!("focus_is_editor={}", GetFocus() as isize == editor.hwnd());

        // 1. Delivery: one callback per synchronous press, whatever the count.
        for n in 1..=3 {
            let result = editor.key_down(VK_ESCAPE);
            println!(
                "press {n}: lresult={result} callbacks={}",
                calls.load(Ordering::SeqCst)
            );
        }
        println!(
            "delivery verdict: {}",
            if calls.load(Ordering::SeqCst) == 3 {
                "1:1 -- the control is not dropping presses"
            } else {
                "LOST PRESSES inside the control"
            }
        );

        // 2. Parked control: moved off-screen exactly like `window::park`, still
        //    visible and still holding focus. Anything typed now goes here.
        SetWindowPos(
            parent,
            ptr::null_mut(),
            -30_000,
            -30_000,
            0,
            0,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE,
        );
        editor.set_text("");
        editor.caret_to_end();
        println!("parked: focus_is_editor={}", GetFocus() as isize == editor.hwnd());
        let typed = editor.key_press(0x41);
        println!(
            "parked: typed='{typed}' chars={} first_visible={}",
            editor.char_count(),
            editor.first_visible_line()
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
