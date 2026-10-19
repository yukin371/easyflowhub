//! Integrations with optional third-party desktop applications.
//!
//! EasyFlowHub deliberately does not bundle Snow Shot.  This module only
//! discovers an existing installation, starts it, and provides the official
//! download URL when the application is unavailable.

use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Official Snow Shot Windows download page.
pub const SNOW_SHOT_INSTALL_URL: &str = "https://snowshot.top/download.html?os=windows";

const SNOW_SHOT_EXECUTABLE_NAMES: &[&str] = &[
    "Snow Shot.exe",
    "SnowShot.exe",
    "snow-shot.exe",
    "snowshot.exe",
    "snow_shot.exe",
    // Keep extensionless names for PATH entries on Unix and for portable
    // wrappers that are launched through a shell on Windows.
    "snow-shot",
    "snowshot",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnowShotLaunchResult {
    /// `launched`, `not_installed`, or `error`.
    pub status: String,
    pub message: Option<String>,
    pub install_url: Option<String>,
}

impl SnowShotLaunchResult {
    fn launched(path: &Path) -> Self {
        Self {
            status: "launched".to_string(),
            message: Some(format!("Snow Shot 已进入截图模式：{}", path.display())),
            install_url: None,
        }
    }

    fn not_installed(message: impl Into<String>) -> Self {
        Self {
            status: "not_installed".to_string(),
            message: Some(message.into()),
            install_url: Some(SNOW_SHOT_INSTALL_URL.to_string()),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            status: "error".to_string(),
            message: Some(message.into()),
            install_url: None,
        }
    }
}

const SNOW_SHOT_MCP_RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);
const SNOW_SHOT_MCP_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
static SNOW_SHOT_LAUNCH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn path_is_file(path: &Path) -> bool {
    path.is_file()
}

/// Resolve a custom path, a list of well-known locations, and PATH entries in
/// that order.  The helper accepts candidate lists so its ordering can be
/// tested without relying on the host's installed applications.
fn resolve_with_candidates(
    custom_path: Option<&str>,
    common_candidates: &[PathBuf],
    path_entries: &[PathBuf],
) -> Result<Option<PathBuf>, String> {
    if let Some(custom_path) = custom_path.map(str::trim).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(custom_path);
        if path_is_file(&path) {
            return Ok(Some(path));
        }
        return Err(format!("指定的 Snow Shot 路径不存在或不是文件：{custom_path}"));
    }

    if let Some(path) = common_candidates.iter().find(|path| path_is_file(path)) {
        return Ok(Some(path.clone()));
    }

    for directory in path_entries {
        for executable in SNOW_SHOT_EXECUTABLE_NAMES {
            let candidate = directory.join(executable);
            if path_is_file(&candidate) {
                return Ok(Some(candidate));
            }
        }
    }

    Ok(None)
}

