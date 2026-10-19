//! Native clipboard operations shared by file and download actions.
//!
//! The webview clipboard API is not a reliable Windows-first boundary: it can
//! be unavailable outside a secure context or reject while another process
//! owns the clipboard. Keep the actual write in the native shell.

#[cfg(target_os = "windows")]
fn copy_text_native(value: &str) -> Result<(), String> {
    clipboard_win::set_clipboard_string(value)
        .map_err(|error| format!("无法写入系统剪贴板：{error}"))
}

#[cfg(target_os = "windows")]
fn read_text_native() -> Result<String, String> {
    clipboard_win::get_clipboard_string()
        .map_err(|error| format!("无法读取系统剪贴板：{error}"))
}

#[cfg(not(target_os = "windows"))]
fn read_text_native() -> Result<String, String> {
    Err("当前平台未启用系统剪贴板读取。".to_string())
}

#[cfg(not(target_os = "windows"))]
fn copy_text_native(_value: &str) -> Result<(), String> {
    Err("当前平台未启用系统剪贴板写入。".to_string())
}

/// Write plain text to the operating system clipboard.
#[tauri::command]
pub fn copy_text_to_clipboard(text: String) -> Result<(), String> {
    if text.is_empty() {
        return Err("不能复制空内容。".to_string());
    }
    copy_text_native(&text)
}

/// Read plain text from the operating system clipboard.
#[tauri::command]
pub fn read_text_from_clipboard() -> Result<String, String> {
    read_text_native()
}

#[cfg(test)]
mod tests {
    use super::copy_text_to_clipboard;

    #[test]
    fn empty_text_is_rejected_before_touching_the_os_clipboard() {
        assert_eq!(copy_text_to_clipboard(String::new()), Err("不能复制空内容。".to_string()));
    }
}

