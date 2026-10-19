//! Bounded in-memory log ring plus an optional file sink.
//!
//! The developer mode toggle controls verbosity and file output only; it never
//! changes safety rules or exposes remote access. Logs are always kept in a
//! bounded in-memory ring so the settings panel can show recent diagnostics
//! even when the toggle was just switched on.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

const MAX_LINES: usize = 500;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

fn buffer() -> &'static Mutex<VecDeque<String>> {
    static BUFFER: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_LINES)))
}

pub fn set_debug(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Resolve (and create) the log directory under app data.
pub fn log_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取日志目录失败: {error}"))?
        .join("logs");
    fs::create_dir_all(&dir).map_err(|error| format!("创建日志目录失败: {error}"))?;
    Ok(dir)
}

/// Wire the log sink and restore the persisted debug preference.
pub fn init(app: &tauri::AppHandle) {
    if let Ok(dir) = log_dir(app) {
        let _ = LOG_DIR.set(dir);
    }
    if let Ok(response) = crate::settings::get_settings(app.clone()) {
        set_debug(response.settings.developer.debug_mode);
    }
    log("info", "app", "EasyFlowHub started");
}

/// Append one line to the in-memory ring (and the file when debug is on).
pub fn log(level: &str, source: &str, message: &str) {
    let line = format!(
        "{} [{}] [{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        level.to_uppercase(),
        source,
        message
    );

    if let Ok(mut guard) = buffer().lock() {
        if guard.len() >= MAX_LINES {
            guard.pop_front();
        }
        guard.push_back(line.clone());
    }

    if debug_enabled() {
        append_to_file(&line);
    }
}

fn append_to_file(line: &str) {
    let Some(dir) = LOG_DIR.get() else {
        return;
    };
    let path = dir.join("app.log");
    if let Ok(metadata) = fs::metadata(&path) {
        if metadata.len() > MAX_FILE_BYTES {
            let _ = fs::rename(&path, dir.join("app.log.1"));
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Return the newest `limit` buffered lines (oldest first).
pub fn snapshot(limit: usize) -> Vec<String> {
    let Ok(guard) = buffer().lock() else {
        return Vec::new();
    };
    let take = limit.max(1).min(guard.len());
    guard.iter().skip(guard.len() - take).cloned().collect()
}

pub fn clear() {
    if let Ok(mut guard) = buffer().lock() {
        guard.clear();
    }
}

// ============================================================================
// Commands
// ============================================================================

#[tauri::command]
pub fn get_debug_logs(limit: Option<usize>) -> Vec<String> {
    snapshot(limit.unwrap_or(200))
}

#[tauri::command]
pub fn clear_debug_logs() {
    clear();
    log("info", "app", "日志已清空");
}

#[tauri::command]
pub fn set_debug_mode(enabled: bool) -> bool {
    set_debug(enabled);
    log("info", "app", if enabled { "开发者模式已开启" } else { "开发者模式已关闭" });
    enabled
}

#[tauri::command]
pub fn open_debug_log_dir(app: tauri::AppHandle) -> Result<(), String> {
    let dir = log_dir(&app)?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|error| format!("打开日志目录失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The ring buffer is process-global and tests run in parallel, so these
    // assertions filter their own tagged lines instead of assuming exclusivity.

    #[test]
    fn ring_buffer_is_bounded_and_keeps_newest_lines() {
        for index in 0..(MAX_LINES + 25) {
            log("info", "test-ring", &format!("ring-{index}"));
        }
        let lines = snapshot(MAX_LINES);
        assert!(lines.len() <= MAX_LINES);
        assert!(lines
            .iter()
            .any(|line| line.contains(&format!("ring-{}", MAX_LINES + 24))));
        assert!(!lines.iter().any(|line| line.ends_with("ring-0")));
    }

    #[test]
    fn snapshot_limits_and_clamps() {
        for index in 0..10 {
            log("info", "test-snap", &format!("snap-{index}"));
        }
        let last = snapshot(3);
        assert!(last.len() <= 3);
        assert!(last.last().expect("line").contains("snap-9"));
        assert_eq!(snapshot(0).len(), 1);
    }

    #[test]
    fn debug_flag_round_trips() {
        set_debug(true);
        assert!(debug_enabled());
        set_debug(false);
        assert!(!debug_enabled());
    }
}
