// src-tauri/src/widget.rs
//! Desktop Widget System - 桌面小组件系统
//!
//! 提供可固定在桌面上的小组件，支持文件夹组件等功能

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{command, AppHandle, Manager};

/// 组件窗口计数器
static WIDGET_COUNTER: AtomicU32 = AtomicU32::new(1);

/// 应用快捷方式信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppShortcut {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub exec_path: String,
}

/// 文件夹组件配置（预留，未来使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct FolderWidgetConfig {
    pub id: String,
    pub name: String,
    pub apps: Vec<AppShortcut>,
}

/// 预设的应用快捷方式（用于原型测试）
fn get_default_apps() -> Vec<AppShortcut> {
    vec![
        AppShortcut {
            id: "notepad".to_string(),
            name: "记事本".to_string(),
            icon: "📝".to_string(),
            exec_path: "notepad.exe".to_string(),
        },
        AppShortcut {
            id: "calculator".to_string(),
            name: "计算器".to_string(),
            icon: "🔢".to_string(),
            exec_path: "calc.exe".to_string(),
        },
        AppShortcut {
            id: "explorer".to_string(),
            name: "文件".to_string(),
            icon: "📁".to_string(),
            exec_path: "explorer.exe".to_string(),
        },
        AppShortcut {
            id: "terminal".to_string(),
            name: "终端".to_string(),
            icon: "💻".to_string(),
            exec_path: "cmd.exe".to_string(),
        },
        AppShortcut {
            id: "browser".to_string(),
            name: "浏览器".to_string(),
            icon: "🌐".to_string(),
            exec_path: "msedge.exe".to_string(),
        },
        AppShortcut {
            id: "settings".to_string(),
            name: "设置".to_string(),
            icon: "⚙️".to_string(),
            exec_path: "ms-settings:".to_string(),
        },
        AppShortcut {
            id: "paint".to_string(),
            name: "画图".to_string(),
            icon: "🎨".to_string(),
            exec_path: "mspaint.exe".to_string(),
        },
        AppShortcut {
            id: "screenshot".to_string(),
            name: "截图".to_string(),
            icon: "📸".to_string(),
            exec_path: "ms-screenclip:".to_string(),
        },
        AppShortcut {
            id: "store".to_string(),
            name: "商店".to_string(),
            icon: "🛒".to_string(),
            exec_path: "ms-windows-store:".to_string(),
        },
    ]
}

/// 创建文件夹组件窗口
#[command]
pub async fn create_folder_widget(
    app: AppHandle,
    widget_id: Option<String>,
) -> Result<String, String> {
    let counter = WIDGET_COUNTER.fetch_add(1, Ordering::SeqCst);
    let label = widget_id.unwrap_or_else(|| format!("folder-widget-{}", counter));

    // 检查窗口是否已存在
    if app.get_webview_window(&label).is_some() {
        // 窗口已存在，显示并聚焦
        if let Some(window) = app.get_webview_window(&label) {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
        return Ok(label);
    }

    // 组件窗口 URL
    let url = format!("/?mode=widget&type=folder&id={}", label);

    let _window = tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App(url.into())
    )
    .title("EasyFlowHub - 文件夹组件")
    .inner_size(80.0, 80.0)  // 收起状态的小尺寸
    .min_inner_size(80.0, 80.0)
    .max_inner_size(400.0, 400.0)
    .transparent(true)
    .always_on_top(true)
    .decorations(false)
    .skip_taskbar(true)
    .resizable(false)
    .center()
    .build()
    .map_err(|e: tauri::Error| e.to_string())?;

    Ok(label)
}

/// 获取文件夹组件的默认应用列表
#[command]
pub async fn get_folder_apps() -> Result<Vec<AppShortcut>, String> {
    Ok(get_default_apps())
}

/// 启动应用
#[command]
pub async fn launch_app(exec_path: String) -> Result<(), String> {
    // 特殊处理 Windows 设置
    if exec_path.starts_with("ms-settings:") {
        // 使用 shell 打开 Windows 设置 URI
        open::that(&exec_path).map_err(|e| format!("Failed to open settings: {}", e))?;
        return Ok(());
    }

    // 使用 tauri-plugin-shell 启动应用
    // 这里我们使用系统默认方式打开
    open::that(&exec_path).map_err(|e| format!("Failed to launch app: {}", e))?;

    Ok(())
}

/// 关闭组件窗口
#[command]
pub async fn close_widget(app: AppHandle, widget_id: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&widget_id) {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 设置窗口大小（用于展开/收起动画）
#[command]
pub async fn set_widget_size(
    app: AppHandle,
    widget_id: String,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&widget_id) {
        window
            .set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: width as u32,
                height: height as u32,
            }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
