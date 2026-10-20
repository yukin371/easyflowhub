//! Native composition root.
//!
//! `run()` assembles the Tauri application: plugins, state injection and
//! command registration. Launcher selection / trigger / window behavior lives
//! in `launcher`; Manager / quick-note / todo window behavior lives in
//! `app_windows`. This file owns no window or domain behavior itself — it only
//! wires them together and registers every command with the invoke handler.

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

mod app_windows;
mod clipboard;
mod download;
mod external_apps;
mod appearance;
mod inbox;
mod launcher;
mod logging;
mod mcp_server;
mod modules;
mod notes;
mod scriptmgr;
mod scriptmgr_server;
mod settings;
mod transcription;
mod voice;
mod widget;
mod window_position;
mod file_search;

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

            // Launcher 窗口生命周期归 launcher 模块：重置置顶标记、聚焦
            // 事件、关闭改为隐藏。
            launcher::setup_launcher_window(app.handle());

            // Manager 窗口关闭时默认隐藏；autostart 启动时保持隐藏。
            app_windows::setup_manager_window(app.handle(), launched_from_autostart);

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
                            let _ = app_windows::create_note_window(app_handle, None).await;
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
                    let _ = app_windows::create_note_window(app, None).await;
                });
            }) {
                eprintln!("Warning: Failed to register Ctrl+Alt+N shortcut: {:?}", e);
            }

            // Ctrl+Alt+M - 显示/隐藏管理窗口（防抖归 app_windows）
            let toggle_manager_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyM);
            let app_handle = app.handle().clone();
            if let Err(e) = app.global_shortcut().on_shortcut(toggle_manager_shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                app_windows::toggle_manager_window(&app_handle);
            }) {
                eprintln!("Warning: Failed to register Ctrl+Alt+M shortcut: {:?}", e);
            }

            // Launcher trigger - loaded from persisted settings. Both the
            // keyboard registration and optional mouse hook are configured in
            // the launcher module.
            let launcher_settings = settings::get_settings(app.handle().clone())
                .map(|response| response.settings.launcher_trigger)
                .unwrap_or_default();
            if let Err(e) = launcher::register_launcher_trigger(app.handle(), &launcher_settings) {
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
                    let _ = app_windows::close_all_note_windows(app).await;
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
                    let _ = app_windows::toggle_note_windows_visibility(app).await;
                });
            }) {
                eprintln!("Warning: Failed to register Ctrl+Alt+H shortcut: {:?}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_windows::toggle_always_on_top,
            app_windows::hide_window,
            app_windows::close_window,
            app_windows::create_note_window,
            app_windows::create_todo_card_window,
            app_windows::toggle_todo_card_pin,
            app_windows::show_manager_window,
            app_windows::hide_manager_window,
            launcher::show_launcher_window,
            launcher::hide_launcher_window,
            launcher::start_launcher_dragging,
            launcher::set_launcher_always_on_top,
            launcher::get_launcher_always_on_top,
            launcher::apply_launcher_trigger_settings,
            app_windows::close_all_note_windows,
            app_windows::toggle_note_windows_visibility,
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
            notes::ensure_todo_inbox_note,
            notes::toggle_pin_note,
            notes::trash_note,
            notes::trash_notes_batch,
            notes::list_trash,
            notes::restore_note,
            notes::restore_notes_batch,
            notes::permanent_delete_note,
            notes::empty_trash,
            inbox::commands::list_inbox_items,
            inbox::commands::capture_inbox_item,
            inbox::commands::archive_inbox_item,
            inbox::commands::restore_inbox_item,
            inbox::commands::convert_inbox_item_to_note,
            inbox::commands::open_inbox_attachment,
            inbox::commands::open_inbox_link,
            inbox::commands::delete_inbox_item,
            inbox::commands::open_inbox_assets_dir,
            settings::get_settings,
            settings::update_settings,
            settings::get_foreground_process_name,
            launcher::replace_selected_text,
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
