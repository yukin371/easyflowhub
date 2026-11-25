//! Throwaway diagnostic: where does RichEdit actually break undo units?
//!
//! B5 wants "50 edits = 50 undo steps". Before recording that as failing, find out
//! what the control does group, so the record says "this is the real behaviour"
//! instead of "our number is small".
//!
//! Run with: cargo build -p hub-editor --example undo_probe && ./target/debug/examples/undo_probe.exe

use std::ptr;

use hub_core::island::PixelRect;
use hub_editor::{Editor, Theme};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, RegisterClassW, MSG, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn units_after(editor: &mut Editor, build: impl Fn(&mut Editor)) -> (usize, String, String, bool) {
    editor.set_text("起点");
    editor.clear_undo();
    editor.caret_to_end();
    build(editor);
    let before = editor.text();

    let mut steps = 0;
    while steps < 200 && editor.undo(1) {
        steps += 1;
    }
    let after = editor.text();
    let restored = after == "起点";
    (steps, before, after, restored)
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("UndoProbeHost");
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
        editor.focus();

        let corpus: Vec<char> = "天地玄黄宇宙洪荒日月盈昃辰宿列张".chars().collect();

        macro_rules! report {
            ($label:expr, $build:expr) => {{
                let (units, before, after, restored) = units_after(&mut editor, $build);
                println!(
                    "{:<26} units={units} restored={restored} edited={before} after_undo={after}",
                    $label
                );
            }};
        }

        // 1. One uninterrupted typing run.
        report!("typing only", |e: &mut Editor| {
            for ch in corpus.iter().take(10) {
                e.append_char(*ch);
            }
        });

        // 2. Same, with a forced group boundary in the middle.
        report!("typing + STOPGROUPTYPING", |e: &mut Editor| {
            for ch in corpus.iter().take(5) {
                e.append_char(*ch);
            }
            e.stop_group_typing();
            for ch in corpus.iter().skip(5).take(5) {
                e.append_char(*ch);
            }
        });

        // 3. Typing, then a deletion, then more typing.
        report!("typing + 2x backspace", |e: &mut Editor| {
            for ch in corpus.iter().take(5) {
                e.append_char(*ch);
            }
            e.append_char('\u{8}');
            e.append_char('\u{8}');
            for ch in corpus.iter().skip(5).take(5) {
                e.append_char(*ch);
            }
        });

        // 4. Typing, then a replacement of a selection.
        report!("typing + replace-all", |e: &mut Editor| {
            for ch in corpus.iter().take(5) {
                e.append_char(*ch);
            }
            e.select_all();
            e.replace_selection("替换");
        });

        // 5. Typing, an explicit caret move, then typing. A caret move is what
        //    breaks a typing group in every editor a user has met.
        report!("typing + caret move", |e: &mut Editor| {
            for ch in corpus.iter().take(5) {
                e.append_char(*ch);
            }
            e.caret_to_end();
            e.select_all();
            e.caret_to_end();
            for ch in corpus.iter().skip(5).take(5) {
                e.append_char(*ch);
            }
        });

        // 6. Two separate paste-style insertions.
        report!("typing + 2x replace", |e: &mut Editor| {
            e.append_char('甲');
            e.select_all();
            e.replace_selection("乙");
            e.caret_to_end();
            e.select_all();
            e.caret_to_end();
            e.replace_selection("丙");
        });

        let (units, _, _, _) = units_after(&mut editor, |e| {
            for index in 0..60 {
                e.append_char(corpus[index % corpus.len()]);
            }
        });
        println!("{:<26} units={units}", "60 chars one run");

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