fn path_entries_from_environment() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn parse_display_icon_path(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    let path = if let Some(quoted) = value.strip_prefix('"') {
        quoted.split_once('"')?.0
    } else if let Some((path, icon_index)) = value.rsplit_once(',') {
        if icon_index.trim().parse::<i32>().is_ok() {
            path.trim()
        } else {
            value
        }
    } else {
        value
    };

    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn candidates_from_snow_shot_registration(
    display_name: &str,
    display_icon: Option<&str>,
    install_location: Option<&str>,
) -> Vec<PathBuf> {
    let normalized_name: String = display_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();
    if !normalized_name.eq_ignore_ascii_case("snowshot") {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    if let Some(path) = display_icon.and_then(parse_display_icon_path) {
        candidates.push(path);
    }

    if let Some(install_location) = install_location.map(str::trim).filter(|s| !s.is_empty()) {
        let directory = PathBuf::from(install_location);
        for executable in SNOW_SHOT_EXECUTABLE_NAMES {
            candidates.push(directory.join(executable));
            candidates.push(directory.join("bin").join(executable));
        }
    }

    candidates
}

#[cfg(windows)]
fn registered_snow_shot_candidates() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    const UNINSTALL_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    const WOW64_UNINSTALL_PATH: &str =
        r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
    let locations = [
        (HKEY_CURRENT_USER, UNINSTALL_PATH),
        (HKEY_LOCAL_MACHINE, UNINSTALL_PATH),
        (HKEY_LOCAL_MACHINE, WOW64_UNINSTALL_PATH),
    ];
    let mut candidates = Vec::new();

    for (hive, path) in locations {
        let root = RegKey::predef(hive);
        let Ok(uninstall_key) = root.open_subkey(path) else {
            continue;
        };

        for subkey_name in uninstall_key.enum_keys().flatten() {
            let Ok(app_key) = uninstall_key.open_subkey(subkey_name) else {
                continue;
            };
            let display_name: String = app_key.get_value("DisplayName").unwrap_or_default();
            let display_icon: Option<String> = app_key.get_value("DisplayIcon").ok();
            let install_location: Option<String> = app_key.get_value("InstallLocation").ok();
            candidates.extend(candidates_from_snow_shot_registration(
                &display_name,
                display_icon.as_deref(),
                install_location.as_deref(),
            ));
        }
    }

    candidates
}

/// Build common installation and portable locations for the current host.
fn common_snow_shot_candidates() -> Vec<PathBuf> {
    let mut directories = Vec::new();

    // A portable copy beside EasyFlowHub is useful even when PATH is not
    // configured.  `current_exe` is also available in development builds.
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            directories.push(parent.to_path_buf());
            directories.push(parent.join("Snow Shot"));
            directories.push(parent.join("SnowShot"));
        }
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let root = PathBuf::from(local_app_data);
        directories.push(root.join("Snow Shot"));
        directories.push(root.join("SnowShot"));
        directories.push(root.join("Programs").join("Snow Shot"));
        directories.push(root.join("Programs").join("SnowShot"));
    }

    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        let root = PathBuf::from(program_files);
        directories.push(root.join("Snow Shot"));
        directories.push(root.join("SnowShot"));
    }

    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        let root = PathBuf::from(program_files_x86);
        directories.push(root.join("Snow Shot"));
        directories.push(root.join("SnowShot"));
    }

    directories
        .into_iter()
        .flat_map(|directory| {
            SNOW_SHOT_EXECUTABLE_NAMES
                .iter()
                .map(move |executable| directory.join(executable))
        })
        .collect()
}

fn resolve_snow_shot_executable(custom_path: Option<&str>) -> Result<Option<PathBuf>, String> {
    let mut candidates = Vec::new();
    #[cfg(windows)]
    candidates.extend(registered_snow_shot_candidates());
    candidates.extend(common_snow_shot_candidates());

    resolve_with_candidates(
        custom_path,
        &candidates,
        &path_entries_from_environment(),
    )
}

fn open_install_page() -> Result<(), String> {
    open::that(SNOW_SHOT_INSTALL_URL)
        .map_err(|error| format!("无法打开 Snow Shot 安装页：{error}"))
}

#[cfg(windows)]
const SNOW_SHOT_MCP_EXECUTABLE: &str = "snow-shot-mcp.exe";
#[cfg(not(windows))]
const SNOW_SHOT_MCP_EXECUTABLE: &str = "snow-shot-mcp";

fn mcp_response_error(response: &Value) -> Option<String> {
    if let Some(error) = response.get("error") {
        return Some(
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Snow Shot MCP 请求失败")
                .to_string(),
        );
    }

    let Some(result) = response.get("result") else {
        return Some("Snow Shot MCP 返回了不完整响应。".to_string());
    };
    if result.get("isError").and_then(Value::as_bool) != Some(true) {
        return None;
    }

    let text = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| {
            content
                .iter()
                .find(|item| item.get("type").and_then(Value::as_str) == Some("text"))
        })
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str);

    let Some(text) = text else {
        return Some("Snow Shot MCP 请求失败，但没有提供错误详情。".to_string());
    };

    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| Some(text.to_string()))
}

fn mcp_structured_content(response: &Value) -> Option<Value> {
    response
        .get("result")
        .and_then(|result| result.get("structuredContent"))
        .cloned()
        .or_else(|| {
            let text = response
                .get("result")?
                .get("content")?
                .as_array()?
                .iter()
                .find(|item| item.get("type").and_then(Value::as_str) == Some("text"))?
                .get("text")?
                .as_str()?;
            serde_json::from_str(text).ok()
        })
}

