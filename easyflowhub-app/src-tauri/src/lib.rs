use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, Position, Size, WebviewWindow,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

mod scriptmgr;
mod scriptmgr_server;
mod logging;
mod notes;
mod inbox;
mod appearance;
mod clipboard;
mod widget;
mod settings;
mod modules;
mod mcp_server;
mod external_apps;
mod window_position;
mod file_search;
mod download;
mod voice;
mod transcription;

// 快速笔记窗口计数器
static NOTE_WINDOW_COUNTER: AtomicU32 = AtomicU32::new(1);
// Todo 卡片窗口计数器
static TODO_CARD_COUNTER: AtomicU32 = AtomicU32::new(1);
// 防止快捷键重复触发
static LAST_TOGGLE_TIME: Mutex<i64> = Mutex::new(0);
// Only the launcher shortcut is reconfigurable at runtime. Other application
// shortcuts keep their existing registrations when the user changes it.
static LAUNCHER_SHORTCUT: Mutex<Option<Shortcut>> = Mutex::new(None);
// Launcher is a transient palette by default. The pin button is the only
// thing that opts it into staying visible above other applications.
static LAUNCHER_PINNED: AtomicBool = AtomicBool::new(false);

/// Foreground window that supplied the most recent selected-text context.
/// Stored as a raw HWND value only for the short-lived Launcher session; it is
/// never persisted and is cleared when no selection is captured.
#[cfg(target_os = "windows")]
static SELECTION_TARGET_WINDOW: Mutex<Option<isize>> = Mutex::new(None);

#[cfg(target_os = "windows")]
static MOUSE_TRIGGER_SENDER: OnceLock<Mutex<Option<mpsc::Sender<()>>>> = OnceLock::new();
#[cfg(target_os = "windows")]
static MOUSE_TRIGGER_CONFIG: OnceLock<Arc<Mutex<MouseTriggerConfig>>> = OnceLock::new();

