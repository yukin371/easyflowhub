//! Manager / quick-note / todo-card window behavior.
//!
//! Owns the creation, positioning, visibility toggling and close behavior of the
//! non-Launcher native windows. Launcher behavior lives in `launcher`;
//! cursor-anchored placement math lives in `window_position`.
//!
//! Lifecycle rule (plan `2026-11-24-window-lifecycle-and-latency-contract.md`
//! Batch B): the Manager window is **created on demand and destroyed on close**.
//! It is not declared in `tauri.conf.json` any more, because a pre-created
//! window pays for a whole WebView2 renderer while nobody is using the
//! workbench — that was the dominant part of the 600 MB idle footprint. The
//! Launcher keeps its pre-created window: it is the only surface with a
//! 60 ms hotkey-to-first-frame budget, so lazy creation there would trade the
//! primary gate for memory (explicitly forbidden in the design, §5.8.1).
//!
//! Closing the Manager is therefore plain Tauri close → destroy; there is no
//! startup `CloseRequested` handler turning it into a hide, because that only
//! made sense while the window always existed.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::Manager;

use crate::launcher::dismiss_launcher;
use crate::settings;

// 快速笔记窗口计数器
static NOTE_WINDOW_COUNTER: AtomicU32 = AtomicU32::new(1);
// Todo 卡片窗口计数器
static TODO_CARD_COUNTER: AtomicU32 = AtomicU32::new(1);
// 防止快捷键重复触发
static LAST_TOGGLE_TIME: Mutex<i64> = Mutex::new(0);

#[tauri::command]
pub async fn toggle_always_on_top(window: tauri::WebviewWindow) -> Result<bool, String> {
    let is_on_top = window.is_always_on_top().map_err(|e: tauri::Error| e.to_string())?;
    window.set_always_on_top(!is_on_top).map_err(|e: tauri::Error| e.to_string())?;
    Ok(!is_on_top)
}

#[tauri::command]
pub async fn hide_window(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "launcher" {
        return dismiss_launcher(&window);
    }
    window.hide().map_err(|e: tauri::Error| e.to_string())
}

#[tauri::command]
pub async fn close_window(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "launcher" {
        return dismiss_launcher(&window);
    }
    window.destroy().map_err(|e: tauri::Error| e.to_string())
}

/// 创建快速笔记窗口
///
/// T-Gate 0 之后本进程不再创建这个 webview；T-Gate 1 起这条命令改为**转给原生壳**，
/// 而不是拒绝：Manager 里那个按钮是真的能用的，只是窗口属于另一个进程。原生壳的
/// 单实例互斥会把这次启动变成一条转发请求，所以壳已经常驻时也不会多出一个进程。
#[tauri::command]
pub async fn create_note_window(app: tauri::AppHandle, note_id: Option<String>) -> Result<String, String> {
    if crate::shell::current().takeover() {
        return crate::native_shell::open_note(note_id.as_deref());
    }
    let counter = NOTE_WINDOW_COUNTER.fetch_add(1, Ordering::SeqCst);
    let label = format!("quick-note-{}", counter);

    // 快速笔记窗口 URL，带参数标识
    let url = match note_id {
        Some(id) => format!("/?mode=quick&note_id={}", id),
        None => "/?mode=quick".to_string(),
    };

    // 从设置中读取窗口大小，如果读取失败则使用默认值
    let (width, height) = match settings::get_settings(app.clone()) {
        Ok(response) => {
            let s = response.settings;
            (s.quick_note.width as f64, s.quick_note.height as f64)
        }
        Err(_) => (400.0, 300.0), // 默认值
    };

    let builder = tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App(url.into())
    )
    .title("EasyFlowHub - 快速笔记")
    .inner_size(width, height)
    .min_inner_size(150.0, 60.0)
    .transparent(true)
    .always_on_top(true)
    .decorations(false)
    .skip_taskbar(false)
    // Builder positions are logical pixels; apply the physical cursor position
    // after creation, before showing the window, to avoid a wrong-monitor flash.
    .visible(false)
    .center();

    let window = builder
    .build()
    .map_err(|e: tauri::Error| e.to_string())?;

    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e: tauri::Error| e.to_string())?;

    let logical_size = window.outer_size().ok().and_then(|size| {
        window.scale_factor().ok().map(|scale| size.to_logical::<f64>(scale))
    });
    let (actual_width, actual_height) = logical_size
        .map(|size| (size.width, size.height))
        .unwrap_or((width, height));
    if let Some(position) = crate::window_position::cursor_anchored_position(&app, actual_width, actual_height) {
        window
            .set_position(tauri::Position::Physical(position))
            .map_err(|e: tauri::Error| e.to_string())?;
    }
    window.show().map_err(|e: tauri::Error| e.to_string())?;
    window.set_focus().map_err(|e: tauri::Error| e.to_string())?;

    Ok(label)
}