fn unavailable_mcp_message(response: &Value) -> Option<String> {
    let structured = mcp_structured_content(response)?;
    if structured.get("reachable").and_then(Value::as_bool) != Some(false)
        && structured.get("mcp_enabled").and_then(Value::as_bool) != Some(false)
    {
        return None;
    }

    Some(
        "Snow Shot 截图接口不可用。请在 Snow Shot 的“设置 → 系统”中开启 MCP integration 后重试。"
            .to_string(),
    )
}

fn write_mcp_message(writer: &mut impl Write, message: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, message)
        .map_err(|error| format!("无法编码 Snow Shot MCP 请求：{error}"))?;
    writer
        .write_all(b"\n")
        .and_then(|_| writer.flush())
        .map_err(|error| format!("无法发送 Snow Shot MCP 请求：{error}"))
}

fn start_mcp_reader(
    stdout: impl std::io::Read + Send + 'static,
) -> (Receiver<Result<Value, String>>, JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let message = line
                .map_err(|error| format!("读取 Snow Shot MCP 响应失败：{error}"))
                .and_then(|line| {
                    serde_json::from_str(&line)
                        .map_err(|error| format!("Snow Shot MCP 返回了无效 JSON：{error}"))
                });
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    (receiver, reader)
}

fn read_mcp_response(
    receiver: &Receiver<Result<Value, String>>,
    writer: &mut impl Write,
    id: u64,
) -> Result<Value, String> {
    let deadline = Instant::now() + SNOW_SHOT_MCP_RESPONSE_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let message = receiver
            .recv_timeout(remaining)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => {
                    "Snow Shot MCP 响应超时，请确认应用运行正常后重试。".to_string()
                }
                RecvTimeoutError::Disconnected => {
                    "Snow Shot MCP 在返回结果前关闭了连接。".to_string()
                }
            })??;

        if message.get("id").and_then(Value::as_u64) == Some(id) {
            return Ok(message);
        }

        // MCP servers may send ping requests while a client request is in
        // flight. Answer them so the bridge does not wait until our timeout.
        if let (Some(request_id), Some(method)) = (
            message.get("id"),
            message.get("method").and_then(Value::as_str),
        ) {
            let response = if method == "ping" {
                json!({ "jsonrpc": "2.0", "id": request_id, "result": {} })
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": { "code": -32601, "message": "Method not found" }
                })
            };
            write_mcp_message(writer, &response)?;
        }
    }
}

fn stop_mcp_process(child: &mut Child) {
    let deadline = Instant::now() + SNOW_SHOT_MCP_SHUTDOWN_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
    }
}

fn call_snow_shot_capture(mcp_executable: &Path) -> Result<(), String> {
    let mut command = Command::new(mcp_executable);
    command
        .arg("--launch-app")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 Snow Shot MCP 桥：{error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Snow Shot MCP 桥未提供标准输出。".to_string())?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Snow Shot MCP 桥未提供标准输入。".to_string())?;
    let (receiver, reader) = start_mcp_reader(stdout);

    let result = (|| {
        write_mcp_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": { "name": "easyflowhub", "version": env!("CARGO_PKG_VERSION") }
                }
            }),
        )?;
        let initialized = read_mcp_response(&receiver, &mut stdin, 1)?;
        if let Some(error) = mcp_response_error(&initialized) {
            return Err(format!("Snow Shot MCP 初始化失败：{error}"));
        }

        write_mcp_message(
            &mut stdin,
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        )?;
        write_mcp_message(
            &mut stdin,
            &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
        )?;
        let tools = read_mcp_response(&receiver, &mut stdin, 2)?;
        if let Some(error) = mcp_response_error(&tools) {
            return Err(format!("无法读取 Snow Shot MCP 工具：{error}"));
        }
        let has_screenshot_tool = tools
            .get("result")
            .and_then(|result| result.get("tools"))
            .and_then(Value::as_array)
            .is_some_and(|tools| {
                tools.iter().any(|tool| {
                    tool.get("name").and_then(Value::as_str)
                        == Some("snow_shot_screenshot_begin")
                })
            });
        if !has_screenshot_tool {
            return Err("当前 Snow Shot 版本没有提供截图 MCP 工具，请更新 Snow Shot 后重试。".to_string());
        }

        write_mcp_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": { "name": "snow_shot_mcp_status", "arguments": {} }
            }),
        )?;
        let status = read_mcp_response(&receiver, &mut stdin, 3)?;
        if let Some(error) = mcp_response_error(&status) {
            return Err(format!("无法检查 Snow Shot MCP 状态：{error}"));
        }
        if let Some(message) = unavailable_mcp_message(&status) {
            return Err(message);
        }

        write_mcp_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "snow_shot_screenshot_begin",
                    "arguments": { "presentation": "visible" }
                }
            }),
        )?;
        let capture = read_mcp_response(&receiver, &mut stdin, 4)?;
        if let Some(error) = mcp_response_error(&capture) {
            return Err(format!("Snow Shot 无法进入截图模式：{error}"));
        }

        Ok(())
    })();

    drop(stdin);
    stop_mcp_process(&mut child);
    let _ = reader.join();
    result
}

