//! Small, user initiated URL download queue.
//!
//! The queue intentionally lives in the native shell.  It is not a general
//! download manager: records are persisted as metadata, while active workers
//! are ordinary Rust threads and are resumed/retried explicitly by the user.
//! A `.part` file is kept for failed downloads so retry can continue with a
//! range request where the server supports it.  No work starts at startup.

use chrono::Utc;
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_RANGE, RANGE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;
use percent_encoding::percent_decode_str;

const MAX_URL_LENGTH: usize = 8_192;
const MAX_FILE_NAME_LENGTH: usize = 240;
const MAX_RECORDS: usize = 100;
const CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    pub file_name: Option<String>,
    #[serde(default)]
    pub target_dir: Option<String>,
    pub target_path: Option<String>,
    pub status: DownloadStatus,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
    pub retry_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSettings {
    // Keep the wire shape aligned with the TypeScript bridge.  `directory`
    // was used by an early draft; accepting it as an alias keeps existing
    // config files readable while all new responses use `targetDir`.
    #[serde(rename = "targetDir", alias = "directory")]
    pub target_dir: Option<String>,
    pub conflict_policy: String,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            target_dir: None,
            conflict_policy: "rename".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedDownloads {
    #[serde(default)]
    settings: DownloadSettings,
    #[serde(default)]
    records: Vec<DownloadRecord>,
}

#[derive(Debug, Default)]
struct DownloadState {
    initialized: bool,
    settings: DownloadSettings,
    records: HashMap<String, DownloadRecord>,
    controls: HashMap<String, Arc<DownloadControl>>,
}

#[derive(Debug, Default)]
struct DownloadControl {
    paused: AtomicBool,
    cancelled: AtomicBool,
}

static STATE: OnceLock<Mutex<DownloadState>> = OnceLock::new();

fn state() -> &'static Mutex<DownloadState> {
    STATE.get_or_init(|| Mutex::new(DownloadState::default()))
}

fn lock_state() -> Result<std::sync::MutexGuard<'static, DownloadState>, String> {
    state()
        .lock()
        .map_err(|_| "下载队列状态已损坏，请重启应用".to_string())
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("获取下载配置目录失败: {error}"))?;
    fs::create_dir_all(&directory).map_err(|error| format!("创建下载配置目录失败: {error}"))?;
    Ok(directory.join("downloads.json"))
}

fn persist_locked(app: &AppHandle, value: &DownloadState) -> Result<(), String> {
    let path = config_path(app)?;
    let mut records = value.records.values().cloned().collect::<Vec<_>>();
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    records.truncate(MAX_RECORDS);
    let data = serde_json::to_vec_pretty(&PersistedDownloads {
        settings: value.settings.clone(),
        records,
    })
    .map_err(|error| format!("序列化下载状态失败: {error}"))?;
    let temporary = path.with_extension(format!("{}.pending", Uuid::new_v4()));
    fs::write(&temporary, data).map_err(|error| format!("保存下载状态失败: {error}"))?;
    if let Err(error) = fs::rename(&temporary, &path) {
        // Windows cannot rename over an existing file.  Only remove the old
        // file for that specific collision; permission, I/O and other errors
        // must leave the last known-good metadata intact.
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            let _ = fs::remove_file(&temporary);
            return Err(format!("保存下载状态失败: {error}"));
        }
        fs::remove_file(&path).map_err(|remove_error| format!("替换下载状态失败: {remove_error}"))?;
        fs::rename(&temporary, &path).map_err(|rename_error| format!("保存下载状态失败: {rename_error}"))?;
    }
    Ok(())
}

fn ensure_initialized(app: &AppHandle) -> Result<(), String> {
    let path = config_path(app)?;
    let mut value = lock_state()?;
    if value.initialized {
        return Ok(());
    }
    if let Ok(data) = fs::read(path) {
        if let Ok(saved) = serde_json::from_slice::<PersistedDownloads>(&data) {
            value.settings = saved.settings;
            for mut record in saved.records {
                if matches!(record.status, DownloadStatus::Queued | DownloadStatus::Downloading | DownloadStatus::Paused) {
                    record.status = DownloadStatus::Failed;
                    record.error = Some("应用重启后下载未继续，请点击重试".to_string());
                    record.updated_at = Utc::now().to_rfc3339();
                }
                value.records.insert(record.id.clone(), record);
            }
        }
    }
    value.settings = normalize_settings(value.settings.clone())?;
    value.initialized = true;
    // Persist normalized settings and restart recovery state best-effort.
    let _ = persist_locked(app, &value);
    Ok(())
}

