//! Controlled, on-demand file search for the Windows-first file manager.
//!
//! The module deliberately keeps an in-memory index. A root is configured by
//! the user and the index is populated only by `file_search_refresh_index`;
//! there is no startup scan or watcher that could unexpectedly walk a disk.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;

const MAX_INDEX_ENTRIES: usize = 50_000;
const MAX_SCAN_DEPTH: usize = 64;
const MAX_TEXT_PREVIEW_BYTES: usize = 512 * 1024;
const MAX_IMAGE_PREVIEW_BYTES: usize = 4 * 1024 * 1024;
const MAX_RECENT_FILES: usize = 100;
const MAX_COPY_ENTRIES: usize = 50_000;
const MAX_COPY_DEPTH: usize = 64;
const MAX_COPY_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: String,
    pub path: String,
    pub name: String,
    pub extension: String,
    pub size: u64,
    pub modified_at: Option<String>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchStatus {
    pub generation: u64,
    pub paused: bool,
    pub root: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilesRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub extension: Option<String>,
    #[serde(default = "default_search_sort")]
    pub sort: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_sort() -> String {
    "relevance".to_string()
}

fn default_search_limit() -> usize {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilesResponse {
    pub items: Vec<FileEntry>,
    pub total: usize,
    pub generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub kind: String,
    pub path: String,
    pub content: Option<String>,
    pub mime_type: Option<String>,
    pub image_data_url: Option<String>,
    pub size: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileMutationResponse {
    pub ok: bool,
    pub path: String,
    pub generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationResponse {
    pub entry: FileEntry,
    pub generation: u64,
}

#[derive(Debug, Default, Clone)]
struct SearchState {
    root: Option<PathBuf>,
    paused: bool,
    generation: u64,
    entries: Vec<FileEntry>,
    initialized: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSearchConfig {
    root: Option<String>,
    paused: bool,
}

static SEARCH_STATE: OnceLock<Mutex<SearchState>> = OnceLock::new();

fn state() -> &'static Mutex<SearchState> {
    SEARCH_STATE.get_or_init(|| Mutex::new(SearchState::default()))
}

fn lock_state() -> Result<std::sync::MutexGuard<'static, SearchState>, String> {
    state()
        .lock()
        .map_err(|_| "文件索引状态已损坏，请重启应用".to_string())
}

fn status_locked(value: &SearchState) -> FileSearchStatus {
    FileSearchStatus {
        generation: value.generation,
        paused: value.paused,
        root: value.root.as_ref().map(|path| path.to_string_lossy().into_owned()),
        count: value.entries.len(),
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("获取应用配置目录失败: {error}"))?;
    fs::create_dir_all(&directory).map_err(|error| format!("创建应用配置目录失败: {error}"))?;
    Ok(directory.join("file-search.json"))
}

fn persist_config_values(app: &AppHandle, root: Option<&Path>, paused: bool) -> Result<(), String> {
    let path = config_path(app)?;
    let config = PersistedSearchConfig {
        root: root.map(|root| root.to_string_lossy().into_owned()),
        paused,
    };
    let serialized = serde_json::to_vec_pretty(&config)
        .map_err(|error| format!("序列化文件索引设置失败: {error}"))?;
    let temporary = path.with_extension(format!("{}.pending", uuid::Uuid::new_v4()));
    fs::write(&temporary, serialized).map_err(|error| format!("保存文件索引设置失败: {error}"))?;
    replace_file(&temporary, &path)
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
        let source_wide = source.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let destination_wide = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        unsafe {
            MoveFileExW(
                PCWSTR(source_wide.as_ptr()),
                PCWSTR(destination_wide.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|error| format!("更新文件索引设置失败: {error}"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        fs::rename(source, destination).map_err(|error| format!("更新文件索引设置失败: {error}"))
    }
}

fn ensure_initialized(app: &AppHandle) -> Result<(), String> {
    let path = config_path(app)?;
    let mut value = lock_state()?;
    if value.initialized {
        return Ok(());
    }
    if let Ok(bytes) = fs::read(path) {
        if let Ok(config) = serde_json::from_slice::<PersistedSearchConfig>(&bytes) {
            value.paused = config.paused;
            value.root = config.root.and_then(|root| validate_root(&root).ok());
        }
    }
    value.initialized = true;
    Ok(())
}

fn is_dangerous_root(path: &Path) -> bool {
    // A filesystem root is too broad for an app-owned index and makes an
    // accidental mutation especially dangerous.
    path.parent().is_none()
}

fn ensure_inside(root: &Path, candidate: &Path) -> Result<PathBuf, String> {
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("路径不存在或无法访问: {error}"))?;
    if !canonical.starts_with(root) {
        return Err("路径不在已配置的文件夹内".to_string());
    }
    Ok(canonical)
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn entry_from_path(path: &Path) -> Result<FileEntry, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("读取文件元数据失败: {error}"))?;
    let modified_at = metadata
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
        .map(|value| value.to_rfc3339());
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let path_string = path.to_string_lossy().into_owned();
    Ok(FileEntry {
        id: path_string.clone(),
        path: path_string,
        name,
        extension: extension(path),
        size: if metadata.is_dir() { 0 } else { metadata.len() },
        modified_at,
        is_dir: metadata.is_dir(),
    })
}

fn scan_directory(root: &Path) -> Result<Vec<FileEntry>, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("索引根目录无法访问: {error}"))?;
    let mut entries = Vec::new();
    let mut pending = vec![(root.clone(), 0usize)];
    let mut visited_directories = HashSet::new();
    while let Some((directory, depth)) = pending.pop() {
        if !visited_directories.insert(directory.clone()) {
            continue;
        }
        if depth > MAX_SCAN_DEPTH {
            return Err(format!("索引目录超过最大深度 {MAX_SCAN_DEPTH}，请缩小索引范围"));
        }
        let read_dir = fs::read_dir(&directory)
            .map_err(|error| format!("读取目录失败 {}: {error}", directory.display()))?;
        for item in read_dir {
            let item = item.map_err(|error| format!("读取目录项失败: {error}"))?;
            // Never follow links: a junction/symlink can otherwise escape the
            // configured root after the root itself has been canonicalized.
            if item
                .file_type()
                .map_err(|error| format!("读取文件类型失败: {error}"))?
                .is_symlink()
            {
                continue;
            }
            let path = item.path();
            let canonical = match path.canonicalize() {
                Ok(value) if value.starts_with(&root) => value,
                _ => continue,
            };
            let entry = match entry_from_path(&canonical) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let is_dir = entry.is_dir;
            entries.push(entry);
            if entries.len() > MAX_INDEX_ENTRIES {
                return Err(format!("索引项目超过上限 {MAX_INDEX_ENTRIES}，请缩小索引范围"));
            }
            if is_dir {
                if depth == MAX_SCAN_DEPTH {
                    return Err(format!("索引目录超过最大深度 {MAX_SCAN_DEPTH}，请缩小索引范围"));
                }
                pending.push((canonical, depth + 1));
            }
        }
    }
    Ok(entries)
}

fn bump_generation(value: &mut SearchState) {
    value.generation = value.generation.saturating_add(1);
}

fn validate_root(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("文件夹路径不能为空".to_string());
    }
    let root = PathBuf::from(trimmed)
        .canonicalize()
        .map_err(|error| format!("文件夹不存在或无法访问: {error}"))?;
    if !root.is_dir() {
        return Err("配置路径必须是文件夹".to_string());
    }
    if is_dangerous_root(&root) {
        return Err("不能将磁盘根目录作为索引范围".to_string());
    }
    Ok(root)
}

#[tauri::command]
pub fn file_search_get_status(app: AppHandle) -> Result<FileSearchStatus, String> {
    ensure_initialized(&app)?;
    let value = lock_state()?;
    Ok(status_locked(&value))
}

#[tauri::command]
pub fn file_search_set_root(app: AppHandle, root: Option<String>) -> Result<FileSearchStatus, String> {
    ensure_initialized(&app)?;
    let validated_root = root.as_deref().map(validate_root).transpose()?;
    let mut value = lock_state()?;
    persist_config_values(&app, validated_root.as_deref(), false)?;
    value.root = validated_root;
    value.entries.clear();
    value.paused = false;
    bump_generation(&mut value);
    Ok(status_locked(&value))
}

#[tauri::command]
pub fn file_search_set_paused(app: AppHandle, paused: bool) -> Result<FileSearchStatus, String> {
    ensure_initialized(&app)?;
    let mut value = lock_state()?;
    let root = value.root.clone();
    persist_config_values(&app, root.as_deref(), paused)?;
    value.paused = paused;
    bump_generation(&mut value);
    Ok(status_locked(&value))
}

#[tauri::command]
pub fn file_search_refresh_index(app: AppHandle) -> Result<FileSearchStatus, String> {
    ensure_initialized(&app)?;
    let (root, initial_generation, paused) = {
        let value = lock_state()?;
        (
            value.root.clone().ok_or_else(|| "请先配置受控文件夹".to_string())?,
            value.generation,
            value.paused,
        )
    };
    if paused {
        return Err("文件索引已暂停，请先恢复后再刷新".to_string());
    }
    let entries = scan_directory(&root)?;
    let mut value = lock_state()?;
    // The root can be changed while a synchronous scan is running; do not
    // install results into the wrong generation.
    if value.root.as_deref() != Some(root.as_path())
        || value.generation != initial_generation
        || value.paused
    {
        return Err("索引范围已变更，请重新刷新".to_string());
    }
    value.entries = entries;
    bump_generation(&mut value);
    Ok(status_locked(&value))
}

#[tauri::command]
pub fn file_search_clear_index(app: AppHandle) -> Result<FileSearchStatus, String> {
    ensure_initialized(&app)?;
    let mut value = lock_state()?;
    value.entries.clear();
    bump_generation(&mut value);
    Ok(status_locked(&value))
}

fn fuzzy_score(needle: &str, haystack: &str) -> Option<i64> {
    if needle.is_empty() {
        return Some(0);
    }
    let needle = needle.to_lowercase();
    let haystack = haystack.to_lowercase();
    if haystack.contains(&needle) {
        return Some(10_000 - haystack.len() as i64);
    }
    let mut cursor = 0usize;
    let mut score = 0i64;
    for character in needle.chars() {
        let relative = haystack[cursor..].find(character)?;
        score += 100 - relative as i64;
        cursor += relative + character.len_utf8();
    }
    Some(score)
}

#[tauri::command]
pub fn file_search_search(app: AppHandle, request: SearchFilesRequest) -> Result<SearchFilesResponse, String> {
    ensure_initialized(&app)?;
    let value = lock_state()?;
    let recent_ranks = if request.sort.eq_ignore_ascii_case("recent") {
        let connection = recent_connection(&app)?;
        let mut statement = connection
            .prepare("SELECT path FROM file_search_recent ORDER BY opened_at DESC LIMIT ?1")
            .map_err(|error| format!("读取最近文件失败: {error}"))?;
        let paths = statement
            .query_map([MAX_RECENT_FILES as i64], |row| row.get::<_, String>(0))
            .map_err(|error| format!("读取最近文件失败: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("读取最近文件失败: {error}"))?;
        paths
            .into_iter()
            .enumerate()
            .map(|(rank, path)| (path, rank))
            .collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    let extension_filter = request
        .extension
        .as_deref()
        .unwrap_or_default()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    let mut scored = value
        .entries
        .iter()
        .filter_map(|entry| {
            if !extension_filter.is_empty() && entry.extension != extension_filter {
                return None;
            }
            let haystack = format!("{} {} {}", entry.name, entry.path, entry.extension);
            let score = fuzzy_score(&request.query, &haystack)?;
            Some((score, entry.clone()))
        })
        .collect::<Vec<_>>();
    match request.sort.to_ascii_lowercase().as_str() {
        "modified" => scored.sort_by(|a, b| b.1.modified_at.cmp(&a.1.modified_at)),
        "name" => scored.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())),
        "recent" => scored.sort_by(|a, b| {
            recent_ranks
                .get(&a.1.path)
                .unwrap_or(&usize::MAX)
                .cmp(recent_ranks.get(&b.1.path).unwrap_or(&usize::MAX))
                .then_with(|| b.1.modified_at.cmp(&a.1.modified_at))
        }),
        _ => scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path))),
    }
    let total = scored.len();
    let limit = request.limit.clamp(1, 200);
    let items = scored.into_iter().take(limit).map(|(_, item)| item).collect();
    Ok(SearchFilesResponse {
        items,
        total,
        generation: value.generation,
    })
}

