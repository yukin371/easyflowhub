// src-tauri/src/appearance.rs
use tauri::{command, AppHandle, Manager};

/// 窗口状态响应结构
#[derive(serde::Serialize)]
pub struct WindowState {
    pub is_always_on_top: bool,
}

/// 设置窗口置顶状态
#[command]
pub async fn set_always_on_top(app: AppHandle, enabled: bool) -> Result<bool, String> {
    // 尝试获取主窗口
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(enabled)
            .map_err(|e| format!("Failed to set always on top: {}", e))?;
        return Ok(enabled);
    }

    // 尝试获取 manager 窗口
    if let Some(window) = app.get_webview_window("manager") {
        window
            .set_always_on_top(enabled)
            .map_err(|e| format!("Failed to set always on top: {}", e))?;
        return Ok(enabled);
    }

    // 尝试获取当前焦点窗口
    if let Some(window) = app.get_focused_window() {
        window
            .set_always_on_top(enabled)
            .map_err(|e| format!("Failed to set always on top: {}", e))?;
        return Ok(enabled);
    }

    Err("No window found".to_string())
}

/// 获取窗口状态
#[command]
pub async fn get_window_state(app: AppHandle) -> Result<WindowState, String> {
    // 尝试获取主窗口
    if let Some(window) = app.get_webview_window("main") {
        let is_always_on_top = window
            .is_always_on_top()
            .map_err(|e| format!("Failed to get always on top state: {}", e))?;
        return Ok(WindowState { is_always_on_top });
    }

    // 尝试获取 manager 窗口
    if let Some(window) = app.get_webview_window("manager") {
        let is_always_on_top = window
            .is_always_on_top()
            .map_err(|e| format!("Failed to get always on top state: {}", e))?;
        return Ok(WindowState { is_always_on_top });
    }

    Ok(WindowState { is_always_on_top: false })
}

/// 设置窗口透明度
/// 注意：Tauri 2 的 WebviewWindow 不直接支持运行时透明度设置
/// 透明窗口需要在创建时通过 .transparent(true) 设置
/// 此函数作为占位符，实际透明度效果通过 CSS 实现
#[command]
pub async fn set_window_opacity(_app: AppHandle, _opacity: f64) -> Result<(), String> {
    // Tauri 2 目前不支持运行时修改窗口透明度
    // 透明窗口需要在 WebviewWindowBuilder 时设置 .transparent(true)
    // 透明度效果应通过前端 CSS 实现
    // 这是一个占位实现，保持 API 兼容性
    Ok(())
}