/// Start Snow Shot, or report that it is absent.
///
/// `custom_path` is intentionally optional.  When supplied, an invalid path
/// is reported as an error instead of silently falling back to another binary;
/// this prevents launching an unexpected executable after a user has selected
/// a specific installation.  The frontend can then call
/// [`open_snow_shot_install_page`] after presenting the recovery action.
#[tauri::command]
pub async fn launch_snow_shot(custom_path: Option<String>) -> SnowShotLaunchResult {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let launch_lock = SNOW_SHOT_LAUNCH_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = match launch_lock.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                return SnowShotLaunchResult::error("Snow Shot 截图操作正在启动，请稍候。")
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return SnowShotLaunchResult::error(
                    "Snow Shot 截图服务状态异常，请重启 EasyFlowHub 后重试。",
                )
            }
        };

        let executable = match resolve_snow_shot_executable(custom_path.as_deref()) {
            Ok(Some(path)) => path,
            Ok(None) => {
                return SnowShotLaunchResult::not_installed(
                    "未检测到 Snow Shot，请打开官方安装页。",
                );
            }
            Err(error) => return SnowShotLaunchResult::error(error),
        };

        let mcp_executable = executable
            .parent()
            .map(|directory| directory.join(SNOW_SHOT_MCP_EXECUTABLE));
        let Some(mcp_executable) = mcp_executable.filter(|path| path_is_file(path)) else {
            return SnowShotLaunchResult::error(
                "Snow Shot 已安装，但未找到随包提供的 MCP 桥。请更新 Snow Shot 后重试。",
            );
        };

        match call_snow_shot_capture(&mcp_executable) {
            Ok(()) => SnowShotLaunchResult::launched(&executable),
            Err(error) => SnowShotLaunchResult::error(error),
        }
    })
    .await;

    match result {
        Ok(result) => result,
        Err(error) => SnowShotLaunchResult::error(format!("Snow Shot 后台操作失败：{error}")),
    }
}