#[cfg(target_os = "windows")]
#[derive(Default)]
struct MouseTriggerConfig {
    enabled: bool,
    excluded_apps: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LauncherSettingsInput {
    #[serde(default = "default_launcher_activation")]
    activation: String,
    #[serde(default = "default_launcher_shortcut")]
    keyboard_shortcut: String,
    #[serde(default)]
    excluded_apps: Vec<LauncherExcludedAppInput>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LauncherExcludedAppInput {
    executable: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LauncherOpenedPayload {
    selected_text: Option<String>,
}

fn default_launcher_activation() -> String {
    "keyboardOrMiddleMouse".to_string()
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn launcher_mouse_hook(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, WM_MBUTTONUP};

    if code >= 0 && wparam.0 as u32 == WM_MBUTTONUP {
        if let Some(sender) = MOUSE_TRIGGER_SENDER.get() {
            if let Ok(guard) = sender.lock() {
                if let Some(sender) = guard.as_ref() {
                    let _ = sender.send(());
                }
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

/// Ask the foreground application to copy its current selection and return the
/// resulting Unicode text. The original text clipboard value is restored when
/// available so merely invoking the Launcher does not unexpectedly replace a
/// user's clipboard. This is intentionally best effort: applications that do
/// not expose a selection simply produce `None`.
#[cfg(target_os = "windows")]
fn capture_selected_text() -> Option<String> {
    use clipboard_win::{empty, get_clipboard_string, seq_num, set_clipboard_string};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_C,
        VK_CONTROL,
    };

    let foreground = unsafe { GetForegroundWindow() };
    let previous_sequence = seq_num();
    let previous_text = get_clipboard_string().ok();
    let key = |vk, flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [
        key(VK_CONTROL, Default::default()),
        key(VK_C, Default::default()),
        key(VK_C, KEYEVENTF_KEYUP),
        key(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return None;
    }
    std::thread::sleep(std::time::Duration::from_millis(45));
    let changed = seq_num() != previous_sequence;
    let selected = if changed {
        get_clipboard_string().ok().map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    } else {
        None
    };

    // Restore a text clipboard when one was present before the synthetic copy.
    // If the clipboard held a non-text format we leave the new text in place;
    // preserving arbitrary formats requires a full format snapshot and is a
    // separate clipboard-management concern.
    if let Some(previous_text) = previous_text {
        let _ = set_clipboard_string(&previous_text);
    } else if !changed {
        let _ = empty();
    }
    if let Ok(mut target) = SELECTION_TARGET_WINDOW.lock() {
        *target = selected.as_ref().map(|_| foreground.0 as isize);
    }
    selected
}

#[cfg(not(target_os = "windows"))]
fn capture_selected_text() -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
fn replace_selected_text_native(text: String) -> Result<(), String> {
    use clipboard_win::{get_clipboard_string, set_clipboard_string};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
        VK_V,
    };
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow};

    let target = SELECTION_TARGET_WINDOW
        .lock()
        .map_err(|_| "无法读取原始选区窗口".to_string())?
        .take()
        .ok_or_else(|| "原始选区窗口已不可用，请重新选中文本后唤起 Launcher。".to_string())?;
    let target = windows::Win32::Foundation::HWND(target as *mut core::ffi::c_void);
    if !unsafe { IsWindow(target) }.as_bool() {
        return Err("原始选区窗口已关闭，请重新选中文本后重试。".to_string());
    }
    let previous_text = get_clipboard_string().ok();
    set_clipboard_string(&text).map_err(|_| "无法写入剪贴板，替换选区失败。".to_string())?;
    if !unsafe { SetForegroundWindow(target) }.as_bool() {
        if let Some(previous) = previous_text.as_deref() { let _ = set_clipboard_string(previous); }
        return Err("无法恢复原始窗口焦点，替换选区失败。".to_string());
    }
    std::thread::sleep(std::time::Duration::from_millis(60));
    let key = |vk, flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    };
    let inputs = [key(VK_CONTROL, Default::default()), key(VK_V, Default::default()), key(VK_V, KEYEVENTF_KEYUP), key(VK_CONTROL, KEYEVENTF_KEYUP)];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    std::thread::sleep(std::time::Duration::from_millis(60));
    if let Some(previous) = previous_text.as_deref() { let _ = set_clipboard_string(previous); }
    if sent == inputs.len() as u32 { Ok(()) } else { Err("系统未接受粘贴输入，替换选区失败。".to_string()) }
}

#[cfg(not(target_os = "windows"))]
fn replace_selected_text_native(_text: String) -> Result<(), String> {
    Err("当前平台不支持替换选中文本。".to_string())
}

/// Replace the selection captured immediately before the Launcher opened.
/// The command is deliberately explicit: invoking a translation action never
/// mutates the source application unless the user presses this result action.
#[tauri::command]
async fn replace_selected_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("launcher") {
        dismiss_launcher(&window)?;
    }
    replace_selected_text_native(text)
}

#[cfg(target_os = "windows")]
fn configure_mouse_trigger(app: &tauri::AppHandle, enabled: bool, excluded_apps: Vec<String>) {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
        WH_MOUSE_LL,
    };

    let config = MOUSE_TRIGGER_CONFIG
        .get_or_init(|| Arc::new(Mutex::new(MouseTriggerConfig::default())))
        .clone();
    if let Ok(mut value) = config.lock() {
        value.enabled = enabled;
        value.excluded_apps = excluded_apps;
    }

    if let Some(sender) = MOUSE_TRIGGER_SENDER.get() {
        if sender.lock().map(|value| value.is_some()).unwrap_or(false) {
            return;
        }
    }

    let (sender, receiver) = mpsc::channel::<()>();
    let _ = MOUSE_TRIGGER_SENDER.set(Mutex::new(Some(sender)));
    let app_handle = app.clone();

    std::thread::spawn(move || unsafe {
        let hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(launcher_mouse_hook), None, 0) {
            Ok(hook) => hook,
            Err(error) => {
                eprintln!("Failed to install Launcher mouse hook: {error}");
                if let Some(sender) = MOUSE_TRIGGER_SENDER.get() {
                    if let Ok(mut value) = sender.lock() {
                        *value = None;
                    }
                }
                return;
            }
        };

        let mut message = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            while receiver.try_recv().is_ok() {
                let should_open = config.lock().map(|value| value.enabled).unwrap_or(false);
                if !should_open {
                    continue;
                }
                let excluded = config
                    .lock()
                    .map(|value| value.excluded_apps.clone())
                    .unwrap_or_default();
                if settings::is_foreground_app_excluded(&excluded) {
                    continue;
                }
                let selected_text = capture_selected_text();
                let _ = toggle_launcher_with_selection(&app_handle, selected_text);
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
    });
}

#[cfg(not(target_os = "windows"))]
fn configure_mouse_trigger(_app: &tauri::AppHandle, _enabled: bool, _excluded_apps: Vec<String>) {}

fn default_launcher_shortcut() -> String {
    "Ctrl+Alt+Space".to_string()
}

impl Default for LauncherSettingsInput {
    fn default() -> Self {
        Self {
            activation: default_launcher_activation(),
            keyboard_shortcut: default_launcher_shortcut(),
            excluded_apps: Vec::new(),
        }
    }
}
#[tauri::command]
async fn toggle_always_on_top(window: WebviewWindow) -> Result<bool, String> {
    let is_on_top = window.is_always_on_top().map_err(|e: tauri::Error| e.to_string())?;
    window.set_always_on_top(!is_on_top).map_err(|e: tauri::Error| e.to_string())?;
    Ok(!is_on_top)
}

#[tauri::command]
async fn hide_window(window: WebviewWindow) -> Result<(), String> {
    if window.label() == "launcher" {
        return dismiss_launcher(&window);
    }
    window.hide().map_err(|e: tauri::Error| e.to_string())
}

#[tauri::command]
async fn close_window(window: WebviewWindow) -> Result<(), String> {
    if window.label() == "launcher" {
        return dismiss_launcher(&window);
    }
    window.destroy().map_err(|e: tauri::Error| e.to_string())
}

fn compute_quick_note_position(
    app: &tauri::AppHandle,
    width: f64,
    height: f64,
) -> Option<PhysicalPosition<i32>> {
    let anchor_window = app
        .get_webview_window("manager")
        .or_else(|| app.webview_windows().into_values().next())?;

    let cursor = anchor_window.cursor_position().ok()?;
    let monitor = anchor_window
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| anchor_window.primary_monitor().ok().flatten())?;

    let work_area = monitor.work_area();
    let (x, y) = window_position::centered_position(
        (cursor.x, cursor.y),
        (width, height),
        monitor.scale_factor(),
        (
            work_area.position.x,
            work_area.position.y,
            work_area.size.width,
            work_area.size.height,
        ),
    );
    Some(PhysicalPosition::new(x, y))
}

/// Center the launcher at the pointer, using its current size and the pointer's monitor DPI.
fn position_launcher_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("launcher") else { return; };
    let logical_size = window.outer_size().ok().and_then(|size| {
        window.scale_factor().ok().map(|scale| size.to_logical::<f64>(scale))
    });
    let (width, height) = logical_size
        .map(|size| (size.width, size.height))
        .unwrap_or((564.0, 320.0));
    let Some(position) = compute_quick_note_position(app, width, height) else { return; };
    let _ = window.set_position(Position::Physical(position));
}