/// 创建 Todo 卡片窗口（桌面悬浮）
///
/// 与快速笔记同理：接管后待办卡片归原生壳（Phase 1 的待办卡片），本进程
/// 不再为它拉起第三个 WebView。
#[tauri::command]
pub async fn create_todo_card_window(app: tauri::AppHandle) -> Result<String, String> {
    if crate::shell::current().takeover() {
        return Err(crate::shell::moved_to_native("待办卡片"));
    }
    let counter = TODO_CARD_COUNTER.fetch_add(1, Ordering::SeqCst);
    let label = format!("todo-card-{}", counter);

    let url = "/?mode=todo-card";

    let _window = tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App(url.into())
    )
    .title("EasyFlowHub - 待办卡片")
    .inner_size(320.0, 400.0)
    .min_inner_size(200.0, 150.0)
    .transparent(true)
    .always_on_top(true)
    .decorations(false)
    .skip_taskbar(true)
    .center()
    .build()
    .map_err(|e: tauri::Error| e.to_string())?;

    Ok(label)
}

/// 切换 Todo 卡片的桌面固定状态
#[tauri::command]
pub async fn toggle_todo_card_pin(app: tauri::AppHandle, label: String, pinned: bool) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetWindowLongPtrW, GWLP_HWNDPARENT};
        use windows::core::HSTRING;
        use tauri::Manager;

        if let Some(win) = app.get_webview_window(&label) {
            // Use raw-window-handle to get the Win32 HWND
            use raw_window_handle::HasWindowHandle;
            let handle = win.window_handle().map_err(|e: raw_window_handle::HandleError| e.to_string())?;
            let raw = handle.as_raw();
            if let raw_window_handle::RawWindowHandle::Win32(win32_handle) = raw {
                let hwnd = win32_handle.hwnd.get() as *mut core::ffi::c_void;
                unsafe {
                    if pinned {
                        if let Ok(progman) = FindWindowW(&HSTRING::from("Progman"), None) {
                            let _ = SetWindowLongPtrW(
                                windows::Win32::Foundation::HWND(hwnd),
                                GWLP_HWNDPARENT,
                                progman.0 as isize,
                            );
                        }
                    } else {
                        let _ = SetWindowLongPtrW(
                            windows::Win32::Foundation::HWND(hwnd),
                            GWLP_HWNDPARENT,
                            0,
                        );
                    }
                }
                return Ok(pinned);
            }
            return Err("Not a Win32 window".to_string());
        }
        Err("Window not found".to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, label, pinned);
        Ok(false)
    }
}