/// Open Snow Shot's official Windows download page.
#[tauri::command]
pub async fn open_snow_shot_install_page() -> Result<(), String> {
    open_install_page()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX_EPOCH")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("easyflowhub-snow-shot-{label}-{unique}"));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        directory
    }

    #[test]
    fn custom_path_has_priority_over_common_and_path_candidates() {
        let directory = temporary_directory("custom-priority");
        let custom = directory.join("custom.exe");
        let common = directory.join("common.exe");
        let path_candidate = directory.join("Snow Shot.exe");
        fs::write(&custom, b"custom").unwrap();
        fs::write(&common, b"common").unwrap();
        fs::write(&path_candidate, b"path").unwrap();

        let resolved = resolve_with_candidates(
            Some(custom.to_str().unwrap()),
            std::slice::from_ref(&common),
            std::slice::from_ref(&directory),
        )
        .unwrap();
        assert_eq!(resolved, Some(custom));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_custom_path_does_not_fall_back_to_other_candidates() {
        let directory = temporary_directory("custom-invalid");
        let fallback = directory.join("Snow Shot.exe");
        fs::write(&fallback, b"fallback").unwrap();

        let result = resolve_with_candidates(
            Some(directory.join("missing.exe").to_str().unwrap()),
            &[],
            std::slice::from_ref(&directory),
        );
        assert!(result.is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn common_location_is_checked_before_path() {
        let directory = temporary_directory("common-before-path");
        let common = directory.join("common.exe");
        let path_candidate = directory.join("SnowShot.exe");
        fs::write(&common, b"common").unwrap();
        fs::write(&path_candidate, b"path").unwrap();

        let resolved = resolve_with_candidates(
            None,
            std::slice::from_ref(&common),
            std::slice::from_ref(&directory),
        )
        .unwrap();
        assert_eq!(resolved, Some(common));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn path_lookup_accepts_snow_shot_executable_name_variants() {
        let directory = temporary_directory("path-variants");
        let candidate = directory.join("snow_shot.exe");
        fs::write(&candidate, b"snow-shot").unwrap();

        let resolved = resolve_with_candidates(None, &[], std::slice::from_ref(&directory)).unwrap();
        let resolved = resolved.expect("PATH candidate should resolve");
        assert_eq!(fs::read(resolved).unwrap(), b"snow-shot");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn parses_registry_display_icon_with_quotes_and_icon_index() {
        let path = parse_display_icon_path(r#""D:\SnowShot\bin\\snow_shot.exe",0"#)
            .expect("registry display icon should contain an executable path");
        assert_eq!(path.file_name().and_then(|name| name.to_str()), Some("snow_shot.exe"));
    }

    #[test]
    fn registered_install_location_finds_snow_shot_in_bin_directory() {
        let directory = temporary_directory("registered-bin");
        let bin = directory.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let executable = bin.join("snow_shot.exe");
        fs::write(&executable, b"snow-shot").unwrap();

        let candidates = candidates_from_snow_shot_registration(
            "Snow Shot",
            None,
            directory.to_str(),
        );
        let resolved = resolve_with_candidates(None, &candidates, &[]).unwrap();
        assert_eq!(resolved, Some(executable));

        let unrelated = candidates_from_snow_shot_registration("Other App", None, directory.to_str());
        assert!(unrelated.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn unavailable_mcp_status_prompts_to_enable_integration() {
        let response = json!({
            "result": {
                "structuredContent": {
                    "reachable": false,
                    "mcp_enabled": null
                }
            }
        });

        let message = unavailable_mcp_message(&response).unwrap();
        assert!(message.contains("设置 → 系统"));
        assert!(message.contains("MCP integration"));
    }

    #[test]
    fn disabled_mcp_status_prompts_to_enable_integration() {
        let response = json!({
            "result": {
                "structuredContent": {
                    "reachable": true,
                    "mcp_enabled": false
                }
            }
        });

        assert!(unavailable_mcp_message(&response).is_some());
    }

    #[test]
    fn mcp_tool_errors_extract_server_message() {
        let response = json!({
            "result": {
                "isError": true,
                "content": [{
                    "type": "text",
                    "text": "{\"error\":{\"message\":\"screenshot busy\"}}"
                }]
            }
        });

        assert_eq!(mcp_response_error(&response).as_deref(), Some("screenshot busy"));
    }

    #[test]
    fn mcp_tool_errors_without_text_are_not_reported_as_success() {
        let response = json!({
            "result": { "isError": true, "content": [] }
        });
        assert!(mcp_response_error(&response).is_some());
    }

    #[test]
    fn malformed_mcp_responses_are_not_reported_as_success() {
        assert!(mcp_response_error(&json!({ "jsonrpc": "2.0", "id": 4 })).is_some());
    }

    #[test]
    fn launch_result_uses_frontend_camel_case_fields() {
        let result = SnowShotLaunchResult::not_installed("missing");
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["status"], "not_installed");
        assert!(value.get("installUrl").is_some());
        assert!(value.get("install_url").is_none());
    }
}
