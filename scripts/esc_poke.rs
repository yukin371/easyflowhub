// Standalone key poker for the spike window: finds the top-level window whose
// title matches *and* whose owning process is the given PID (two instances can
// run side by side), posts an Esc to its RichEdit child, and reports three
// things: whether the window moved off-screen, which control owns keyboard focus
// afterwards, and whether the document length changed.
//
// The focus line is the point. Off-screen is not closed: the island used to keep
// keyboard focus after parking, so typing after Esc went into a window nobody
// could see. "Is the editor still the focus owner" is the only check that
// distinguishes closed from hidden, and it is not visible in a screenshot.
//
// No WM_CHAR is posted on purpose. PostMessage delivers straight to the window
// procedure and inserts text regardless of focus, so it would "prove" the bug
// exists even after a fix. Focus ownership is the honest signal; the length is
// reported only as context for what was on screen.
//
// Raw FFI on purpose: plain `rustc` can build this, and runtime codegen through
// Add-Type is not available in this environment.
//
//   esc_poke.exe <pid>

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// `GUITHREADINFO`. Field order is fixed by winuser.h; the handles are pointer
/// sized, so `usize` keeps this correct for 64-bit without a cfg branch.
#[repr(C)]
struct GuiThreadInfo {
    cb_size: u32,
    flags: u32,
    hwnd_active: isize,
    hwnd_focus: isize,
    hwnd_capture: isize,
    hwnd_menu_owner: isize,
    hwnd_move_size: isize,
    hwnd_caret: isize,
    rc_caret: Rect,
}

type Bool = i32;

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(cb: Option<unsafe extern "system" fn(isize, isize) -> Bool>, lp: isize) -> Bool;
    fn GetWindowTextW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
    fn GetWindowTextLengthW(hwnd: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    fn FindWindowExW(parent: isize, after: isize, class: *const u16, title: *const u16) -> isize;
    fn PostMessageW(hwnd: isize, msg: u32, wp: usize, lp: isize) -> i32;
    fn SendMessageW(hwnd: isize, msg: u32, wp: usize, lp: isize) -> isize;
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn GetForegroundWindow() -> isize;
    fn GetGUIThreadInfo(thread: u32, info: *mut GuiThreadInfo) -> Bool;
}

#[link(name = "kernel32")]
extern "system" {
    fn Sleep(ms: u32);
}

struct Match {
    hwnd: isize,
    title: String,
}

unsafe extern "system" fn collect(hwnd: isize, lp: isize) -> Bool {
    let want = &mut *(lp as *mut (u32, Vec<Match>));
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid != want.0 {
        return 1;
    }
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return 1;
    }
    let mut buf = vec![0u16; len as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    let title = String::from_utf16_lossy(&buf[..copied.max(0) as usize]);
    want.1.push(Match { hwnd, title });
    1
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn rect_of(hwnd: isize) -> Rect {
    let mut r = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetWindowRect(hwnd, &mut r) };
    r
}

/// Keyboard focus of the thread that owns `hwnd`.
///
/// `GetFocus` only answers for the calling thread, so a cross-process check has
/// to go through `GetGUIThreadInfo`. Returns `0` when the thread has no focus
/// owner -- which is exactly the state a parked panel should be in.
fn focus_of(hwnd: isize) -> isize {
    let thread = unsafe { GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) };
    let mut info = GuiThreadInfo {
        cb_size: std::mem::size_of::<GuiThreadInfo>() as u32,
        flags: 0,
        hwnd_active: 0,
        hwnd_focus: 0,
        hwnd_capture: 0,
        hwnd_menu_owner: 0,
        hwnd_move_size: 0,
        hwnd_caret: 0,
        rc_caret: Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
    };
    let ok = unsafe { GetGUIThreadInfo(thread, &mut info) };
    if ok == 0 {
        return -1;
    }
    info.hwnd_focus
}

/// Document length in characters (`WM_GETTEXTLENGTH` works cross-process).
fn text_len(hwnd: isize) -> isize {
    unsafe { SendMessageW(hwnd, 0x000E, 0, 0) }
}

fn main() {
    let pid: u32 = match std::env::args().nth(1).and_then(|a| a.parse().ok()) {
        Some(p) => p,
        None => {
            println!("usage: esc_poke <pid>");
            std::process::exit(2);
        }
    };

    let mut pass: (u32, Vec<Match>) = (pid, Vec::new());
    unsafe { EnumWindows(Some(collect), &mut pass as *mut _ as isize) };
    let windows = pass.1;
    for w in &windows {
        println!("window pid={pid} hwnd={} title={:?}", w.hwnd, w.title);
    }
    let Some(target) = windows
        .iter()
        .find(|w| w.title == "easyflowhub-native-spike")
    else {
        println!("result=NO_WINDOW");
        std::process::exit(1);
    };

    let class = wide("RICHEDIT50W");
    let child = unsafe { FindWindowExW(target.hwnd, 0, class.as_ptr(), std::ptr::null()) };
    if child == 0 {
        println!("result=NO_EDITOR_CHILD");
        std::process::exit(1);
    }

    let before = rect_of(target.hwnd);
    let focus_before = focus_of(target.hwnd);
    let len_before = text_len(child);
    println!(
        "target={} child={} rect_before=+{}+{} off_screen_before={}",
        target.hwnd,
        child,
        before.left,
        before.top,
        before.left < -10_000
    );
    println!(
        "focus_before={} focus_is_editor_before={} chars_before={}",
        focus_before,
        focus_before == child,
        len_before
    );

    unsafe {
        PostMessageW(child, 0x0100, 0x1B, 1); // WM_KEYDOWN VK_ESCAPE
        Sleep(20);
        PostMessageW(child, 0x0101, 0x1B, 0xC000_0001u32 as isize); // WM_KEYUP
    }

    unsafe { Sleep(500) };
    let after = rect_of(target.hwnd);
    let focus_after = focus_of(target.hwnd);
    let len_after = text_len(child);
    let foreground = unsafe { GetForegroundWindow() };
    let off_screen = after.left < -10_000;
    let focus_is_editor = focus_after == child;

    println!(
        "rect_after=+{}+{} off_screen_after={}",
        after.left, after.top, off_screen
    );
    println!(
        "focus_after={} focus_is_editor_after={} chars_after={} foreground={} foreground_is_target={}",
        focus_after,
        focus_is_editor,
        len_after,
        foreground,
        foreground == target.hwnd
    );

    // Parked *and* not focusable is the definition of closed. Either half alone
    // is the old bug: off-screen with focus writes into nothing, and focus
    // released without moving leaves a panel on screen.
    let ok = off_screen && !focus_is_editor;
    println!(
        "result={}",
        if ok {
            "PASS"
        } else if !off_screen {
            "FAIL_STILL_ON_SCREEN"
        } else {
            "FAIL_FOCUS_LEFT_IN_EDITOR"
        }
    );
    std::process::exit(if ok { 0 } else { 1 });
}
