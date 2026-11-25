// Standalone key poker for the spike window: finds the top-level window whose
// title matches *and* whose owning process is the given PID (two instances can
// run side by side), posts an Esc to its RichEdit child, and reports whether the
// window moved off-screen.
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

type Bool = i32;

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(cb: Option<unsafe extern "system" fn(isize, isize) -> Bool>, lp: isize) -> Bool;
    fn GetWindowTextW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
    fn GetWindowTextLengthW(hwnd: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    fn FindWindowExW(parent: isize, after: isize, class: *const u16, title: *const u16) -> isize;
    fn PostMessageW(hwnd: isize, msg: u32, wp: usize, lp: isize) -> i32;
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
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
    let before = rect_of(target.hwnd);
    println!(
        "target={} child={} rect_before=+{}+{} off_screen_before={}",
        target.hwnd,
        child,
        before.left,
        before.top,
        before.left < -10_000
    );
    if child == 0 {
        println!("result=NO_EDITOR_CHILD");
        std::process::exit(1);
    }

    unsafe {
        PostMessageW(child, 0x0100, 0x1B, 1); // WM_KEYDOWN VK_ESCAPE
        Sleep(20);
        PostMessageW(child, 0x0101, 0x1B, 0xC000_0001u32 as isize); // WM_KEYUP
    }

    unsafe { Sleep(500) };
    let after = rect_of(target.hwnd);
    println!(
        "rect_after=+{}+{} off_screen_after={}",
        after.left,
        after.top,
        after.left < -10_000
    );
}
