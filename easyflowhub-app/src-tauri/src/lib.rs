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
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

mod app_windows;
mod autostart;
mod clipboard;
mod db;
mod download;
mod external_apps;
mod appearance;
mod inbox;
mod launcher;
mod logging;
mod mcp_server;
mod modules;
mod native_shell;
mod notes;
mod scriptmgr;
mod scriptmgr_server;
mod settings;
mod shell;
mod transcription;
mod voice;
mod widget;
mod window_position;
mod file_search;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `--shell-report` 回答"这个构建会做什么"，但不构造 App：不抢单实例互斥、
    // 不拉 WebView、不注册热键。于是门禁可以在"真程序已经在跑"的机器上判断
    // 构建行为，而不是先杀掉用户正在用的进程。
    if std::env::args().any(|arg| arg == "--shell-report") {
        println!("{}", shell::report());
        return;
    }

    let app = tauri::Builder::default()
        // 单实例必须是第一个注册的插件：它要在其余插件初始化、以及
        // tauri.conf.json 的窗口创建之前就完成 argv 转发并退出第二个进程。
        // 注册晚了第二个进程会留下一个 about:blank 窗口和一整套热键注册。
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // 开机自启触发的第二个实例（上一次的进程尚未退干净、或用户在
            // 登录瞬间又点了一次图标）不应该把 Manager 抢到前台。
            if argv.iter().any(|arg| arg == "--autostart") {
                return;
            }
            // 当另一个实例尝试启动时，显示主窗口（不存在则按需创建）。
            //
            // 必须排到主线程队列里而不是就地创建：这个回调本身跑在事件循环
            // 线程上，而 WebView2 的窗口创建需要事件循环继续转动。就地建窗口
            // 会互相等待——第二个进程卡在握手、第一个进程留下一个 about:blank
            // 的空白窗口（实测 2026-11-24）。
            app_windows::request_manager_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().with_denylist(&["launcher"]).build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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

            // 自启动对账：持久化意图为「开」而系统启动项被外部删除时补回来
            // （plan §2.4 / G1 门禁）。只读一次注册表，状态一致时不写。
            let autostart_state = autostart::reconcile(app.handle());
            if autostart_state.repaired {
                crate::logging::log("info", "autostart", "启动项已在本次启动中自愈");
            }

            // Launcher 窗口生命周期归 launcher 模块：重置置顶标记、聚焦
            // 事件、关闭改为隐藏。
            //
            // T-Gate 0 起这个窗口不再由 tauri.conf.json 声明：本进程只保留
            // 按需的 Manager，面板归原生壳。回退模式（`--legacy-ui`）才现建
            // 一个，否则"回退"只是一句没人验证过的话。
            let handover = shell::current();
            shell::announce(handover);
            if !handover.takeover() {
                launcher::ensure_window(app.handle())?;
            }
            launcher::setup_launcher_window(app.handle());

            // 创建托盘菜单
            let show_manager = MenuItem::with_id(app, "show_manager", "打开管理中心", true, None::<&str>)
                .map_err(|e| format!("Failed to create menu item: {}", e))?;
            let new_note = MenuItem::with_id(app, "new_note", "新建快速笔记", true, None::<&str>)
                .map_err(|e| format!("Failed to create menu item: {}", e))?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
                .map_err(|e| format!("Failed to create menu item: {}", e))?;

            // 接管后托盘只剩「打开管理中心」与「退出」：托盘上的快速笔记入口
            // 会创建快速笔记 webview，那正是 T-Gate 0 要停的东西，原生托盘
            // 有自己的「新建快速笔记」。回退模式保留原来的三项。
            let menu = if handover.takeover() {
                Menu::with_items(app, &[&show_manager, &quit])
            } else {
                Menu::with_items(app, &[&show_manager, &new_note, &quit])
            }
            .map_err(|e| format!("Failed to create menu: {}", e))?;

            // 创建托盘图标
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show_manager" => {
                        app_windows::request_manager_window(app);
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
                        // Tray click toggles: hide a visible Manager, otherwise
                        // create/reveal it. Creation is on demand (Phase 0.5).
                        let app = tray.app_handle();
                        match app.get_webview_window("manager") {
                            Some(window) if window.is_visible().unwrap_or(false) => {
                                let _ = window.hide();
                            }
                            Some(window) => {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                            None => app_windows::request_manager_window(app),
                        }
                    }
                })
                .build(app)
                .map_err(|e| format!("Failed to create tray: {}", e))?;

            // 先清除所有已注册的快捷键
            let _ = app.global_shortcut().unregister_all();

            // 接管后本进程一个全局键都不注册：五个组合全归原生壳。让位不是
            // 礼貌问题——第一次跑这个切换时这里还留着 Ctrl+Alt+M，原生壳当场
            // 报 `RegisterHotKey failed (1409)`，同一个程序里两个栈抢一个组合，
            // 谁赢取决于启动顺序。Manager 仍可从托盘打开。
            //
            // 这份计划由 `shell::shortcut_plan` 给出，`--shell-report` 读的是
            // 同一份：门禁断言"那三条键没有注册"时，断的是这个进程真正的行为。
            for planned in shell::shortcut_plan(handover.takeover()) {
                let Ok(shortcut) = planned.chord.parse::<Shortcut>() else {
                    eprintln!("Warning: cannot parse {} for {}", planned.chord, planned.action.id());
                    continue;
                };
                let app_handle = app.handle().clone();
                let action = planned.action;
                if let Err(e) = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    shell::run_shortcut(&app_handle, action);
                }) {
                    eprintln!("Warning: Failed to register {}: {:?}", planned.chord, e);
                }
            }

            // Launcher trigger - loaded from persisted settings. Both the
            // keyboard registration and optional mouse hook are configured in
            // the launcher module. Skipped under handover: the native shell owns
            // the combination now, and registering it here would take it back.
            if !handover.takeover() {
                let launcher_settings = settings::get_settings(app.handle().clone())
                    .map(|response| response.settings.launcher_trigger)
                    .unwrap_or_default();
                if let Err(e) = launcher::register_launcher_trigger(app.handle(), &launcher_settings) {
                    eprintln!("Warning: Failed to register Launcher trigger: {e}");
                }
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
            launcher::report_first_frame,
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
            inbox::commands::list_inbox_trash,
            inbox::commands::capture_inbox_item,
            inbox::commands::archive_inbox_item,
            inbox::commands::restore_inbox_item,
            inbox::commands::convert_inbox_item_to_note,
            inbox::commands::open_inbox_attachment,
            inbox::commands::open_inbox_link,
            inbox::commands::delete_inbox_item,
            inbox::commands::restore_inbox_item_from_trash,
            inbox::commands::permanently_delete_inbox_item,
            inbox::commands::empty_inbox_trash,
            inbox::commands::open_inbox_assets_dir,
            settings::get_settings,
            settings::update_settings,
            settings::get_foreground_process_name,
            autostart::get_autostart_state,
            autostart::set_autostart_state,
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

    app.run(|app_handle, event| match event {
        // T-Gate 0 之后这个进程通常只剩 Manager 一个窗口，而 Tauri 的默认规则是
        // 「最后一个窗口销毁即退出」：runtime 收到 Destroyed、窗口表清空就发
        // `ExitRequested { code: None }`。接管态里本进程一个热键都不注册，托盘是
        // 唯一入口——关掉 Manager 会把托盘一起带走，用户再想打开只能重新启动程序。
        //
        // 所以「没有窗口」是一种休息态：只挡 `code: None` 这条隐式路径。托盘
        // 「退出」走的是 `app.exit(0)`，带 `Some(0)`，仍然照常退出。
        tauri::RunEvent::ExitRequested { code: None, api, .. } => {
            if shell::current().takeover() {
                api.prevent_exit();
            }
        }
        tauri::RunEvent::Exit => {
            if let Some(state) = app_handle.try_state::<scriptmgr_server::ScriptmgrState>() {
                state.shutdown();
            }
        }
        _ => {}
    });
}