fn normalize_policy(policy: &str) -> Result<String, String> {
    match policy.trim().to_ascii_lowercase().as_str() {
        "fail" | "rename" | "overwrite" => Ok(policy.trim().to_ascii_lowercase()),
        _ => Err("冲突策略必须是 fail、rename 或 overwrite".to_string()),
    }
}

fn normalize_settings(mut settings: DownloadSettings) -> Result<DownloadSettings, String> {
    settings.conflict_policy = normalize_policy(&settings.conflict_policy)?;
    if let Some(directory) = settings.target_dir.take() {
        let path = validate_target_dir(&directory)?;
        settings.target_dir = Some(path.to_string_lossy().into_owned());
    }
    Ok(settings)
}

fn validate_target_dir(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value.trim());
    if value.trim().is_empty() {
        return Err("下载目录不能为空".to_string());
    }
    let path = path
        .canonicalize()
        .map_err(|error| format!("下载目录无法访问: {error}"))?;
    if !path.is_dir() {
        return Err("下载目录必须是文件夹".to_string());
    }
    Ok(path)
}

/// Validate a persisted destination before using it for a retry.  A record is
/// user data and can outlive a settings change, so the target path must not be
/// trusted merely because it was written by an earlier process.  Requiring an
/// absolute path and a canonical parent inside the selected directory also
/// prevents a crafted record from redirecting a worker outside the download
/// root (including through `..` components).
fn validate_existing_target(value: &str, root: &Path) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("下载目标路径必须是绝对路径".to_string());
    }
    let parent = path.parent().ok_or_else(|| "下载目标路径无效".to_string())?;
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("下载目录无法访问: {error}"))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| format!("下载目标目录无法访问: {error}"))?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err("下载目标路径超出下载目录".to_string());
    }
    if path.exists() {
        let canonical_path = path
            .canonicalize()
            .map_err(|error| format!("下载目标路径无法访问: {error}"))?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err("下载目标路径超出下载目录".to_string());
        }
    }
    Ok(path)
}

fn default_target_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = app.path().download_dir() {
        if fs::create_dir_all(&path).is_ok() && path.is_dir() {
            return Ok(path);
        }
    }
    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取默认下载目录失败: {error}"))?
        .join("downloads");
    fs::create_dir_all(&path).map_err(|error| format!("创建默认下载目录失败: {error}"))?;
    Ok(path)
}

fn validate_url(raw: &str) -> Result<Url, String> {
    if raw.trim().len() > MAX_URL_LENGTH {
        return Err("下载地址过长".to_string());
    }
    let parsed = Url::parse(raw.trim()).map_err(|_| "下载地址格式无效".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("仅支持带主机名的 HTTP(S) 下载地址".to_string());
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return Err("下载地址不能包含用户名或密码".to_string());
    }
    Ok(parsed)
}

fn sanitize_file_name(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || value.len() > MAX_FILE_NAME_LENGTH {
        return None;
    }
    let value = value
        .trim_matches(['.', ' '])
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    if value.is_empty() || value == "." || value == ".." {
        return None;
    }
    // Windows device names remain special even when used with an extension
    // (for example `CON.txt`).  Reject them before they reach File::create.
    let stem = value.split('.').next().unwrap_or_default().to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
    {
        return None;
    }
    Some(value)
}

fn requested_file_name(value: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = value else { return Ok(None) };
    if raw.trim().is_empty() {
        return Ok(None);
    }
    sanitize_file_name(&raw)
        .map(Some)
        .ok_or_else(|| "文件名无效或超出长度限制".to_string())
}

fn file_name_from_url(url: &Url) -> String {
    url.path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .and_then(decode_and_sanitize_file_name)
        .unwrap_or_else(|| "download.bin".to_string())
}