fn refresh_index_after_operation(root: &Path, value: &mut SearchState) -> Result<(), String> {
    if value.paused {
        value.entries.clear();
    } else {
        match scan_directory(root) {
            Ok(entries) => value.entries = entries,
            Err(error) => {
                value.entries.clear();
                bump_generation(value);
                return Err(format!("文件操作已完成，但索引刷新失败: {error}"));
            }
        }
    }
    bump_generation(value);
    Ok(())
}

fn validate_existing_target(
    app: &AppHandle,
    path: &str,
    expected_generation: u64,
) -> Result<(std::sync::MutexGuard<'static, SearchState>, PathBuf, PathBuf), String> {
    ensure_initialized(app)?;
    let value = lock_state()?;
    if value.generation != expected_generation {
        return Err("文件索引已更新，请刷新后重试".to_string());
    }
    let root = value
        .root
        .clone()
        .ok_or_else(|| "请先配置受控文件夹".to_string())?;
    let target = ensure_inside(&root, Path::new(path))?;
    if target == root {
        return Err("不能操作受控文件夹本身".to_string());
    }
    Ok((value, root, target))
}

fn image_mime(path: &Path) -> Option<&'static str> {
    match extension(path).as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn is_text_extension(path: &Path) -> bool {
    matches!(
        extension(path).as_str(),
        "txt" | "md" | "markdown" | "json" | "toml" | "yaml" | "yml" | "csv" | "log"
            | "xml" | "html" | "css" | "js" | "ts" | "tsx" | "jsx" | "rs" | "py"
            | "go" | "java" | "c" | "cpp" | "h" | "hpp" | "ini" | "conf"
    )
}