/// 创建快速笔记窗口
#[tauri::command]
async fn create_note_window(app: tauri::AppHandle, note_id: Option<String>) -> Result<String, String> {
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
        .set_size(Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e: tauri::Error| e.to_string())?;

    let logical_size = window.outer_size().ok().and_then(|size| {
        window.scale_factor().ok().map(|scale| size.to_logical::<f64>(scale))
    });
    let (actual_width, actual_height) = logical_size
        .map(|size| (size.width, size.height))
        .unwrap_or((width, height));
    if let Some(position) = compute_quick_note_position(&app, actual_width, actual_height) {
        window
            .set_position(Position::Physical(position))
            .map_err(|e: tauri::Error| e.to_string())?;
    }
    window.show().map_err(|e: tauri::Error| e.to_string())?;
    window.set_focus().map_err(|e: tauri::Error| e.to_string())?;

    Ok(label)
}

/// 创建 Todo 卡片窗口（桌面悬浮）
#[tauri::command]
async fn create_todo_card_window(app: tauri::AppHandle) -> Result<String, String> {
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
async fn toggle_todo_card_pin(app: tauri::AppHandle, label: String, pinned: bool) -> Result<bool, String> {
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

/// 显示管理窗口
#[tauri::command]
async fn show_manager_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("manager") {
        window.show().map_err(|e: tauri::Error| e.to_string())?;
        window.set_focus().map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

/// 隐藏管理窗口
#[tauri::command]
async fn hide_manager_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("manager") {
        window.hide().map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

/// Show the action launcher and focus its search field.
#[tauri::command]
async fn show_launcher_window(app: tauri::AppHandle) -> Result<(), String> {
    reveal_launcher(&app, None)
}

fn reveal_launcher(app: &tauri::AppHandle, selected_text: Option<String>) -> Result<(), String> {
    let window = app
        .get_webview_window("launcher")
        .ok_or_else(|| "Launcher window not found".to_string())?;
    position_launcher_window(app);
    window
        .set_always_on_top(LAUNCHER_PINNED.load(Ordering::Relaxed))
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    window
        .emit("launcher-opened", LauncherOpenedPayload { selected_text })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Repeating the configured mouse/keyboard trigger dismisses a focused panel.
/// If another app owns focus, bring the panel back to the pointer instead.
fn toggle_launcher_with_selection(
    app: &tauri::AppHandle,
    selected_text: Option<String>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("launcher")
        .ok_or_else(|| "Launcher window not found".to_string())?;
    if window.is_visible().map_err(|e| e.to_string())?
        && window.is_focused().map_err(|e| e.to_string())?
    {
        return dismiss_launcher(&window);
    }
    reveal_launcher(app, selected_text)
}

/// Every Launcher dismissal must notify its persistent webview so it can
/// cancel work started during the current visible session.
fn dismiss_launcher(window: &WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())?;
    window.emit("launcher-hidden", ()).map_err(|e| e.to_string())
}

/// Hide the launcher without destroying its webview so the next trigger opens
/// it immediately and keeps the user's current panel state.
#[tauri::command]
async fn hide_launcher_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("launcher") {
        dismiss_launcher(&window)?;
    }
    Ok(())
}

/// Start a native drag for the frameless launcher window. The frontend calls
/// this directly from the header's primary mouse event; the data attribute is
/// retained as a fallback for Tauri's injected drag-region handler.
#[tauri::command]
async fn start_launcher_dragging(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("launcher")
        .ok_or_else(|| "Launcher window not found".to_string())?;
    window
        .start_dragging()
        .map_err(|e: tauri::Error| e.to_string())
}

/// Set the launcher window's pin state without affecting the Manager window.
#[tauri::command]
async fn set_launcher_always_on_top(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    let window = app
        .get_webview_window("launcher")
        .ok_or_else(|| "Launcher window not found".to_string())?;
    window
        .set_always_on_top(enabled)
        .map_err(|e: tauri::Error| e.to_string())?;
    LAUNCHER_PINNED.store(enabled, Ordering::Relaxed);
    Ok(enabled)
}

#[tauri::command]
fn get_launcher_always_on_top(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app
        .get_webview_window("launcher")
        .ok_or_else(|| "Launcher window not found".to_string())?;
    window.is_always_on_top().map_err(|e| e.to_string())
}

/// Register the launcher trigger from persisted settings.
///
/// The global-shortcut plugin owns keyboard registration and reports conflicts
/// from the operating system. Mouse middle-button activation uses the native
/// Windows low-level hook and shares the same foreground exclusion policy.
fn register_launcher_trigger(
    app: &tauri::AppHandle,
    config: &settings::LauncherTriggerSettings,
) -> Result<(), String> {
    let previous = LAUNCHER_SHORTCUT.lock().unwrap().clone();

    if !config.enabled {
        if let Some(previous) = previous {
            app.global_shortcut()
                .unregister(previous)
                .map_err(|e| format!("Failed to unregister launcher shortcut: {e}"))?;
            *LAUNCHER_SHORTCUT.lock().unwrap() = None;
        }
        configure_mouse_trigger(app, false, config.excluded_apps.clone());
        return Ok(());
    }

    let mouse_enabled = config.trigger == "mouse_middle" || config.trigger == "keyboard_or_mouse_middle";
    let keyboard_enabled = config.trigger == "keyboard" || config.trigger == "keyboard_or_mouse_middle";
    if !keyboard_enabled && !mouse_enabled {
        return Err(format!("不支持的 Launcher 触发方式: {}", config.trigger));
    }

    let shortcut = if keyboard_enabled {
        Some(
            config
                .keyboard_shortcut
                .parse::<Shortcut>()
                .map_err(|e| format!("快捷键格式无效 '{}': {e}", config.keyboard_shortcut))?,
        )
    } else {
        None
    };

    if let Some(shortcut) = shortcut {
        let app_handle = app.clone();
        let excluded_apps = config.excluded_apps.clone();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed
                    || settings::is_foreground_app_excluded(&excluded_apps)
                {
                    return;
                }
                let selected_text = capture_selected_text();
                let _ = toggle_launcher_with_selection(&app_handle, selected_text);
            })
            .map_err(|e| {
                format!(
                    "无法注册 Launcher 快捷键 '{}': {e}。该组合可能已被其他软件占用。",
                    config.keyboard_shortcut
                )
            })?;

        if let Some(previous) = previous {
            if let Err(error) = app.global_shortcut().unregister(previous.clone()) {
                let _ = app.global_shortcut().unregister(shortcut.clone());
                return Err(format!("Failed to replace launcher shortcut: {error}"));
            }
        }
        *LAUNCHER_SHORTCUT.lock().unwrap() = Some(shortcut);
    } else if let Some(previous) = previous {
        app.global_shortcut()
            .unregister(previous)
            .map_err(|e| format!("Failed to unregister launcher shortcut: {e}"))?;
        *LAUNCHER_SHORTCUT.lock().unwrap() = None;
    }

    configure_mouse_trigger(app, mouse_enabled, config.excluded_apps.clone());
    Ok(())
}

/// Re-apply launcher trigger settings without restarting the application.
/// Persist settings separately through `update_settings` so this command can
/// also be used for a conflict check before committing a new value.
#[tauri::command]
fn apply_launcher_trigger_settings(
    app: tauri::AppHandle,
    config: LauncherSettingsInput,
) -> Result<LauncherSettingsInput, String> {
    let native_config = settings::LauncherTriggerSettings {
        enabled: true,
        trigger: match config.activation.as_str() {
            "keyboard" => "keyboard".to_string(),
            "middleMouse" => "mouse_middle".to_string(),
            "keyboardOrMiddleMouse" => "keyboard_or_mouse_middle".to_string(),
            other => return Err(format!("不支持的 Launcher 触发方式: {other}")),
        },
        keyboard_shortcut: config.keyboard_shortcut.clone(),
        excluded_apps: config
            .excluded_apps
            .iter()
            .map(|entry| entry.executable.clone())
            .collect(),
    };
    register_launcher_trigger(&app, &native_config)?;
    settings::save_launcher_trigger_settings(&app, &native_config)?;
    Ok(config)
}

/// 关闭所有快速笔记窗口
#[tauri::command]
async fn close_all_note_windows(app: tauri::AppHandle) -> Result<(), String> {
    // 获取所有窗口
    let windows = app.webview_windows();
    for (label, window) in windows {
        // 只关闭以 "quick-note-" 开头的窗口
        if label.starts_with("quick-note-") {
            let _ = window.close();
        }
    }
    Ok(())
}

/// 显示或隐藏所有快速笔记窗口
#[tauri::command]
async fn toggle_note_windows_visibility(app: tauri::AppHandle) -> Result<bool, String> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let launched_from_autostart = std::env::args().any(|arg| arg == "--autostart");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().with_denylist(&["launcher"]).build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // 当另一个实例尝试启动时，显示主窗口
            if let Some(window) = app.get_webview_window("manager") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--autostart"])
                .app_name("EasyFlowHub")
                .build(),
        )
        .setup(move |app| {
            // 日志与开发者模式：读取持久化偏好，之后所有 native / sidecar
            // 诊断都写入有界内存缓冲（开启调试时同时落盘）。
            crate::logging::init(app.handle());

            // Launcher windows are transient by default. Explicitly reset the
            // native flag on startup so an older build that created the
            // window as always-on-top cannot leak that state into this build.
            if let Some(window) = app.get_webview_window("launcher") {
                // The launcher is explicitly closed by its close button, Esc,
                // or a repeated trigger. Keeping it visible while
                // it loses focus is important because a header drag briefly
                // changes focus on some WebView2 builds.
                let _ = window.set_always_on_top(false);
                let window_clone = window.clone();
                let focus_window = window.clone();
                window.on_window_event(move |event| match event {
                    tauri::WindowEvent::Focused(true) => {
                        let _ = focus_window.emit("launcher-focused", ());
                    }
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = dismiss_launcher(&window_clone);
                    }
                    _ => {}
                });
            }

            // 监听 manager 窗口关闭事件，改为隐藏
            if let Some(window) = app.get_webview_window("manager") {
                let window_clone = window.clone();
                if launched_from_autostart {
                    let _ = window.hide();
                }
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // 阻止默认关闭行为
                        api.prevent_close();
                        // 隐藏窗口
                        let _ = window_clone.hide();
                    }
                });
            }

            // 创建托盘菜单
            let show_manager = MenuItem::with_id(app, "show_manager", "打开管理中心", true, None::<&str>)
                .map_err(|e| format!("Failed to create menu item: {}", e))?;
            let new_note = MenuItem::with_id(app, "new_note", "新建快速笔记", true, None::<&str>)
                .map_err(|e| format!("Failed to create menu item: {}", e))?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
                .map_err(|e| format!("Failed to create menu item: {}", e))?;

            let menu = Menu::with_items(app, &[&show_manager, &new_note, &quit])
                .map_err(|e| format!("Failed to create menu: {}", e))?;

            // 创建托盘图标
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show_manager" => {
                        if let Some(window) = app.get_webview_window("manager") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "new_note" => {
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = create_note_window(app_handle, None).await;
                        });
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("manager") {
                            if let Ok(visible) = window.is_visible() {
                                if visible {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(app)
                .map_err(|e| format!("Failed to create tray: {}", e))?;

            // 先清除所有已注册的快捷键
            let _ = app.global_shortcut().unregister_all();

            // Ctrl+Alt+N - 创建快速笔记窗口
            let new_note_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN);
            let app_handle = app.handle().clone();
            if let Err(e) = app.global_shortcut().on_shortcut(new_note_shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let app = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = create_note_window(app, None).await;
                });
            }) {
                eprintln!("Warning: Failed to register Ctrl+Alt+N shortcut: {:?}", e);
            }

            // Ctrl+Alt+M - 显示/隐藏管理窗口
            let toggle_manager_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyM);
            let app_handle = app.handle().clone();
            if let Err(e) = app.global_shortcut().on_shortcut(toggle_manager_shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }

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

                if let Some(window) = app_handle.get_webview_window("manager") {
                    if let Ok(visible) = window.is_visible() {
                        if visible {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            }) {
                eprintln!("Warning: Failed to register Ctrl+Alt+M shortcut: {:?}", e);
            }

            // Launcher trigger - loaded from persisted settings. Both the
            // keyboard registration and optional mouse hook are configured here.
            let launcher_settings = settings::get_settings(app.handle().clone())
                .map(|response| response.settings.launcher_trigger)
                .unwrap_or_default();
            if let Err(e) = register_launcher_trigger(app.handle(), &launcher_settings) {
                eprintln!("Warning: Failed to register Launcher trigger: {e}");
            }

            // Ctrl+Alt+D - 关闭所有快速笔记窗口
            let close_all_notes_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD);
            let app_handle = app.handle().clone();
            if let Err(e) = app.global_shortcut().on_shortcut(close_all_notes_shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let app = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = close_all_note_windows(app).await;
                });
            }) {
                eprintln!("Warning: Failed to register Ctrl+Alt+D shortcut: {:?}", e);
            }

            // Ctrl+Alt+H - 隐藏/显示所有快速笔记窗口
            let toggle_notes_visibility_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyH);
            let app_handle = app.handle().clone();
            if let Err(e) = app.global_shortcut().on_shortcut(toggle_notes_visibility_shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let app = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = toggle_note_windows_visibility(app).await;
                });
            }) {
                eprintln!("Warning: Failed to register Ctrl+Alt+H shortcut: {:?}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            toggle_always_on_top,
            hide_window,
            close_window,
            create_note_window,
            create_todo_card_window,
            toggle_todo_card_pin,
            show_manager_window,
            hide_manager_window,
            show_launcher_window,
            hide_launcher_window,
            start_launcher_dragging,
            set_launcher_always_on_top,
            get_launcher_always_on_top,
            apply_launcher_trigger_settings,
            close_all_note_windows,
            toggle_note_windows_visibility,
            scriptmgr_server::ensure_scriptmgr,
            logging::get_debug_logs,
            logging::clear_debug_logs,
            logging::set_debug_mode,
            logging::open_debug_log_dir,
            scriptmgr::list_scripts,
            scriptmgr::describe_script,
            scriptmgr::run_script,
            notes::list_notes,
            notes::get_note,
            notes::save_note,
            notes::delete_note,
            notes::search_notes,
            notes::save_image,
            notes::save_image_from_path,
            notes::create_note,
            notes::toggle_pin_note,
            notes::trash_note,
            notes::trash_notes_batch,
            notes::list_trash,
            notes::restore_note,
            notes::restore_notes_batch,
            notes::permanent_delete_note,
            notes::empty_trash,
            inbox::list_inbox_items,
            inbox::capture_inbox_item,
            inbox::archive_inbox_item,
            inbox::restore_inbox_item,
            inbox::convert_inbox_item_to_note,
            inbox::open_inbox_attachment,
            inbox::open_inbox_link,
            inbox::delete_inbox_item,
            inbox::open_inbox_assets_dir,
            settings::get_settings,
            settings::update_settings,
            settings::get_foreground_process_name,
            replace_selected_text,
            appearance::set_always_on_top,
            appearance::get_window_state,
            appearance::set_window_opacity,
            clipboard::copy_text_to_clipboard,
            clipboard::read_text_from_clipboard,
            widget::create_folder_widget,
            widget::get_folder_apps,
            widget::launch_app,
            widget::close_widget,
            widget::set_widget_size,
            external_apps::launch_snow_shot,
            external_apps::open_snow_shot_install_page,
            file_search::file_search_get_status,
            file_search::file_search_set_root,
            file_search::file_search_set_paused,
            file_search::file_search_refresh_index,
            file_search::file_search_clear_index,
            file_search::file_search_search,
            file_search::file_search_preview,
            file_search::file_search_open_path,
            file_search::file_search_reveal_in_explorer,
            file_search::file_search_copy_path,
            file_search::file_search_recent,
            file_search::file_search_rename_path,
            file_search::file_search_move_path,
            file_search::file_search_copy_file,
            file_search::file_search_delete_to_recycle_bin,
            download::download_get_settings,
            download::download_update_settings,
            download::download_list,
            download::download_enqueue,
            download::download_pause,
            download::download_resume,
            download::download_cancel,
            download::download_retry,
            download::download_open,
            download::download_reveal,
            download::download_copy_path,
            voice::voice_get_state,
            voice::voice_start,
            voice::voice_pause,
            voice::voice_resume,
            voice::voice_stop,
            voice::voice_open,
            voice::voice_open_privacy_settings,
            transcription::transcribe_audio,
            transcription::get_transcript,
            modules::load_modules_config,
            modules::save_modules_config,
            modules::toggle_module_config,
            mcp_server::get_mcp_info
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // 托管 scriptmgr HTTP sidecar：面板按需拉取数据时由 ensure_scriptmgr
    // 启动，应用退出时回收子进程，避免孤儿进程长期占用 8765 端口。
    app.manage(scriptmgr_server::ScriptmgrState::default());

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(state) = app_handle.try_state::<scriptmgr_server::ScriptmgrState>() {
                state.shutdown();
            }
        }
    });
}