/// Show the Manager window, creating it when it does not exist yet.
///
/// Keeping the "create" step here (instead of in `tauri.conf.json`) is what
/// makes the idle footprint independent of the workbench: nothing is allocated
/// until the user actually opens it. Window geometry is restored by
/// `tauri-plugin-window-state`, which applies the saved state on creation.
pub(crate) fn ensure_manager_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("manager") {
        return Ok(window);
    }

    tauri::WebviewWindowBuilder::new(app, "manager", tauri::WebviewUrl::App("/".into()))
        .title("EasyFlowHub")
        .inner_size(1000.0, 700.0)
        .min_inner_size(600.0, 400.0)
        .center()
        .build()
        .map_err(|error| format!("Failed to create manager window: {error}"))
}

/// Create-if-needed then reveal + focus. Every entry point (tray, hotkey,
/// single-instance callback, frontend command) goes through this so no path can
/// hold a stale "window not found" assumption.
///
/// Must NOT be called from the event-loop thread: building a WebView blocks the
/// calling thread until WebView2 finishes, and WebView2 needs that same loop to
/// make progress — calling it from a `run_on_main_thread` callback deadlocks the
/// whole app (measured 2026-11-24: the second instance hung, the Manager stayed
/// an `about:blank` shell). Main-thread callers use [`request_manager_window`].
pub(crate) fn reveal_manager_window(app: &tauri::AppHandle) {
    match ensure_manager_window(app) {
        Ok(window) => {
            let _ = window.show();
            let _ = window.set_focus();
        }
        Err(error) => crate::logging::log("warn", "window", &error),
    }
}

/// Event-loop-safe variant for tray / single-instance callbacks: hop off the
/// main thread first, then create.
pub(crate) fn request_manager_window(app: &tauri::AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        reveal_manager_window(&handle);
    });
}

/// 显示管理窗口
#[tauri::command]
pub async fn show_manager_window(app: tauri::AppHandle) -> Result<(), String> {
    reveal_manager_window(&app);
    Ok(())
}

/// 隐藏管理窗口
#[tauri::command]
pub async fn hide_manager_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("manager") {
        window.hide().map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

/// 关闭所有快速笔记窗口
#[tauri::command]
pub async fn close_all_note_windows(app: tauri::AppHandle) -> Result<(), String> {
    // 获取所有窗口
    let windows = app.webview_windows();
    for (label, window) in windows {
        // 只关闭以 "quick-note-" 开头的窗口。`destroy()` 而不是 `close()`：
        // 关闭走的是同一套语义，且不会被将来某个 CloseRequested handler 拦成隐藏。
        if label.starts_with("quick-note-") {
            let _ = window.destroy();
        }
    }
    Ok(())
}

/// 显示或隐藏所有快速笔记窗口
#[tauri::command]
pub async fn toggle_note_windows_visibility(app: tauri::AppHandle) -> Result<bool, String> {
    let windows = app.webview_windows();
    let note_windows: Vec<_> = windows
        .into_iter()
        .filter(|(label, _)| label.starts_with("quick-note-"))
        .collect();

    if note_windows.is_empty() {
        return Ok(false);
    }

    let any_active = note_windows.iter().any(|(_, window)| {
        window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false)
    });

    if any_active {
        for (_, window) in note_windows {
            let _ = window.minimize();
        }
        Ok(false)
    } else {
        for (_, window) in note_windows.iter() {
            let _ = window.unminimize();
            let _ = window.show();
        }
        if let Some((_, first_window)) = note_windows.first() {
            let _ = first_window.set_focus();
        }
        Ok(true)
    }
}

/// Ctrl+Alt+M: debounced show / hide of the Manager window.
pub(crate) fn toggle_manager_window(app: &tauri::AppHandle) {
    // 防抖
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    if let Ok(mut last) = LAST_TOGGLE_TIME.lock() {
        if now - *last < 300 {
            return;
        }
        *last = now;
    }

    if let Some(window) = app.get_webview_window("manager") {
        if let Ok(visible) = window.is_visible() {
            if visible {
                let _ = window.hide();
            } else {
                let _ = window.show();
                let _ = window.set_focus();
            }
            return;
        }
    }
    // First press after a close: the window was destroyed, so build it.
    reveal_manager_window(app);
}