fn read_bounded(path: &Path, max_bytes: usize) -> Result<(Vec<u8>, bool), String> {
    let file = File::open(path).map_err(|error| format!("读取预览失败: {error}"))?;
    let mut bytes = Vec::with_capacity(max_bytes.saturating_add(1));
    file.take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取预览失败: {error}"))?;
    let truncated = bytes.len() > max_bytes;
    bytes.truncate(bytes.len().min(max_bytes));
    Ok((bytes, truncated))
}

#[tauri::command]
pub fn file_search_preview(
    app: AppHandle,
    path: String,
    expected_generation: u64,
) -> Result<FilePreview, String> {
    let (_state, _root, target) = validate_existing_target(&app, &path, expected_generation)?;
    let metadata = fs::metadata(&target).map_err(|error| format!("读取文件元数据失败: {error}"))?;
    if metadata.is_dir() {
        return Ok(FilePreview {
            kind: "unsupported".to_string(),
            path: target.to_string_lossy().into_owned(),
            content: None,
            mime_type: None,
            image_data_url: None,
            size: 0,
            truncated: false,
        });
    }
    if let Some(mime_type) = image_mime(&target) {
        if metadata.len() > MAX_IMAGE_PREVIEW_BYTES as u64 {
            return Ok(FilePreview {
                kind: "unsupported".to_string(),
                path: target.to_string_lossy().into_owned(),
                content: None,
                mime_type: Some(mime_type.to_string()),
                image_data_url: None,
                size: metadata.len(),
                truncated: true,
            });
        }
        let (bytes, truncated) = read_bounded(&target, MAX_IMAGE_PREVIEW_BYTES)?;
        if truncated {
            return Ok(FilePreview {
                kind: "unsupported".to_string(),
                path: target.to_string_lossy().into_owned(),
                content: None,
                mime_type: Some(mime_type.to_string()),
                image_data_url: None,
                size: metadata.len(),
                truncated: true,
            });
        }
        return Ok(FilePreview {
            kind: "image".to_string(),
            path: target.to_string_lossy().into_owned(),
            content: None,
            mime_type: Some(mime_type.to_string()),
            image_data_url: Some(format!(
                "data:{mime_type};base64,{}",
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
            )),
            size: metadata.len(),
            truncated: false,
        });
    }
    if is_text_extension(&target) {
        let (bytes, mut truncated) = read_bounded(&target, MAX_TEXT_PREVIEW_BYTES)?;
        let valid_bytes = match std::str::from_utf8(&bytes) {
            Ok(_) => bytes.len(),
            Err(error) if error.error_len().is_none() => {
                truncated = true;
                error.valid_up_to()
            }
            Err(_) => return Err("文本不是有效的 UTF-8，无法预览".to_string()),
        };
        let content = String::from_utf8(bytes[..valid_bytes].to_vec())
            .map_err(|_| "文本不是有效的 UTF-8，无法预览".to_string())?;
        return Ok(FilePreview {
            kind: "text".to_string(),
            path: target.to_string_lossy().into_owned(),
            content: Some(content),
            mime_type: Some("text/plain".to_string()),
            image_data_url: None,
            size: metadata.len(),
            truncated,
        });
    }
    Ok(FilePreview {
        kind: "unsupported".to_string(),
        path: target.to_string_lossy().into_owned(),
        content: None,
        mime_type: None,
        image_data_url: None,
        size: metadata.len(),
        truncated: false,
    })
}

