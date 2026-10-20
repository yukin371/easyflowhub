//! Launcher window behavior: selection capture / replace, keyboard + middle
//! mouse trigger registration, and the Launcher's show / hide / pin window
//! actions.
//!
//! This module owns the Launcher's native lifecycle. The window itself is not
//! restored by the window-state plugin, is hidden (not destroyed) on close,
//! and every dismissal emits `launcher-hidden` so the webview can cancel
//! in-flight work. Window placement reuses `window_position`; the Manager /
//! quick-note / todo window behavior lives in `app_windows`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use tauri::{Emitter, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::settings;

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
pub struct LauncherSettingsInput {
    #[serde(default = "default_launcher_activation")]
    activation: String,
    #[serde(default = "default_launcher_shortcut")]
    keyboard_shortcut: String,
    #[serde(default)]
    excluded_apps: Vec<LauncherExcludedAppInput>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherExcludedAppInput {
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

fn default_launcher_shortcut() -> String {
    "Ctrl+Alt+Space".to_string()
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
pub async fn replace_selected_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
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

impl Default for LauncherSettingsInput {
    fn default() -> Self {
        Self {
            activation: default_launcher_activation(),
            keyboard_shortcut: default_launcher_shortcut(),
            excluded_apps: Vec::new(),
        }
    }
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
    let Some(position) = crate::window_position::cursor_anchored_position(app, width, height) else { return; };
    let _ = window.set_position(tauri::Position::Physical(position));
}

/// Show the action launcher and focus its search field.
#[tauri::command]
pub async fn show_launcher_window(app: tauri::AppHandle) -> Result<(), String> {
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
pub(crate) fn dismiss_launcher(window: &WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())?;
    window.emit("launcher-hidden", ()).map_err(|e| e.to_string())
}

/// Hide the launcher without destroying its webview so the next trigger opens
/// it immediately and keeps the user's current panel state.
#[tauri::command]
pub async fn hide_launcher_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("launcher") {
        dismiss_launcher(&window)?;
    }
    Ok(())
}

/// Start a native drag for the frameless launcher window. The frontend calls
/// this directly from the header's primary mouse event; the data attribute is
/// retained as a fallback for Tauri's injected drag-region handler.
#[tauri::command]
pub async fn start_launcher_dragging(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("launcher")
        .ok_or_else(|| "Launcher window not found".to_string())?;
    window
        .start_dragging()
        .map_err(|e: tauri::Error| e.to_string())
}

/// Set the launcher window's pin state without affecting the Manager window.
#[tauri::command]
pub async fn set_launcher_always_on_top(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
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
pub fn get_launcher_always_on_top(app: tauri::AppHandle) -> Result<bool, String> {
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
pub(crate) fn register_launcher_trigger(
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
pub fn apply_launcher_trigger_settings(
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

/// Wire the native Launcher window events at startup: reset the always-on-top
/// flag so an older build cannot leak that state, emit `launcher-focused` on
/// focus, and turn close requests into a hide.
pub(crate) fn setup_launcher_window(app: &tauri::AppHandle) {
    // Launcher windows are transient by default. Explicitly reset the
    // native flag on startup so an older build that created the
    // window as always-on-top cannot leak that state into this build.
    let Some(window) = app.get_webview_window("launcher") else { return; };
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