/// Decode a percent-encoded filename supplied by a URL or RFC 5987
/// Content-Disposition parameter before applying the Windows filename rules.
/// Invalid UTF-8 is rejected instead of allowing a lossy replacement to reach
/// the filesystem.
fn decode_and_sanitize_file_name(raw: &str) -> Option<String> {
    let decoded = percent_decode_str(raw).decode_utf8().ok()?;
    sanitize_file_name(decoded.as_ref())
}

fn content_disposition_filename(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_matches('"');
    // RFC 5987 uses charset'language'value.  We only decode UTF-8; unknown
    // charsets stay on the conservative regular filename path below.
    let encoded = {
        let mut fields = raw.splitn(3, '\'');
        match (fields.next(), fields.next(), fields.next()) {
            (Some(charset), Some(_language), Some(value))
                if charset.eq_ignore_ascii_case("utf-8") => value,
            _ => raw,
        }
    };
    decode_and_sanitize_file_name(encoded)
}

fn file_name_from_content_disposition(response: &Response) -> Option<String> {
    let value = response.headers().get(CONTENT_DISPOSITION)?.to_str().ok()?;
    for part in value.split(';').map(str::trim) {
        let lower = part.to_ascii_lowercase();
        if lower.starts_with("filename*=") {
            let raw = part.split_once('=')?.1.trim().trim_matches('"');
            if let Some(name) = content_disposition_filename(raw) {
                return Some(name);
            }
        } else if lower.starts_with("filename=") {
            if let Some(name) = sanitize_file_name(part.split_once('=')?.1.trim().trim_matches('"')) {
                return Some(name);
            }
        }
    }
    None
}