fn recent_connection(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let connection = crate::notes::get_connection(app)?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS file_search_recent (path TEXT PRIMARY KEY, opened_at INTEGER NOT NULL);",
        )
        .map_err(|error| format!("初始化最近文件记录失败: {error}"))?;
    Ok(connection)
}

fn record_recent(app: &AppHandle, path: &Path) -> Result<(), String> {
    let connection = recent_connection(app)?;
    connection
        .execute(
            "INSERT INTO file_search_recent(path, opened_at) VALUES (?1, ?2) ON CONFLICT(path) DO UPDATE SET opened_at = excluded.opened_at",
            rusqlite::params![path.to_string_lossy().to_string(), Utc::now().timestamp()],
        )
        .map_err(|error| format!("记录最近文件失败: {error}"))?;
    connection
        .execute(
            "DELETE FROM file_search_recent WHERE path NOT IN (SELECT path FROM file_search_recent ORDER BY opened_at DESC LIMIT ?1)",
            [MAX_RECENT_FILES as i64],
        )
        .map_err(|error| format!("整理最近文件失败: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn file_search_open_path(
    app: AppHandle,
    path: String,
    expected_generation: u64,
) -> Result<FileEntry, String> {
    let (_state, _root, target) = validate_existing_target(&app, &path, expected_generation)?;
    open_and_record_recent(
        &target,
        |path| {
            app.opener()
                .open_path(path.to_string_lossy().to_string(), None::<String>)
                .map_err(|error| format!("打开文件失败: {error}"))
        },
        |path| record_recent(&app, path),
    )?;
    entry_from_path(&target)
}

fn open_and_record_recent(
    path: &Path,
    open: impl FnOnce(&Path) -> Result<(), String>,
    record_recent: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    open(path)?;
    let _ = record_recent(path);
    Ok(())
}

#[tauri::command]
pub fn file_search_reveal_in_explorer(
    app: AppHandle,
    path: String,
    expected_generation: u64,
) -> Result<String, String> {
    let (_state, _root, target) = validate_existing_target(&app, &path, expected_generation)?;
    app.opener()
        .reveal_item_in_dir(&target)
        .map_err(|error| format!("在资源管理器中定位失败: {error}"))?;
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn file_search_copy_path(
    app: AppHandle,
    path: String,
    expected_generation: u64,
) -> Result<String, String> {
    let (_state, _root, target) = validate_existing_target(&app, &path, expected_generation)?;
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn file_search_recent(
    app: AppHandle,
    limit: Option<usize>,
) -> Result<SearchFilesResponse, String> {
    ensure_initialized(&app)?;
    let value = lock_state()?;
    let root = value.root.clone().ok_or_else(|| "请先配置受控文件夹".to_string())?;
    let generation = value.generation;
    drop(value);
    let connection = recent_connection(&app)?;
    let mut statement = connection
        .prepare("SELECT path FROM file_search_recent ORDER BY opened_at DESC LIMIT ?1")
        .map_err(|error| format!("读取最近文件失败: {error}"))?;
    let paths = statement
        .query_map([limit.unwrap_or(20).clamp(1, 100) as i64], |row| row.get::<_, String>(0))
        .map_err(|error| format!("读取最近文件失败: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取最近文件失败: {error}"))?;
    let mut result = Vec::new();
    for path in paths {
        let target = match ensure_inside(&root, Path::new(&path)) {
            Ok(value) if value.exists() => value,
            _ => continue,
        };
        if let Ok(entry) = entry_from_path(&target) {
            result.push(entry);
        }
    }
    let total = result.len();
    let current = lock_state()?;
    if current.generation != generation || current.root.as_deref() != Some(root.as_path()) {
        return Err("索引范围已变更，请重新读取最近文件".to_string());
    }
    Ok(SearchFilesResponse {
        items: result,
        total,
        generation,
    })
}

#[derive(Default)]
struct CopyBudget {
    entries: usize,
    bytes: u64,
}

fn copy_entry_inner(
    source: &Path,
    destination: &Path,
    depth: usize,
    budget: &mut CopyBudget,
) -> Result<(), String> {
    if depth > MAX_COPY_DEPTH {
        return Err(format!("复制目录超过最大深度 {MAX_COPY_DEPTH}"));
    }
    budget.entries += 1;
    if budget.entries > MAX_COPY_ENTRIES {
        return Err(format!("复制项目超过上限 {MAX_COPY_ENTRIES}"));
    }
    let metadata = fs::symlink_metadata(source).map_err(|error| format!("读取复制源失败: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if destination.exists() {
        return Err("目标位置已存在同名文件".to_string());
    }
    if metadata.is_dir() {
        fs::create_dir(destination).map_err(|error| format!("创建目标文件夹失败: {error}"))?;
        let result = (|| {
            for child in fs::read_dir(source).map_err(|error| format!("读取复制源失败: {error}"))? {
                let child = child.map_err(|error| format!("读取复制源失败: {error}"))?;
                copy_entry_inner(
                    &child.path(),
                    &destination.join(child.file_name()),
                    depth + 1,
                    budget,
                )?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(destination);
        }
        result
    } else {
        budget.bytes = budget.bytes.saturating_add(metadata.len());
        if budget.bytes > MAX_COPY_BYTES {
            return Err("单次复制不能超过 2 GiB".to_string());
        }
        let mut source_file = File::open(source).map_err(|error| format!("打开复制源失败: {error}"))?;
        let mut destination_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|error| format!("创建复制目标失败（目标可能已存在）: {error}"))?;
        if let Err(error) = std::io::copy(&mut source_file, &mut destination_file) {
            let _ = fs::remove_file(destination);
            return Err(format!("复制文件失败: {error}"));
        }
        Ok(())
    }
}

fn copy_entry(source: &Path, destination: &Path) -> Result<(), String> {
    copy_entry_inner(source, destination, 0, &mut CopyBudget::default())
}

fn safe_child_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed != "."
        && trimmed != ".."
        && !Path::new(trimmed).components().any(|component| {
            matches!(component, Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_))
        })
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
}

#[tauri::command]
pub fn file_search_rename_path(
    app: AppHandle,
    path: String,
    new_name: String,
    expected_generation: u64,
) -> Result<FileOperationResponse, String> {
    let (mut value, root, target) = validate_existing_target(&app, &path, expected_generation)?;
    if !safe_child_name(&new_name) {
        return Err("新文件名不安全".to_string());
    }
    let parent = target
        .parent()
        .ok_or_else(|| "无法确定目标文件夹".to_string())?;
    let parent = ensure_inside(&root, parent)?;
    let destination = parent.join(new_name.trim());
    if destination.exists() {
        return Err("目标名称已存在".to_string());
    }
    fs::rename(&target, &destination).map_err(|error| format!("重命名失败: {error}"))?;
    refresh_index_after_operation(&root, &mut value)?;
    let entry = entry_from_path(&destination)?;
    let generation = value.generation;
    Ok(FileOperationResponse { entry, generation })
}

#[tauri::command]
pub fn file_search_move_path(
    app: AppHandle,
    path: String,
    destination_dir: String,
    expected_generation: u64,
) -> Result<FileOperationResponse, String> {
    let (mut value, root, target) = validate_existing_target(&app, &path, expected_generation)?;
    let destination_dir = ensure_inside(&root, Path::new(&destination_dir))?;
    if !destination_dir.is_dir() {
        return Err("目标位置必须是已存在的文件夹".to_string());
    }
    if destination_dir == target || destination_dir.starts_with(&target) {
        return Err("不能将文件夹移动到自身或其子目录".to_string());
    }
    let name = target
        .file_name()
        .ok_or_else(|| "无法确定文件名".to_string())?;
    let destination = destination_dir.join(name);
    if destination.exists() {
        return Err("目标位置已存在同名文件".to_string());
    }
    fs::rename(&target, &destination).map_err(|error| format!("移动失败: {error}"))?;
    refresh_index_after_operation(&root, &mut value)?;
    let entry = entry_from_path(&destination)?;
    let generation = value.generation;
    Ok(FileOperationResponse { entry, generation })
}

#[tauri::command]
pub fn file_search_copy_file(
    app: AppHandle,
    path: String,
    destination_dir: String,
    expected_generation: u64,
) -> Result<FileOperationResponse, String> {
    let (mut value, root, source) = validate_existing_target(&app, &path, expected_generation)?;
    let destination_dir = ensure_inside(&root, Path::new(&destination_dir))?;
    if !destination_dir.is_dir() {
        return Err("目标位置必须是已存在的文件夹".to_string());
    }
    if destination_dir == source || destination_dir.starts_with(&source) {
        return Err("不能将文件夹复制到自身或其子目录".to_string());
    }
    let name = source
        .file_name()
        .ok_or_else(|| "无法确定文件名".to_string())?;
    let destination = destination_dir.join(name);
    copy_entry(&source, &destination)?;
    refresh_index_after_operation(&root, &mut value)?;
    let entry = entry_from_path(&destination)?;
    let generation = value.generation;
    Ok(FileOperationResponse { entry, generation })
}

#[cfg(target_os = "windows")]
fn recycle_path(path: &Path) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{
        SHFileOperationW, SHFILEOPSTRUCTW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI,
        FO_DELETE,
    };
    let mut source = path.as_os_str().encode_wide().collect::<Vec<_>>();
    source.push(0);
    source.push(0);
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(source.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI).0 as u16,
        ..Default::default()
    };
    let result = unsafe { SHFileOperationW(&mut operation) };
    if result != 0 || operation.fAnyOperationsAborted.as_bool() {
        return Err(format!("移动到回收站失败，Windows 错误码 {result}"));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn recycle_path(_path: &Path) -> Result<(), String> {
    Err("当前平台不支持回收站删除".to_string())
}

#[tauri::command]
pub fn file_search_delete_to_recycle_bin(
    app: AppHandle,
    path: String,
    expected_generation: u64,
) -> Result<FileMutationResponse, String> {
    let (mut value, root, target) = validate_existing_target(&app, &path, expected_generation)?;
    recycle_path(&target)?;
    let target_string = target.to_string_lossy().to_string();
    value.entries.retain(|entry| {
        entry.path != target_string && !Path::new(&entry.path).starts_with(&target)
    });
    refresh_index_after_operation(&root, &mut value)?;
    let generation = value.generation;
    Ok(FileMutationResponse {
        ok: true,
        path: target_string,
        generation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fuzzy_matching_supports_subsequence_and_exact_precedence() {
        assert!(fuzzy_score("rpt", "report.txt").is_some());
        assert!(fuzzy_score("report", "report.txt") > fuzzy_score("rpt", "report.txt"));
        assert!(fuzzy_score("xyz", "report.txt").is_none());
    }

    #[test]
    fn safe_child_name_rejects_path_traversal() {
        assert!(safe_child_name("renamed.txt"));
        assert!(!safe_child_name("../outside.txt"));
        assert!(!safe_child_name("folder\\file.txt"));
        assert!(!safe_child_name(""));
        assert!(!safe_child_name("."));
    }

    #[test]
    fn scan_skips_symlinks_and_obeys_limit_shape() {
        let root = std::env::temp_dir().join(format!("easyflowhub-file-search-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("nested")).expect("create test root");
        fs::write(root.join("readme.txt"), b"hello").expect("write test file");
        fs::write(root.join("nested").join("data.json"), b"{}").expect("write nested file");
        let entries = scan_directory(&root).expect("scan test root");
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|entry| entry.name == "readme.txt"));
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn preview_text_has_a_bounded_size() {
        assert_eq!(MAX_TEXT_PREVIEW_BYTES, 512 * 1024);
        assert!(Duration::from_secs(1).as_secs() > 0);
    }

    #[test]
    fn failed_post_operation_scan_clears_stale_entries_and_bumps_generation() {
        let root = std::env::temp_dir().join(format!("easyflowhub-refresh-failure-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create test root");
        let canonical_root = root.canonicalize().expect("canonicalize test root");
        let mut value = SearchState {
            root: Some(canonical_root.clone()),
            generation: 12,
            entries: vec![FileEntry {
                id: canonical_root.join("stale.txt").to_string_lossy().into_owned(),
                path: canonical_root.join("stale.txt").to_string_lossy().into_owned(),
                name: "stale.txt".to_string(),
                extension: "txt".to_string(),
                size: 4,
                modified_at: None,
                is_dir: false,
            }],
            ..SearchState::default()
        };
        fs::remove_dir_all(&root).expect("remove test root before failed scan");

        let error = refresh_index_after_operation(&canonical_root, &mut value)
            .expect_err("refresh should fail for a removed root");

        assert!(error.contains("文件操作已完成，但索引刷新失败"));
        assert!(value.entries.is_empty());
        assert_eq!(value.generation, 13);
        assert_eq!(value.root.as_deref(), Some(canonical_root.as_path()));
    }

    #[test]
    fn recent_history_failure_does_not_turn_a_successful_open_into_an_error() {
        let path = Path::new("C:/Files/example.txt");
        let mut opened = false;

        let result = open_and_record_recent(
            path,
            |_| {
                opened = true;
                Ok(())
            },
            |_| Err("database unavailable".to_string()),
        );

        assert!(opened);
        assert!(result.is_ok());
    }

    #[test]
    fn image_mime_is_extension_based_without_reading_payload() {
        assert_eq!(image_mime(Path::new("photo.PNG")), Some("image/png"));
        assert_eq!(image_mime(Path::new("archive.zip")), None);
    }

    #[test]
    fn copy_rejects_existing_destination_without_overwriting() {
        let root = std::env::temp_dir().join(format!("easyflowhub-copy-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create test root");
        let source = root.join("source.txt");
        let destination = root.join("destination.txt");
        fs::write(&source, b"source").expect("write source");
        fs::write(&destination, b"original").expect("write destination");
        let error = copy_entry(&source, &destination).expect_err("existing target must fail");
        assert!(error.contains("已存在"));
        assert_eq!(fs::read(&destination).expect("read destination"), b"original");
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn copy_directory_updates_nested_entries_and_cleans_partial_output() {
        let root = std::env::temp_dir().join(format!("easyflowhub-copy-dir-{}", uuid::Uuid::new_v4()));
        let source = root.join("source");
        let destination = root.join("copy");
        fs::create_dir_all(source.join("nested")).expect("create source");
        fs::write(source.join("nested").join("one.txt"), b"1").expect("write nested file");
        copy_entry(&source, &destination).expect("copy directory");
        assert_eq!(fs::read(destination.join("nested").join("one.txt")).expect("read copy"), b"1");
        fs::remove_dir_all(root).expect("remove test root");
    }
}
