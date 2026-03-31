use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, PhysicalPosition, Position, Size, WebviewWindow,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

mod scriptmgr;
mod notes;
mod appearance;
mod widget;
mod settings;
mod modules;
mod mcp_server;

// 快速笔记窗口计数器
static NOTE_WINDOW_COUNTER: AtomicU32 = AtomicU32::new(1);
// Todo 卡片窗口计数器
static TODO_CARD_COUNTER: AtomicU32 = AtomicU32::new(1);
// 防止快捷键重复触发
static LAST_TOGGLE_TIME: Mutex<i64> = Mutex::new(0);

#[tauri::command]
async fn toggle_always_on_top(window: WebviewWindow) -> Result<bool, String> {
    let is_on_top = window.is_always_on_top().map_err(|e: tauri::Error| e.to_string())?;
    window.set_always_on_top(!is_on_top).map_err(|e: tauri::Error| e.to_string())?;
    Ok(!is_on_top)
}

#[tauri::command]
async fn hide_window(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e: tauri::Error| e.to_string())
}

#[tauri::command]
async fn close_window(window: WebviewWindow) -> Result<(), String> {
    window.close().map_err(|e: tauri::Error| e.to_string())
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
    let min_x = work_area.position.x;
    let min_y = work_area.position.y;
    let max_x = work_area.position.x + work_area.size.width as i32 - width.round() as i32;
    let max_y = work_area.position.y + work_area.size.height as i32 - height.round() as i32;

    let desired_x = cursor.x.round() as i32 + 18;
    let desired_y = cursor.y.round() as i32 + 18;

    Some(PhysicalPosition::new(
        desired_x.clamp(min_x, max_x.max(min_x)),
        desired_y.clamp(min_y, max_y.max(min_y)),
    ))
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
    .skip_taskbar(false);

    let builder = if let Some(position) = compute_quick_note_position(&app, width, height) {
        builder.position(position.x as f64, position.y as f64)
    } else {
        builder.center()
    };

    let window = builder
    .build()
    .map_err(|e: tauri::Error| e.to_string())?;

    window
        .set_size(Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e: tauri::Error| e.to_string())?;

    if let Some(position) = compute_quick_note_position(&app, width, height) {
        window
            .set_position(Position::Physical(position))
            .map_err(|e: tauri::Error| e.to_string())?;
    }

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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // 监听 manager 窗口关闭事件，改为隐藏
            if let Some(window) = app.get_webview_window("manager") {
                let window_clone = window.clone();
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
            close_all_note_windows,
            toggle_note_windows_visibility,
            scriptmgr::list_scripts,
            scriptmgr::describe_script,
            scriptmgr::run_script,
            notes::list_notes,
            notes::get_note,
            notes::save_note,
            notes::delete_note,
            notes::search_notes,
            notes::save_image,
            notes::create_note,
            notes::toggle_pin_note,
            notes::trash_note,
            notes::trash_notes_batch,
            notes::list_trash,
            notes::restore_note,
            notes::restore_notes_batch,
            notes::permanent_delete_note,
            notes::empty_trash,
            settings::get_settings,
            settings::update_settings,
            appearance::set_always_on_top,
            appearance::get_window_state,
            appearance::set_window_opacity,
            widget::create_folder_widget,
            widget::get_folder_apps,
            widget::launch_app,
            widget::close_widget,
            widget::set_widget_size,
            modules::load_modules_config,
            modules::save_modules_config,
            modules::toggle_module_config,
            mcp_server::get_mcp_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