fn unique_destination(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("download");
    let extension = path.extension().and_then(|value| value.to_str());
    for index in 1..10_000 {
        let name = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = path.with_file_name(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

fn apply_conflict_policy(destination: PathBuf, policy: &str) -> Result<PathBuf, String> {
    match policy {
        "fail" if destination.exists() => Err("目标文件已存在".to_string()),
        "rename" => Ok(unique_destination(&destination)),
        "overwrite" => Ok(destination),
        _ => Err("冲突策略无效".to_string()),
    }
}

fn set_record<F>(app: &AppHandle, id: &str, change: F) -> Result<Option<DownloadRecord>, String>
where
    F: FnOnce(&mut DownloadRecord),
{
    let mut value = lock_state()?;
    let (previous, snapshot) = {
        let Some(record) = value.records.get_mut(id) else { return Ok(None) };
        let previous = record.clone();
        change(record);
        record.updated_at = Utc::now().to_rfc3339();
        (previous, record.clone())
    };
    if let Err(error) = persist_locked(app, &value) {
        if let Some(record) = value.records.get_mut(id) {
            *record = previous;
        }
        return Err(error);
    }
    Ok(Some(snapshot))
}

fn get_record(app: &AppHandle, id: &str) -> Result<DownloadRecord, String> {
    ensure_initialized(app)?;
    lock_state()?.records.get(id).cloned().ok_or_else(|| "下载任务不存在".to_string())
}

fn part_path(destination: &Path, id: &str) -> PathBuf {
    destination.with_file_name(format!(".{}.{}.part", destination.file_name().and_then(|v| v.to_str()).unwrap_or("download"), id))
}

fn wait_while_paused(control: &DownloadControl) -> Result<(), String> {
    while control.paused.load(Ordering::Relaxed) {
        if control.cancelled.load(Ordering::Relaxed) {
            return Err("下载已取消".to_string());
        }
        thread::sleep(Duration::from_millis(100));
    }
    if control.cancelled.load(Ordering::Relaxed) {
        return Err("下载已取消".to_string());
    }
    Ok(())
}

/// Stream a response into the task's `.part` file. This keeps the file and
/// cancellation boundary independent from Tauri so native tests can exercise
/// successful, partial, and cancelled transfers without a desktop runtime.
fn stream_response_to_part(
    response: &mut Response,
    part: &Path,
    offset: u64,
    total: Option<u64>,
    control: &DownloadControl,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<u64, String> {
    let mut output = if offset > 0 {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(part)
            .map_err(|error| format!("打开临时文件失败: {error}"))?
    } else {
        File::create(part).map_err(|error| format!("创建临时文件失败: {error}"))?
    };
    let mut downloaded = offset;
    let mut buffer = vec![0u8; CHUNK_SIZE];
    loop {
        wait_while_paused(control)?;
        let read = response
            .read(&mut buffer)
            .map_err(|error| format!("读取下载内容失败: {error}"))?;
        if read == 0 {
            break;
        }
        if control.cancelled.load(Ordering::Relaxed) {
            return Err("下载已取消".to_string());
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| format!("写入下载文件失败: {error}"))?;
        downloaded = downloaded.saturating_add(read as u64);
        on_progress(downloaded, total);
    }
    if control.cancelled.load(Ordering::Relaxed) {
        return Err("下载已取消".to_string());
    }
    output
        .flush()
        .map_err(|error| format!("刷新下载文件失败: {error}"))?;
    if let Some(total) = total {
        if downloaded < total {
            return Err("下载内容不完整".to_string());
        }
    }
    Ok(downloaded)
}

fn update_progress(app: &AppHandle, id: &str, downloaded: u64, total: Option<u64>) {
    let _ = set_record(app, id, |record| {
        record.downloaded_bytes = downloaded;
        record.total_bytes = total.or(record.total_bytes);
    });
}

fn open_response(client: &Client, url: &str, offset: u64) -> Result<Response, String> {
    let mut request = client.get(url);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    request.send().map_err(|error| format!("连接下载地址失败: {error}"))
}

fn response_starts_at(response: &Response, offset: u64) -> bool {
    if offset == 0 {
        return true;
    }
    response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|value| content_range_starts_at(value, offset))
        .unwrap_or(false)
}

fn content_range_starts_at(value: &str, offset: u64) -> bool {
    let Some(range) = value.strip_prefix("bytes ") else { return false };
    let Some((start, _)) = range.split_once('-') else { return false };
    start.parse::<u64>().ok() == Some(offset)
}

fn run_worker(app: AppHandle, id: String, url: String, requested_name: Option<String>, target_dir: Option<String>, policy: String, control: Arc<DownloadControl>) {
    let result = download_worker(&app, &id, &url, requested_name.as_deref(), target_dir.as_deref(), &policy, &control);
    let is_current = lock_state()
        .map(|value| value.controls.get(&id).map(|current| Arc::ptr_eq(current, &control)).unwrap_or(false))
        .unwrap_or(false);
    if !is_current {
        return;
    }
    match result {
        Ok(()) => {
            // Once the atomic final rename has succeeded the download is
            // complete.  A late cancel click must not hide a valid file by
            // rewriting the terminal state to `cancelled`.
            let _ = set_record(&app, &id, |record| {
                record.status = DownloadStatus::Completed;
                record.error = None;
                if let Some(total) = record.total_bytes {
                    record.downloaded_bytes = total;
                }
            });
        }
        Err(error) if error == "下载已取消" || control.cancelled.load(Ordering::Relaxed) => {
            let _ = set_record(&app, &id, |record| {
                record.status = DownloadStatus::Cancelled;
                record.error = None;
            });
        }
        Err(error) => {
            let _ = set_record(&app, &id, |record| {
                record.status = DownloadStatus::Failed;
                record.error = Some(error);
            });
        }
    }
    if let Ok(mut value) = lock_state() {
        if value.controls.get(&id).map(|current| Arc::ptr_eq(current, &control)).unwrap_or(false) {
            value.controls.remove(&id);
        }
        let _ = persist_locked(&app, &value);
    }
}

fn download_worker(app: &AppHandle, id: &str, url: &str, requested_name: Option<&str>, requested_dir: Option<&str>, policy: &str, control: &DownloadControl) -> Result<(), String> {
    if control.cancelled.load(Ordering::Relaxed) {
        return Err("下载已取消".to_string());
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .user_agent("EasyFlowHub/0.2")
        .build()
        .map_err(|error| format!("初始化下载器失败: {error}"))?;
    let parsed = validate_url(url)?;
    let (target_dir, existing_target) = {
        let value = lock_state()?;
        let directory = match requested_dir.or(value.settings.target_dir.as_deref()) {
            Some(path) => validate_target_dir(path)?,
            None => default_target_dir(app)?,
        };
        let record = value.records.get(id).ok_or_else(|| "下载任务不存在".to_string())?;
        let existing_target = record
            .target_path
            .as_deref()
            .map(|path| validate_existing_target(path, &directory))
            .transpose()?;
        (directory, existing_target.map(|path| path.to_string_lossy().into_owned()))
    };

    let mut destination = existing_target.clone().map(PathBuf::from).unwrap_or_else(|| {
        target_dir.join(sanitize_file_name(requested_name.unwrap_or_default()).unwrap_or_else(|| file_name_from_url(&parsed)))
    });

    let initial_destination = destination.clone();
    let prior_part = part_path(&destination, id);
    let mut offset = fs::metadata(&prior_part).map(|metadata| metadata.len()).unwrap_or(0);
    let mut response = open_response(&client, url, offset)?;
    if offset > 0
        && (response.status() != reqwest::StatusCode::PARTIAL_CONTENT || !response_starts_at(&response, offset))
    {
        offset = 0;
        response = open_response(&client, url, 0)?;
    }
    if !response.status().is_success() {
        return Err(format!("下载服务器返回 HTTP {}", response.status().as_u16()));
    }

    if existing_target.is_none() {
        let response_name = file_name_from_content_disposition(&response);
        if requested_name.is_none() {
            destination = target_dir.join(response_name.unwrap_or_else(|| file_name_from_url(&parsed)));
        }
        // A server-provided filename changes the `.part` path. Never append
        // a ranged response fetched for the old path to the new file.
        if destination != initial_destination && offset > 0 {
            offset = 0;
            response = open_response(&client, url, 0)?;
            if !response.status().is_success() {
                return Err(format!("下载服务器返回 HTTP {}", response.status().as_u16()));
            }
        }
        destination = apply_conflict_policy(destination, policy)?;
    }
    let part = part_path(&destination, id);
    if offset > 0 && part != prior_part {
        offset = 0;
    }
    if control.cancelled.load(Ordering::Relaxed) {
        return Err("下载已取消".to_string());
    }
    let initial_status = if control.paused.load(Ordering::Relaxed) {
        DownloadStatus::Paused
    } else {
        DownloadStatus::Downloading
    };
    let total = response.content_length().map(|length| length.saturating_add(offset));
    let _ = set_record(app, id, |record| {
        // A pause/cancel request can arrive while the worker is resolving
        // headers. Preserve that user intent instead of overwriting it.
        if !matches!(record.status, DownloadStatus::Cancelled) {
            record.status = initial_status;
        }
        record.target_path = Some(destination.to_string_lossy().into_owned());
        record.file_name = destination.file_name().and_then(|v| v.to_str()).map(str::to_string);
        record.downloaded_bytes = offset;
        record.total_bytes = total;
        record.error = None;
    });

    let _downloaded = stream_response_to_part(
        &mut response,
        &part,
        offset,
        total,
        control,
        |downloaded, total| update_progress(app, id, downloaded, total),
    )?;
    if policy == "overwrite" && destination.exists() {
        fs::remove_file(&destination).map_err(|error| format!("替换目标文件失败: {error}"))?;
    }
    if control.cancelled.load(Ordering::Relaxed) {
        return Err("下载已取消".to_string());
    }
    fs::rename(&part, &destination).map_err(|error| format!("保存下载文件失败: {error}"))?;
    Ok(())
}

fn spawn_worker(app: AppHandle, record: DownloadRecord, control: Arc<DownloadControl>, policy: String) {
    thread::spawn(move || run_worker(app, record.id, record.url, record.file_name, record.target_dir, policy, control));
}

#[tauri::command]
pub fn download_get_settings(app: AppHandle) -> Result<DownloadSettings, String> {
    ensure_initialized(&app)?;
    Ok(lock_state()?.settings.clone())
}

#[tauri::command]
pub fn download_update_settings(app: AppHandle, settings: DownloadSettings) -> Result<DownloadSettings, String> {
    ensure_initialized(&app)?;
    let settings = normalize_settings(settings)?;
    let mut value = lock_state()?;
    let previous = value.settings.clone();
    value.settings = settings.clone();
    if let Err(error) = persist_locked(&app, &value) {
        value.settings = previous;
        return Err(error);
    }
    Ok(settings)
}

#[tauri::command]
pub fn download_list(app: AppHandle) -> Result<Vec<DownloadRecord>, String> {
    ensure_initialized(&app)?;
    let mut records = lock_state()?.records.values().cloned().collect::<Vec<_>>();
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(records)
}

#[tauri::command]
pub fn download_enqueue(app: AppHandle, url: String, file_name: Option<String>, target_dir: Option<String>, conflict_policy: Option<String>) -> Result<DownloadRecord, String> {
    ensure_initialized(&app)?;
    let parsed = validate_url(&url)?;
    let requested_name = requested_file_name(file_name)?;
    let default_policy = lock_state()?.settings.conflict_policy.clone();
    let policy = normalize_policy(conflict_policy.as_deref().unwrap_or(&default_policy))?;
    let directory = match target_dir {
        Some(path) => validate_target_dir(&path)?,
        None => {
            let value = lock_state()?;
            match value.settings.target_dir.as_deref() { Some(path) => validate_target_dir(path)?, None => default_target_dir(&app)? }
        }
    };
    let now = Utc::now().to_rfc3339();
    let record = DownloadRecord {
        id: Uuid::new_v4().to_string(),
        url: parsed.to_string(),
        file_name: requested_name,
        target_dir: Some(directory.to_string_lossy().into_owned()),
        target_path: None,
        status: DownloadStatus::Queued,
        downloaded_bytes: 0,
        total_bytes: None,
        error: None,
        retry_count: 0,
        created_at: now.clone(),
        updated_at: now,
    };
    let control = Arc::new(DownloadControl::default());
    {
        let mut value = lock_state()?;
        value.records.insert(record.id.clone(), record.clone());
        value.controls.insert(record.id.clone(), control.clone());
        if let Err(error) = persist_locked(&app, &value) {
            value.records.remove(&record.id);
            value.controls.remove(&record.id);
            return Err(error);
        }
    }
    spawn_worker(app, record.clone(), control, policy);
    Ok(record)
}

fn control_for(app: &AppHandle, id: &str) -> Result<(DownloadRecord, Arc<DownloadControl>), String> {
    ensure_initialized(app)?;
    let value = lock_state()?;
    let record = value.records.get(id).cloned().ok_or_else(|| "下载任务不存在".to_string())?;
    let control = value.controls.get(id).cloned().ok_or_else(|| "下载任务未在运行".to_string())?;
    Ok((record, control))
}

#[tauri::command]
pub fn download_pause(app: AppHandle, id: String) -> Result<DownloadRecord, String> {
    let (record, control) = control_for(&app, &id)?;
    if !matches!(record.status, DownloadStatus::Downloading | DownloadStatus::Queued) { return Err("当前下载不能暂停".to_string()); }
    control.paused.store(true, Ordering::Relaxed);
    set_record(&app, &id, |value| value.status = DownloadStatus::Paused)?.ok_or_else(|| "下载任务不存在".to_string())
}

#[tauri::command]
pub fn download_resume(app: AppHandle, id: String) -> Result<DownloadRecord, String> {
    let (record, control) = control_for(&app, &id)?;
    if !matches!(record.status, DownloadStatus::Paused) { return Err("当前下载未暂停".to_string()); }
    control.paused.store(false, Ordering::Relaxed);
    set_record(&app, &id, |value| value.status = DownloadStatus::Downloading)?.ok_or_else(|| "下载任务不存在".to_string())
}

#[tauri::command]
pub fn download_cancel(app: AppHandle, id: String) -> Result<DownloadRecord, String> {
    let (record, control) = control_for(&app, &id)?;
    if !matches!(record.status, DownloadStatus::Downloading | DownloadStatus::Queued | DownloadStatus::Paused) {
        return Err("当前下载不能取消".to_string());
    }
    control.cancelled.store(true, Ordering::Relaxed);
    set_record(&app, &id, |value| value.status = DownloadStatus::Cancelled)?.ok_or_else(|| "下载任务不存在".to_string())
}

#[tauri::command]
pub fn download_retry(app: AppHandle, id: String) -> Result<DownloadRecord, String> {
    ensure_initialized(&app)?;
    let (record, control) = {
        let mut value = lock_state()?;
        if value.controls.contains_key(&id) {
            return Err("下载任务仍在结束中，请稍后重试".to_string());
        }
        let record = value.records.get_mut(&id).ok_or_else(|| "下载任务不存在".to_string())?;
        if !matches!(record.status, DownloadStatus::Failed | DownloadStatus::Cancelled) { return Err("只有失败或已取消的下载可以重试".to_string()); }
        let previous = record.clone();
        record.status = DownloadStatus::Queued;
        record.error = None;
        record.retry_count = record.retry_count.saturating_add(1);
        record.updated_at = Utc::now().to_rfc3339();
        let snapshot = record.clone();
        let control = Arc::new(DownloadControl::default());
        value.controls.insert(id.clone(), control.clone());
        if let Err(error) = persist_locked(&app, &value) {
            value.records.insert(id.clone(), previous);
            value.controls.remove(&id);
            return Err(error);
        }
        (snapshot, control)
    };
    let policy = lock_state()?.settings.conflict_policy.clone();
    spawn_worker(app, record.clone(), control, policy);
    Ok(record)
}

fn terminal_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let record = get_record(app, id)?;
    if !matches!(record.status, DownloadStatus::Completed) || record.target_path.is_none() { return Err("下载文件尚未完成".to_string()); }
    let root = record
        .target_dir
        .as_deref()
        .ok_or_else(|| "下载目录信息缺失".to_string())
        .and_then(validate_target_dir)?;
    let path = PathBuf::from(record.target_path.unwrap());
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("下载文件无法访问: {error}"))?;
    if !canonical.starts_with(&root) || !canonical.is_file() {
        return Err("下载文件路径无效".to_string());
    }
    Ok(canonical)
}

#[tauri::command]
pub fn download_open(app: AppHandle, id: String) -> Result<(), String> {
    let path = terminal_path(&app, &id)?;
    app.opener().open_path(path.to_string_lossy().to_string(), None::<String>).map_err(|error| format!("打开下载文件失败: {error}"))
}

#[tauri::command]
pub fn download_reveal(app: AppHandle, id: String) -> Result<String, String> {
    let path = terminal_path(&app, &id)?;
    app.opener().reveal_item_in_dir(&path).map_err(|error| format!("定位下载文件失败: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn download_copy_path(app: AppHandle, id: String) -> Result<String, String> {
    Ok(terminal_path(&app, &id)?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn local_response(body: &'static [u8]) -> Response {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local test server");
        let address = listener.local_addr().expect("read local test address");
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept test request");
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("write test response headers");
            stream.write_all(body).expect("write test response body");
        });
        Client::builder()
            .build()
            .expect("build test client")
            .get(format!("http://{address}/sample.bin"))
            .send()
            .expect("request test response")
    }

    #[test]
    fn url_validation_only_allows_http_and_https_without_credentials() {
        assert!(validate_url("https://example.com/a").is_ok());
        assert!(validate_url("ftp://example.com/a").is_err());
        assert!(validate_url("https://user:pass@example.com/a").is_err());
        assert!(validate_url("not a url").is_err());
    }

    #[test]
    fn file_name_falls_back_and_sanitizes_path_components() {
        assert_eq!(file_name_from_url(&Url::parse("https://example.com/a/file.txt").unwrap()), "file.txt");
        assert_eq!(file_name_from_url(&Url::parse("https://example.com/a/report%20%E6%B5%8B.txt").unwrap()), "report 测.txt");
        assert_eq!(file_name_from_url(&Url::parse("https://example.com/").unwrap()), "download.bin");
        assert_eq!(sanitize_file_name("../a:b?.txt").unwrap(), "_a_b_.txt");
        assert!(sanitize_file_name("CON.txt").is_none());
        assert!(sanitize_file_name("COM1.txt").is_none());
        assert!(sanitize_file_name("COM0.txt").is_some());
        assert!(requested_file_name(Some("x".repeat(MAX_FILE_NAME_LENGTH + 1))).is_err());
    }

    #[test]
    fn content_disposition_filename_decodes_rfc5987_utf8() {
        assert_eq!(
            content_disposition_filename("UTF-8''report%20%E6%B5%8B.txt"),
            Some("report 测.txt".to_string())
        );
        assert_eq!(
            content_disposition_filename("utf-8'en'%E2%9C%93.zip"),
            Some("✓.zip".to_string())
        );
        assert!(content_disposition_filename("UTF-8''%FF.txt").is_none());
    }

    #[test]
    fn unique_destination_never_overwrites_existing_file() {
        let root = std::env::temp_dir().join(format!("easyflowhub-download-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let first = root.join("file.txt");
        fs::write(&first, b"one").unwrap();
        let second = unique_destination(&first);
        assert_eq!(second.file_name().unwrap(), "file (1).txt");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persisted_target_must_stay_inside_download_root() {
        let root = std::env::temp_dir().join(format!("easyflowhub-download-root-{}", Uuid::new_v4()));
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let valid = validate_existing_target(&nested.join("file.zip").to_string_lossy(), &root).unwrap();
        assert_eq!(valid, nested.join("file.zip"));
        let outside = std::env::temp_dir().join(format!("easyflowhub-download-outside-{}", Uuid::new_v4()));
        fs::create_dir_all(&outside).unwrap();
        assert!(validate_existing_target(&outside.join("file.zip").to_string_lossy(), &root).is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn control_flags_cover_pause_resume_and_cancel() {
        let control = DownloadControl::default();
        assert!(wait_while_paused(&control).is_ok());
        control.paused.store(true, Ordering::Relaxed);
        control.paused.store(false, Ordering::Relaxed);
        control.cancelled.store(true, Ordering::Relaxed);
        assert!(wait_while_paused(&control).is_err());
    }

    #[test]
    fn stream_response_writes_part_and_reports_success() {
        let root = std::env::temp_dir().join(format!("easyflowhub-download-stream-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create stream test root");
        let part = root.join("sample.part");
        let mut response = local_response(b"download body");
        let control = DownloadControl::default();
        let mut progress = Vec::new();
        let downloaded = stream_response_to_part(&mut response, &part, 0, Some(13), &control, |value, _| progress.push(value))
            .expect("stream should succeed");
        assert_eq!(downloaded, 13);
        assert_eq!(fs::read(&part).expect("read part"), b"download body");
        assert!(!progress.is_empty());
        fs::remove_dir_all(root).expect("remove stream test root");
    }

    #[test]
    fn cancelled_stream_keeps_part_boundary_without_writing_final_file() {
        let root = std::env::temp_dir().join(format!("easyflowhub-download-cancel-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create cancel test root");
        let part = root.join("sample.part");
        let mut response = local_response(b"download body");
        let control = DownloadControl::default();
        control.cancelled.store(true, Ordering::Relaxed);
        let error = stream_response_to_part(&mut response, &part, 0, Some(13), &control, |_, _| {})
            .expect_err("cancelled stream should fail");
        assert_eq!(error, "下载已取消");
        assert!(part.exists());
        assert!(!root.join("sample.bin").exists());
        fs::remove_dir_all(root).expect("remove cancel test root");
    }

    #[test]
    fn conflict_policy_covers_fail_rename_and_overwrite() {
        let root = std::env::temp_dir().join(format!("easyflowhub-download-conflict-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create conflict test root");
        let destination = root.join("sample.bin");
        fs::write(&destination, b"existing").expect("write existing destination");
        assert!(apply_conflict_policy(destination.clone(), "fail").is_err());
        assert_eq!(apply_conflict_policy(destination.clone(), "rename").expect("rename policy"), root.join("sample (1).bin"));
        assert_eq!(apply_conflict_policy(destination.clone(), "overwrite").expect("overwrite policy"), destination);
        fs::remove_dir_all(root).expect("remove conflict test root");
    }

    #[test]
    fn status_serializes_for_typed_bridge() {
        assert_eq!(serde_json::to_string(&DownloadStatus::Downloading).unwrap(), "\"downloading\"");
        let settings = DownloadSettings { target_dir: Some("D:\\Downloads".to_string()), conflict_policy: "rename".to_string() };
        let encoded = serde_json::to_value(&settings).unwrap();
        assert_eq!(encoded.get("targetDir").and_then(|value| value.as_str()), Some("D:\\Downloads"));
        assert!(encoded.get("directory").is_none());
        assert_eq!(normalize_policy("OVERWRITE").unwrap(), "overwrite");
        assert!(normalize_policy("replace").is_err());
    }

    #[test]
    fn ranged_response_must_report_the_requested_start() {
        assert!(content_range_starts_at("bytes 1024-2047/4096", 1024));
        assert!(!content_range_starts_at("bytes 0-2047/4096", 1024));
        assert!(!content_range_starts_at("invalid", 1024));
    }
}
