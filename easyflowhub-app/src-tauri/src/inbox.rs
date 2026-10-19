use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;

const MAX_TEXT_CHARS: usize = 1_000_000;
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxAttachment {
    pub id: String,
    pub item_id: String,
    pub kind: String,
    pub name: String,
    pub mime_type: String,
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub source: String,
    pub context: String,
    pub captured_at: String,
    pub archived_at: Option<String>,
    pub converted_note_id: Option<String>,
    pub attachments: Vec<InboxAttachment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureInboxItemInput {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub image_data_url: Option<String>,
    #[serde(default)]
    pub file_paths: Vec<String>,
    #[serde(default)]
    pub audio_file_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemsResponse {
    pub ok: bool,
    pub count: usize,
    pub items: Vec<InboxItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemResponse {
    pub ok: bool,
    pub item: InboxItem,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxStateResponse {
    pub ok: bool,
    pub item_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertInboxItemResponse {
    pub ok: bool,
    pub item_id: String,
    pub note_id: String,
}

#[derive(Debug)]
struct PendingAttachment {
    attachment: InboxAttachment,
    temporary_path: PathBuf,
    final_path: PathBuf,
}

pub(crate) fn init_db(conn: &Connection) -> SqliteResult<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS inbox_items (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file', 'audio')),
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT 'manager',
            context TEXT NOT NULL DEFAULT '',
            captured_at TEXT NOT NULL,
            archived_at TEXT DEFAULT NULL,
            converted_note_id TEXT DEFAULT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_inbox_items_captured_at
            ON inbox_items(captured_at DESC);
        CREATE INDEX IF NOT EXISTS idx_inbox_items_archived_at
            ON inbox_items(archived_at, captured_at DESC);

        CREATE TABLE IF NOT EXISTS inbox_attachments (
            id TEXT PRIMARY KEY,
            item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK (kind IN ('image', 'file', 'audio', 'transcript')),
            name TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            filename TEXT NOT NULL UNIQUE,
            size_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_inbox_attachments_item_id
            ON inbox_attachments(item_id, created_at);
        "#,
    )?;
    migrate_capture_kinds_schema(conn)
}

/// Rebuild the additive Inbox tables once for databases created before the
/// audio or transcript kinds existed. SQLite cannot alter a CHECK constraint
/// in place.
fn migrate_capture_kinds_schema(conn: &Connection) -> SqliteResult<()> {
    let item_sql: Option<String> = conn
        .query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'inbox_items'", [], |row| row.get(0))
        .optional()?;
    let attachment_sql: Option<String> = conn
        .query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'inbox_attachments'", [], |row| row.get(0))
        .optional()?;
    let needs_items = item_sql.as_deref().is_some_and(|sql| !sql.contains("'audio'"));
    let needs_attachments = attachment_sql
        .as_deref()
        .is_some_and(|sql| !sql.contains("'audio'") || !sql.contains("'transcript'"));
    if !needs_items && !needs_attachments { return Ok(()); }
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = OFF;
        ALTER TABLE inbox_attachments RENAME TO inbox_attachments_legacy;
        ALTER TABLE inbox_items RENAME TO inbox_items_legacy;
        CREATE TABLE inbox_items (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file', 'audio')),
            title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT 'manager', context TEXT NOT NULL DEFAULT '',
            captured_at TEXT NOT NULL, archived_at TEXT DEFAULT NULL, converted_note_id TEXT DEFAULT NULL
        );
        INSERT INTO inbox_items SELECT id, kind, title, content, source, context, captured_at, archived_at, converted_note_id FROM inbox_items_legacy;
        CREATE TABLE inbox_attachments (
            id TEXT PRIMARY KEY, item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK (kind IN ('image', 'file', 'audio', 'transcript')),
            name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
            size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
        );
        INSERT INTO inbox_attachments SELECT id, item_id, kind, name, mime_type, filename, size_bytes, created_at FROM inbox_attachments_legacy;
        DROP TABLE inbox_attachments_legacy;
        DROP TABLE inbox_items_legacy;
        CREATE INDEX IF NOT EXISTS idx_inbox_items_captured_at ON inbox_items(captured_at DESC);
        CREATE INDEX IF NOT EXISTS idx_inbox_items_archived_at ON inbox_items(archived_at, captured_at DESC);
        CREATE INDEX IF NOT EXISTS idx_inbox_attachments_item_id ON inbox_attachments(item_id, created_at);
        PRAGMA foreign_keys = ON;
        "#,
    )
}

fn get_connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    let conn = crate::notes::get_connection(app)?;
    init_db(&conn).map_err(|error| format!("Failed to initialize inbox storage: {error}"))?;
    Ok(conn)
}

/// Internal accessor for sibling native owners (transcription) that read or
/// extend Inbox data through this module's controlled helpers.
pub(crate) fn inbox_connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    get_connection(app)
}

/// A stored speech-to-text transcript belonging to an Inbox audio item.
#[derive(Debug, Clone)]
pub(crate) struct InboxTranscript {
    pub attachment_id: String,
    pub text: String,
    pub model: Option<String>,
    pub created_at: String,
}

const TRANSCRIPT_PREFIX: &str = "转写文稿";
const TRANSCRIPT_SUFFIX: &str = "）.txt";
const MAX_TRANSCRIPT_BYTES: u64 = 1024 * 1024;

/// Transcript attachment display name. The provider model is kept in the
/// display name so a transcript can be traced back to its provider without a
/// second storage table.
pub(crate) fn transcript_attachment_name(model: &str) -> String {
    if model.trim().is_empty() {
        format!("{TRANSCRIPT_PREFIX}.txt")
    } else {
        format!("{TRANSCRIPT_PREFIX}（{}）.txt", clean_display_name(model))
    }
}

fn transcript_model_from_name(name: &str) -> Option<String> {
    let rest = name.strip_prefix(&format!("{TRANSCRIPT_PREFIX}（"))?;
    let model = rest.strip_suffix(TRANSCRIPT_SUFFIX)?;
    let model = model.trim();
    if model.is_empty() { None } else { Some(model.to_string()) }
}

/// Find the first audio attachment of an item for transcription. Returns the
/// storage filename and declared size.
pub(crate) fn find_audio_attachment_at(
    conn: &Connection,
    item_id: &str,
) -> Result<Option<(String, u64)>, String> {
    let row = conn
        .query_row(
            r#"
            SELECT filename, size_bytes FROM inbox_attachments
            WHERE item_id = ? AND kind = 'audio'
            ORDER BY created_at, id LIMIT 1
            "#,
            [item_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?.max(0) as u64)),
        )
        .optional()
        .map_err(|error| format!("Failed to query inbox audio attachment: {error}"))?;
    Ok(row)
}

/// Replace any existing transcript of an item with a new one inside a single
/// transaction. The audio attachment and the item itself are never touched;
/// a failed transcript write leaves the previous state intact.
pub(crate) fn store_transcript_at(
    conn: &mut Connection,
    assets_dir: &Path,
    item_id: &str,
    text: &str,
    model: &str,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("转写结果为空。".to_string());
    }
    if text.len() as u64 > MAX_TRANSCRIPT_BYTES {
        return Err("转写文本超过 1 MB 限制。".to_string());
    }
    let existing: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, filename FROM inbox_attachments WHERE item_id = ? AND kind = 'transcript'")
            .map_err(|error| format!("Failed to prepare transcript query: {error}"))?;
        let rows = stmt
            .query_map([item_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|error| format!("Failed to query inbox transcript: {error}"))?
            .collect::<SqliteResult<Vec<_>>>()
            .map_err(|error| format!("Failed to collect inbox transcript: {error}"))?;
        rows
    };

    let attachment_id = Uuid::new_v4().to_string();
    let storage_key = format!("{}.txt", Uuid::new_v4());
    let temporary_path = assets_dir.join(format!(".{storage_key}.pending"));
    let final_path = assets_dir.join(&storage_key);
    if let Err(error) = fs::write(&temporary_path, text.as_bytes()) {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!("保存转写文本失败: {error}"));
    }

    let created_at = Utc::now().to_rfc3339();
    let display_name = transcript_attachment_name(model);
    let write_result = (|| -> Result<(), String> {
        let transaction = conn
            .transaction()
            .map_err(|error| format!("Failed to begin transcript write: {error}"))?;
        for (existing_id, existing_filename) in &existing {
            transaction
                .execute("DELETE FROM inbox_attachments WHERE id = ?", [existing_id])
                .map_err(|error| format!("Failed to replace inbox transcript: {error}"))?;
            let _ = fs::remove_file(assets_dir.join(existing_filename));
        }
        transaction
            .execute(
                r#"
                INSERT INTO inbox_attachments
                    (id, item_id, kind, name, mime_type, filename, size_bytes, created_at)
                VALUES (?, ?, 'transcript', ?, 'text/plain', ?, ?, ?)
                "#,
                params![
                    attachment_id,
                    item_id,
                    display_name,
                    storage_key,
                    text.len() as i64,
                    created_at
                ],
            )
            .map_err(|error| format!("Failed to save inbox transcript: {error}"))?;
        fs::rename(&temporary_path, &final_path)
            .map_err(|error| format!("无法完成转写附件保存: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Failed to commit inbox transcript: {error}"))
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        let _ = fs::remove_file(&final_path);
        return Err(error);
    }

    Ok(attachment_id)
}

/// Read a transcript attachment file as bounded UTF-8 text, for embedding
/// into a converted note.
fn read_transcript_attachment_text(assets_dir: &Path, filename: &str) -> Result<String, String> {
    let safe_name = Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == filename)
        .ok_or_else(|| "附件存储路径无效。".to_string())?;
    let bytes = fs::read(assets_dir.join(safe_name)).map_err(|error| format!("读取转写文本失败: {error}"))?;
    if bytes.len() as u64 > MAX_TRANSCRIPT_BYTES {
        return Err("转写文本超过 1 MB 限制。".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "转写文本不是有效的 UTF-8 内容。".to_string())
}

/// Read the current transcript of an item, if one exists.
pub(crate) fn read_transcript_at(
    conn: &Connection,
    assets_dir: &Path,
    item_id: &str,
) -> Result<Option<InboxTranscript>, String> {
    let row = conn
        .query_row(
            r#"
            SELECT id, name, filename, created_at FROM inbox_attachments
            WHERE item_id = ? AND kind = 'transcript'
            ORDER BY created_at, id LIMIT 1
            "#,
            [item_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("Failed to query inbox transcript: {error}"))?;
    let Some((attachment_id, name, filename, created_at)) = row else {
        return Ok(None);
    };
    let text = match read_transcript_attachment_text(assets_dir, &filename) {
        Ok(text) => text,
        // A missing or unreadable file must not break listing or the panel;
        // treat it as "no transcript" instead of failing the whole command.
        Err(_) => return Ok(None),
    };
    Ok(Some(InboxTranscript {
        attachment_id,
        model: transcript_model_from_name(&name),
        text,
        created_at,
    }))
}

fn map_attachment(row: &rusqlite::Row<'_>) -> SqliteResult<InboxAttachment> {
    Ok(InboxAttachment {
        id: row.get(0)?,
        item_id: row.get(1)?,
        kind: row.get(2)?,
        name: row.get(3)?,
        mime_type: row.get(4)?,
        filename: row.get(5)?,
        size_bytes: row.get::<_, i64>(6)?.max(0) as u64,
        created_at: row.get(7)?,
    })
}

fn list_attachments(conn: &Connection, item_id: &str) -> Result<Vec<InboxAttachment>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, item_id, kind, name, mime_type, filename, size_bytes, created_at
            FROM inbox_attachments
            WHERE item_id = ?
            ORDER BY created_at, id
            "#,
        )
        .map_err(|error| format!("Failed to prepare inbox attachments: {error}"))?;
    let rows = stmt
        .query_map([item_id], map_attachment)
        .map_err(|error| format!("Failed to query inbox attachments: {error}"))?
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|error| format!("Failed to collect inbox attachments: {error}"))?;
    Ok(rows)
}

fn get_item(conn: &Connection, id: &str) -> Result<Option<InboxItem>, String> {
    let mut item = conn
        .query_row(
            r#"
            SELECT id, kind, title, content, source, context, captured_at, archived_at, converted_note_id
            FROM inbox_items WHERE id = ?
            "#,
            [id],
            |row| {
                Ok(InboxItem {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    source: row.get(4)?,
                    context: row.get(5)?,
                    captured_at: row.get(6)?,
                    archived_at: row.get(7)?,
                    converted_note_id: row.get(8)?,
                    attachments: Vec::new(),
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to get inbox item: {error}"))?;

    if let Some(item) = &mut item {
        item.attachments = list_attachments(conn, &item.id)?;
    }
    Ok(item)
}

fn clean_display_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .filter(|character| !character.is_control() && *character != '/' && *character != '\\')
        .take(180)
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        "附件".to_string()
    } else {
        clean.to_string()
    }
}

fn extension_for_file(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    if !extension.is_empty() && extension.len() <= 12 && extension.chars().all(|c| c.is_ascii_alphanumeric()) {
        extension
    } else {
        "bin".to_string()
    }
}

fn image_extension(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/bmp" => Some("bmp"),
        _ => None,
    }
}

fn validate_image_signature(bytes: &[u8], mime_type: &str) -> bool {
    match mime_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        "image/bmp" => bytes.starts_with(b"BM"),
        _ => false,
    }
}

fn encoded_image_length_allowed(encoded_len: usize, max_bytes: usize) -> bool {
    let max_encoded_len = max_bytes.saturating_add(2) / 3 * 4;
    encoded_len <= max_encoded_len
}

fn decode_image_data_url(data_url: &str) -> Result<(Vec<u8>, &'static str), String> {
    let (header, encoded) = data_url
        .split_once(',')
        .ok_or_else(|| "剪贴板图片格式无效。".to_string())?;
    let mime_type = header
        .strip_prefix("data:")
        .and_then(|value| value.strip_suffix(";base64"))
        .ok_or_else(|| "只支持 PNG、JPEG、GIF、WebP 或 BMP 图片。".to_string())?;
    let extension = image_extension(mime_type)
        .ok_or_else(|| "只支持 PNG、JPEG、GIF、WebP 或 BMP 图片。".to_string())?;
    if !encoded_image_length_allowed(encoded.len(), MAX_IMAGE_BYTES) {
        return Err("图片超过 32 MB 限制。".to_string());
    }
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
        .map_err(|_| "剪贴板图片数据无法解码。".to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err("图片为空或超过 32 MB 限制。".to_string());
    }
    if !validate_image_signature(&bytes, mime_type) {
        return Err("图片内容与声明的格式不匹配。".to_string());
    }
    Ok((bytes, extension))
}

fn is_http_url(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((scheme, remainder)) = trimmed.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    let authority = remainder
        .split(|character| matches!(character, '/' | '?' | '#'))
        .next()
        .unwrap_or_default();
    if authority.is_empty() {
        return false;
    }
    Url::parse(trimmed).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
    })
}

pub(crate) fn assets_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    crate::notes::assets_path(app)
}

fn prepare_attachment(
    assets_dir: &Path,
    item_id: &str,
    kind: &str,
    name: String,
    mime_type: String,
    extension: &str,
    write: impl FnOnce(&Path) -> Result<u64, String>,
    created_at: &str,
) -> Result<PendingAttachment, String> {
    let attachment_id = Uuid::new_v4().to_string();
    let storage_key = format!("{}.{}", Uuid::new_v4(), extension);
    let temporary_path = assets_dir.join(format!(".{storage_key}.pending"));
    let final_path = assets_dir.join(&storage_key);
    match write(&temporary_path) {
        Ok(size_bytes) => Ok(PendingAttachment {
            attachment: InboxAttachment {
                id: attachment_id,
                item_id: item_id.to_string(),
                kind: kind.to_string(),
                name: clean_display_name(&name),
                mime_type,
                filename: storage_key,
                size_bytes,
                created_at: created_at.to_string(),
            },
            temporary_path,
            final_path,
        }),
        Err(error) => {
            let _ = fs::remove_file(&temporary_path);
            Err(error)
        }
    }
}

fn cleanup_pending(attachments: &[PendingAttachment]) {
    for attachment in attachments {
        let _ = fs::remove_file(&attachment.temporary_path);
        let _ = fs::remove_file(&attachment.final_path);
    }
}

fn insert_item_row(
    conn: &Connection,
    id: &str,
    kind: &str,
    title: &str,
    content: &str,
    source: &str,
    context: &str,
    captured_at: &str,
) -> SqliteResult<()> {
    conn.execute(
        r#"
        INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
        params![id, kind, title, content, source, context, captured_at],
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_inbox_items(
    app: tauri::AppHandle,
    include_archived: Option<bool>,
) -> Result<InboxItemsResponse, String> {
    let conn = get_connection(&app)?;
    let archived = include_archived.unwrap_or(false);
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, kind, title, content, source, context, captured_at, archived_at, converted_note_id
            FROM inbox_items
            WHERE (? = 1 OR archived_at IS NULL)
            ORDER BY captured_at DESC, id DESC
            "#,
        )
        .map_err(|error| format!("Failed to prepare inbox list: {error}"))?;
    let rows = stmt
        .query_map([archived as i32], |row| {
            Ok(InboxItem {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                source: row.get(4)?,
                context: row.get(5)?,
                captured_at: row.get(6)?,
                archived_at: row.get(7)?,
                converted_note_id: row.get(8)?,
                attachments: Vec::new(),
            })
        })
        .map_err(|error| format!("Failed to query inbox list: {error}"))?;
    let mut items = rows
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|error| format!("Failed to collect inbox list: {error}"))?;
    drop(stmt);
    for item in &mut items {
        item.attachments = list_attachments(&conn, &item.id)?;
    }

    Ok(InboxItemsResponse {
        ok: true,
        count: items.len(),
        items,
    })
}

#[tauri::command]
pub fn capture_inbox_item(
    app: tauri::AppHandle,
    input: CaptureInboxItemInput,
) -> Result<InboxItemResponse, String> {
    let content = input.content.trim().to_string();
    if content.chars().count() > MAX_TEXT_CHARS || input.title.chars().count() > 500 {
        return Err("标题或内容超过允许长度。".to_string());
    }
    let image = input
        .image_data_url
        .as_deref()
        .map(decode_image_data_url)
        .transpose()?;
    if content.is_empty() && image.is_none() && input.file_paths.is_empty() && input.audio_file_path.is_none() {
        return Err("请输入文字、链接或添加附件后再保存。".to_string());
    }
    let lower_content = content.to_ascii_lowercase();
    if lower_content.starts_with("http://") || lower_content.starts_with("https://") {
        if !is_http_url(&content) {
            return Err("链接格式无效，只支持 http 或 https 地址。".to_string());
        }
    }

    let canonical_files = input
        .file_paths
        .iter()
        .map(|path| {
            let canonical = fs::canonicalize(path)
                .map_err(|error| format!("无法读取附件 {path}: {error}"))?;
            let metadata = fs::metadata(&canonical)
                .map_err(|error| format!("无法读取附件信息: {error}"))?;
            if !metadata.is_file() {
                return Err(format!("附件不是普通文件: {path}"));
            }
            if metadata.len() > MAX_FILE_BYTES {
                return Err(format!("附件超过 1 GB 限制: {path}"));
            }
            Ok((canonical, metadata.len()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let canonical_audio = input.audio_file_path.as_deref().map(|path| {
        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("无法读取录音文件 {path}: {error}"))?;
        let metadata = fs::metadata(&canonical)
            .map_err(|error| format!("无法读取录音文件信息: {error}"))?;
        if !metadata.is_file() {
            return Err(format!("录音文件不是普通文件: {path}"));
        }
        if metadata.len() > MAX_FILE_BYTES {
            return Err("录音文件超过 1 GB 限制。".to_string());
        }
        Ok((canonical, metadata.len()))
    }).transpose()?;

    let id = Uuid::new_v4().to_string();
    let captured_at = Utc::now().to_rfc3339();
    let source = input
        .source
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "manager".to_string());
    if source.chars().count() > 120 {
        return Err("来源字段过长。".to_string());
    }
    let context = input.context.unwrap_or_default();
    if context.chars().count() > 1000 {
        return Err("上下文字段过长。".to_string());
    }
    let title = if !input.title.trim().is_empty() {
        input.title.trim().to_string()
    } else if !content.is_empty() {
        content.lines().next().unwrap_or_default().chars().take(80).collect()
    } else if let Some((_, extension)) = &image {
        format!("图片捕获.{extension}")
    } else if let Some((path, _)) = &canonical_audio {
        path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| "录音".to_string())
    } else {
        canonical_files
            .first()
            .and_then(|(path, _)| path.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "文件捕获".to_string())
    };
    let kind = if !content.is_empty() && is_http_url(&content) {
        "link"
    } else if !content.is_empty() {
        "text"
    } else if image.is_some() && canonical_files.is_empty() {
        "image"
    } else if canonical_audio.is_some() && image.is_none() && canonical_files.is_empty() {
        "audio"
    } else {
        "file"
    };

    let assets_dir = assets_dir(&app)?;
    let mut pending = Vec::new();
    if let Some((bytes, extension)) = image {
        let mime_type = match extension {
            "jpg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            _ => "image/png",
        };
        let name = format!("剪贴板图片.{extension}");
        match prepare_attachment(
            &assets_dir,
            &id,
            "image",
            name,
            mime_type.to_string(),
            extension,
            |path| {
                fs::write(path, &bytes).map_err(|error| format!("保存剪贴板图片失败: {error}"))?;
                Ok(bytes.len() as u64)
            },
            &captured_at,
        ) {
            Ok(attachment) => pending.push(attachment),
            Err(error) => {
                cleanup_pending(&pending);
                return Err(error);
            }
        }
    }

    if let Some((path, size)) = canonical_audio {
        match prepare_attachment(
            &assets_dir,
            &id,
            "audio",
            path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| "录音.wav".to_string()),
            "audio/wav".to_string(),
            "wav",
            |destination| {
                fs::copy(&path, destination).map_err(|error| format!("复制录音失败: {error}"))?;
                Ok(size)
            },
            &captured_at,
        ) {
            Ok(attachment) => pending.push(attachment),
            Err(error) => {
                cleanup_pending(&pending);
                return Err(error);
            }
        }
    }

    let mut unique_paths = HashSet::new();
    for (path, size) in canonical_files {
        if !unique_paths.insert(path.clone()) {
            continue;
        }
        let display_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "附件".to_string());
        let extension = extension_for_file(&path);
        let mime_type = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            "pdf" => "application/pdf",
            "txt" | "md" | "csv" => "text/plain",
            _ => "application/octet-stream",
        };
        match prepare_attachment(
            &assets_dir,
            &id,
            if mime_type.starts_with("image/") { "image" } else { "file" },
            display_name,
            mime_type.to_string(),
            &extension,
            |destination| {
                fs::copy(&path, destination).map_err(|error| format!("复制附件失败: {error}"))?;
                Ok(size)
            },
            &captured_at,
        ) {
            Ok(attachment) => pending.push(attachment),
            Err(error) => {
                cleanup_pending(&pending);
                return Err(error);
            }
        }
    }

    let mut conn = match get_connection(&app) {
        Ok(conn) => conn,
        Err(error) => {
            cleanup_pending(&pending);
            return Err(error);
        }
    };
    let write_result = (|| -> Result<(), String> {
        let transaction = conn
            .transaction()
            .map_err(|error| format!("Failed to begin inbox capture: {error}"))?;
        insert_item_row(
            &transaction,
            &id,
            kind,
            &title,
            &content,
            &source,
            &context,
            &captured_at,
        )
        .map_err(|error| format!("Failed to save inbox item: {error}"))?;
        for pending_attachment in &pending {
            let attachment = &pending_attachment.attachment;
            transaction
                .execute(
                    r#"
                    INSERT INTO inbox_attachments
                        (id, item_id, kind, name, mime_type, filename, size_bytes, created_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                    params![
                        attachment.id,
                        attachment.item_id,
                        attachment.kind,
                        attachment.name,
                        attachment.mime_type,
                        attachment.filename,
                        attachment.size_bytes as i64,
                        attachment.created_at
                    ],
                )
                .map_err(|error| format!("Failed to save inbox attachment: {error}"))?;
        }
        for pending_attachment in &pending {
            fs::rename(&pending_attachment.temporary_path, &pending_attachment.final_path)
                .map_err(|error| format!("无法完成附件保存: {error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("Failed to commit inbox capture: {error}"))
    })();

    if let Err(error) = write_result {
        cleanup_pending(&pending);
        return Err(error);
    }

    let item = get_item(&conn, &id)?
        .ok_or_else(|| "Inbox 项目保存后无法重新读取。".to_string())?;
    Ok(InboxItemResponse { ok: true, item })
}

fn set_archived(app: &tauri::AppHandle, id: &str, archived: bool) -> Result<InboxStateResponse, String> {
    let conn = get_connection(app)?;
    let archived_at = archived.then(|| Utc::now().to_rfc3339());
    conn.execute(
        "UPDATE inbox_items SET archived_at = ? WHERE id = ?",
        params![archived_at, id],
    )
    .map_err(|error| format!("Failed to update inbox item: {error}"))?;
    Ok(InboxStateResponse {
        ok: true,
        item_id: id.to_string(),
    })
}

#[tauri::command]
pub fn archive_inbox_item(app: tauri::AppHandle, id: String) -> Result<InboxStateResponse, String> {
    set_archived(&app, &id, true)
}

#[tauri::command]
pub fn restore_inbox_item(app: tauri::AppHandle, id: String) -> Result<InboxStateResponse, String> {
    set_archived(&app, &id, false)
}

fn convert_item_to_note(conn: &mut Connection, id: &str, assets_dir: &Path) -> Result<String, String> {
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to begin Inbox conversion: {error}"))?;
    let item: (String, String, String, Option<String>) = transaction
        .query_row(
            "SELECT title, content, captured_at, converted_note_id FROM inbox_items WHERE id = ?",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|error| format!("无法找到收集项: {error}"))?;
    if let Some(note_id) = item.3 {
        transaction
            .commit()
            .map_err(|error| format!("Failed to finish Inbox conversion: {error}"))?;
        return Ok(note_id);
    }

    let mut attachment_stmt = transaction
        .prepare("SELECT kind, name, filename FROM inbox_attachments WHERE item_id = ? ORDER BY created_at, id")
        .map_err(|error| format!("Failed to prepare Inbox attachment query: {error}"))?;
    let attachment_rows = attachment_stmt
        .query_map([&id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
        .map_err(|error| format!("Failed to query Inbox attachments: {error}"))?;
    let attachment_refs = attachment_rows
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|error| format!("Failed to collect Inbox attachments: {error}"))?;
    drop(attachment_stmt);

    let mut content_parts = Vec::new();
    if !item.1.trim().is_empty() {
        content_parts.push(item.1.clone());
    }
    for (kind, name, filename) in attachment_refs {
        if kind == "image" {
            content_parts.push(format!("![{name}](asset:{filename})"));
        } else if kind == "transcript" {
            // Embed the transcript text so the note is self-contained; a
            // failed or missing file falls back to the attachment reference.
            match read_transcript_attachment_text(assets_dir, &filename) {
                Ok(text) if !text.trim().is_empty() => content_parts.push(text),
                _ => content_parts.push(format!("附件：{name}")),
            }
        } else {
            content_parts.push(format!("附件：{name}"));
        }
    }
    let note_content = content_parts.join("\n\n");
    let note_title = if item.0.trim().is_empty() {
        item.1.lines().next().unwrap_or("收集内容").chars().take(80).collect::<String>()
    } else {
        item.0
    };
    let note_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    transaction
        .execute(
            r#"
            INSERT INTO notes (id, title, content, tags, created_at, updated_at, is_pinned)
            VALUES (?, ?, ?, 'inbox', ?, ?, 0)
            "#,
            params![note_id, note_title, note_content, item.2, now],
        )
        .map_err(|error| format!("Failed to create note from Inbox item: {error}"))?;
    transaction
        .execute(
            "UPDATE inbox_items SET converted_note_id = ?, archived_at = COALESCE(archived_at, ?) WHERE id = ?",
            params![note_id, now, id],
        )
        .map_err(|error| format!("Failed to mark Inbox item as converted: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit Inbox conversion: {error}"))?;

    Ok(note_id)
}

#[tauri::command]
pub fn convert_inbox_item_to_note(
    app: tauri::AppHandle,
    id: String,
) -> Result<ConvertInboxItemResponse, String> {
    let mut conn = get_connection(&app)?;
    let assets_dir = assets_dir(&app)?;
    let note_id = convert_item_to_note(&mut conn, &id, &assets_dir)?;
    Ok(ConvertInboxItemResponse { ok: true, item_id: id, note_id })
}

#[tauri::command]
pub fn open_inbox_attachment(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = get_connection(&app)?;
    let filename: String = conn
        .query_row("SELECT filename FROM inbox_attachments WHERE id = ?", [&id], |row| row.get(0))
        .map_err(|error| format!("无法找到附件: {error}"))?;
    let safe_name = Path::new(&filename)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == filename)
        .ok_or_else(|| "附件存储路径无效。".to_string())?;
    let path = assets_dir(&app)?.join(safe_name);
    open::that(path).map_err(|error| format!("打开附件失败: {error}"))
}

#[tauri::command]
pub fn open_inbox_link(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = get_connection(&app)?;
    let content: String = conn
        .query_row(
            "SELECT content FROM inbox_items WHERE id = ? AND kind = 'link'",
            [&id],
            |row| row.get(0),
        )
        .map_err(|error| format!("无法找到链接: {error}"))?;
    if !is_http_url(&content) {
        return Err("只允许打开 http 或 https 链接。".to_string());
    }
    open::that(content).map_err(|error| format!("打开链接失败: {error}"))
}

// ============================================================================
// Permanent delete + resource folder
// ============================================================================

/// Stored attachment filenames belonging to an item.
fn item_attachment_filenames(conn: &Connection, item_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT filename FROM inbox_attachments WHERE item_id = ? ORDER BY created_at, id")
        .map_err(|error| format!("Failed to prepare attachment query: {error}"))?;
    let rows = stmt
        .query_map([item_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query attachments: {error}"))?;
    rows.collect::<SqliteResult<Vec<_>>>()
        .map_err(|error| format!("Failed to collect attachments: {error}"))
}

/// Remove stored attachment files. Best-effort: a file that is already gone or
/// locked must not block the database delete. Path traversal names are ignored.
fn remove_attachment_files(directory: &Path, filenames: &[String]) -> usize {
    let mut removed = 0;
    for filename in filenames {
        let safe_name = Path::new(filename)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| *name == filename);
        let Some(safe_name) = safe_name else { continue };
        if fs::remove_file(directory.join(safe_name)).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Permanently delete an Inbox item and its attachments.
///
/// Files are kept when the item was already converted to a note: the note body
/// references them through `asset:` links, so removing them would break the
/// note. Unconverted items also drop the recorder's raw `.wav` (same filename)
/// so the audio directory does not grow without bound.
#[tauri::command]
pub fn delete_inbox_item(app: tauri::AppHandle, id: String) -> Result<InboxStateResponse, String> {
    let mut conn = get_connection(&app)?;
    let assets = assets_dir(&app)?;
    let recording_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取录音目录失败: {error}"))?
        .join("audio");
    let filenames = item_attachment_filenames(&conn, &id)?;
    let converted_note_id: Option<String> = conn
        .query_row(
            "SELECT converted_note_id FROM inbox_items WHERE id = ?",
            [&id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("无法找到收集项: {error}"))?
        .flatten();

    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to begin Inbox delete: {error}"))?;
    transaction
        .execute("DELETE FROM inbox_attachments WHERE item_id = ?", [&id])
        .map_err(|error| format!("Failed to delete Inbox attachments: {error}"))?;
    let removed = transaction
        .execute("DELETE FROM inbox_items WHERE id = ?", [&id])
        .map_err(|error| format!("Failed to delete Inbox item: {error}"))?;
    if removed == 0 {
        return Err("找不到要删除的收集项。".to_string());
    }
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit Inbox delete: {error}"))?;

    if converted_note_id.is_none() {
        remove_attachment_files(&assets, &filenames);
        remove_attachment_files(&recording_dir, &filenames);
    }

    Ok(InboxStateResponse {
        ok: true,
        item_id: id,
    })
}

/// Open the shared resource folder that stores Inbox / note attachments
/// (recordings, images and copied files) for manual management.
#[tauri::command]
pub fn open_inbox_assets_dir(app: tauri::AppHandle) -> Result<(), String> {
    let directory = assets_dir(&app)?;
    app.opener()
        .open_path(directory.to_string_lossy().to_string(), None::<String>)
        .map_err(|error| format!("打开资源文件夹失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        crate::notes::init_db(&conn).expect("notes schema");
        init_db(&conn).expect("inbox schema");
        conn
    }

    #[test]
    fn inbox_schema_is_additive_and_idempotent_for_existing_notes() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(
            "CREATE TABLE notes (id TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL, updated_at TEXT NOT NULL, is_pinned INTEGER NOT NULL DEFAULT 0); INSERT INTO notes (id, title, content, created_at, updated_at) VALUES ('old', 'Old note', 'preserved', '2026-01-01', '2026-01-02');",
        )
        .expect("legacy note schema");

        crate::notes::init_db(&conn).expect("migrate existing notes");
        init_db(&conn).expect("create inbox tables");
        init_db(&conn).expect("repeat migration");

        let note: (String, String) = conn
            .query_row("SELECT title, content FROM notes WHERE id = 'old'", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("old note remains");
        assert_eq!(note, ("Old note".to_string(), "preserved".to_string()));
        let inbox_table_count: i32 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'inbox_items'", [], |row| row.get(0))
            .expect("inbox table exists");
        assert_eq!(inbox_table_count, 1);
    }

    #[test]
    fn captured_items_keep_source_context_and_can_be_archived_and_restored() {
        let conn = memory_db();
        insert_item_row(
            &conn,
            "item-1",
            "text",
            "A note",
            "Remember this",
            "launcher",
            "clipboard",
            "2026-10-17T10:00:00Z",
        )
        .expect("insert item");
        conn.execute(
            "UPDATE inbox_items SET archived_at = '2026-10-17T11:00:00Z' WHERE id = 'item-1'",
            [],
        )
        .expect("archive item");
        let archived_at: Option<String> = conn
            .query_row("SELECT archived_at FROM inbox_items WHERE id = 'item-1'", [], |row| row.get(0))
            .expect("archived state");
        assert_eq!(archived_at.as_deref(), Some("2026-10-17T11:00:00Z"));
        conn.execute("UPDATE inbox_items SET archived_at = NULL WHERE id = 'item-1'", [])
            .expect("restore item");
        let (source, context, archived): (String, String, Option<String>) = conn
            .query_row(
                "SELECT source, context, archived_at FROM inbox_items WHERE id = 'item-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("restored item");
        assert_eq!(source, "launcher");
        assert_eq!(context, "clipboard");
        assert!(archived.is_none());
    }

    #[test]
    fn conversion_creates_a_note_and_persists_an_idempotent_link() {
        let mut conn = memory_db();
        let assets_dir = std::env::temp_dir().join(format!("easyflowhub-inbox-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&assets_dir).expect("test assets directory");
        insert_item_row(
            &conn,
            "item-convert",
            "text",
            "Captured title",
            "Captured body",
            "launcher",
            "clipboard",
            "2026-10-17T10:00:00Z",
        )
        .expect("insert item");
        let first_note_id = convert_item_to_note(&mut conn, "item-convert", &assets_dir).expect("first conversion");
        let second_note_id = convert_item_to_note(&mut conn, "item-convert", &assets_dir).expect("idempotent conversion");
        let (note_count, archived, note_id): (i32, Option<String>, String) = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM notes WHERE id = ?), archived_at, converted_note_id FROM inbox_items WHERE id = 'item-convert'",
                [&first_note_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("converted item");
        assert_eq!(note_count, 1);
        assert!(archived.is_some());
        assert_eq!(first_note_id, second_note_id);
        assert_eq!(note_id, first_note_id);
        let _ = fs::remove_dir_all(&assets_dir);
    }

    #[test]
    fn transcript_storage_round_trips_and_replaces_previous_text() {
        let mut conn = memory_db();
        let assets_dir = std::env::temp_dir().join(format!("easyflowhub-inbox-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&assets_dir).expect("test assets directory");
        insert_item_row(
            &conn,
            "item-transcript",
            "audio",
            "录音",
            "",
            "voice",
            "manager",
            "2026-10-19T10:00:00Z",
        )
        .expect("insert item");

        let attachment_id = store_transcript_at(&mut conn, &assets_dir, "item-transcript", "第一段转写", "whisper-1")
            .expect("first transcript should be stored");
        let stored = read_transcript_at(&conn, &assets_dir, "item-transcript")
            .expect("transcript should read back")
            .expect("transcript should exist");
        assert_eq!(stored.text, "第一段转写");
        assert_eq!(stored.model.as_deref(), Some("whisper-1"));
        assert_eq!(stored.attachment_id, attachment_id);

        // Re-transcribing replaces the previous transcript instead of appending.
        store_transcript_at(&mut conn, &assets_dir, "item-transcript", "第二段转写", "whisper-2")
            .expect("replacement transcript should be stored");
        let replaced = read_transcript_at(&conn, &assets_dir, "item-transcript")
            .expect("replaced transcript should read back")
            .expect("replaced transcript should exist");
        assert_eq!(replaced.text, "第二段转写");
        assert_eq!(replaced.model.as_deref(), Some("whisper-2"));
        let transcript_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM inbox_attachments WHERE item_id = 'item-transcript' AND kind = 'transcript'",
                [],
                |row| row.get(0),
            )
            .expect("transcript count");
        assert_eq!(transcript_count, 1);

        // Conversion embeds the transcript text into the note body.
        let note_id = convert_item_to_note(&mut conn, "item-transcript", &assets_dir).expect("conversion");
        let note_content: String = conn
            .query_row("SELECT content FROM notes WHERE id = ?", [&note_id], |row| row.get(0))
            .expect("note content");
        assert!(note_content.contains("第二段转写"));

        let _ = fs::remove_dir_all(&assets_dir);
    }

    #[test]
    fn transcript_display_names_round_trip_the_model() {
        assert_eq!(transcript_attachment_name("whisper-1"), "转写文稿（whisper-1）.txt");
        assert_eq!(transcript_attachment_name("  "), "转写文稿.txt");
        assert_eq!(transcript_model_from_name("转写文稿（whisper-1）.txt").as_deref(), Some("whisper-1"));
        assert_eq!(transcript_model_from_name("转写文稿.txt"), None);
    }

    #[test]
    fn legacy_inbox_tables_gain_audio_and_transcript_kinds() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(
            r#"
            CREATE TABLE inbox_items (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file')),
                title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'manager', context TEXT NOT NULL DEFAULT '',
                captured_at TEXT NOT NULL, archived_at TEXT DEFAULT NULL, converted_note_id TEXT DEFAULT NULL
            );
            INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at)
                VALUES ('legacy', 'text', 'Legacy', 'keep me', 'manager', 'manual', '2026-01-01T00:00:00Z');
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            "#,
        )
        .expect("legacy inbox schema");

        init_db(&conn).expect("migrate legacy inbox tables");

        let content: String = conn
            .query_row("SELECT content FROM inbox_items WHERE id = 'legacy'", [], |row| row.get(0))
            .expect("legacy item remains");
        assert_eq!(content, "keep me");
        conn.execute(
            "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at) VALUES ('t1', 'legacy', 'transcript', '转写文稿.txt', 'text/plain', 't1.txt', 4, '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("transcript kind accepted after migration");
    }

    #[test]
    fn image_capture_rejects_unsupported_and_mismatched_payloads() {
        assert!(decode_image_data_url("data:image/svg+xml;base64,PHN2Zz4=").is_err());
        let mismatched = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"not png");
        assert!(decode_image_data_url(&format!("data:image/png;base64,{mismatched}")).is_err());
        assert!(encoded_image_length_allowed(4, 3));
        assert!(!encoded_image_length_allowed(8, 3));
    }

    #[test]
    fn urls_and_attachment_names_are_restricted_to_safe_values() {
        assert!(is_http_url("https://example.com/path"));
        assert!(!is_http_url("http://?x"));
        assert!(!is_http_url("https:///path"));
        assert!(!is_http_url("javascript:alert(1)"));
        assert!(!is_http_url("https://example.com unsafe"));
        assert_eq!(clean_display_name("..\\secret.txt\n"), "..secret.txt");
        assert_eq!(extension_for_file(Path::new("report.tar.gz")), "gz");
    }

    #[test]
    fn attachment_storage_uses_a_generated_filename_and_staging_path() {
        let dir = std::env::temp_dir().join(format!("easyflowhub-inbox-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("test directory");
        let pending = prepare_attachment(
            &dir,
            "item-1",
            "file",
            "../report.txt".to_string(),
            "text/plain".to_string(),
            "txt",
            |path| {
                fs::write(path, b"file").map_err(|error| error.to_string())?;
                Ok(4)
            },
            "2026-10-17T10:00:00Z",
        )
        .expect("staged attachment");

        assert_eq!(pending.attachment.name, "..report.txt");
        assert!(!pending.attachment.filename.contains('/') && !pending.attachment.filename.contains('\\'));
        assert_ne!(pending.attachment.filename, "../report.txt");
        assert!(pending.temporary_path.exists());
        fs::rename(&pending.temporary_path, &pending.final_path).expect("promote attachment");
        assert!(pending.final_path.exists());
        cleanup_pending(&[pending]);
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn remove_attachment_files_deletes_only_safe_existing_files() {
        let dir = std::env::temp_dir().join(format!("easyflowhub-inbox-delete-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("test directory");
        fs::write(dir.join("kept.wav"), b"audio").expect("write attachment");

        let removed = remove_attachment_files(
            &dir,
            &[
                "kept.wav".to_string(),
                "missing.wav".to_string(),
                "../escape.wav".to_string(),
                "nested/dir.wav".to_string(),
            ],
        );

        assert_eq!(removed, 1);
        assert!(!dir.join("kept.wav").exists());
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn item_attachment_filenames_only_returns_the_item_files() {
        let conn = memory_db();
        for item in ["item-a", "item-b"] {
            conn.execute(
                "INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at) VALUES (?, 'audio', '', '', 'manager', '', '2026-10-19T09:00:00Z')",
                [item],
            )
            .expect("insert item");
        }
        for (attachment, item, filename) in [
            ("att-1", "item-a", "item-a.wav"),
            ("att-2", "item-a", "item-a-transcript.txt"),
            ("att-3", "item-b", "item-b.wav"),
        ] {
            conn.execute(
                "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at) VALUES (?, ?, 'audio', ?, 'audio/wav', ?, 10, '2026-10-19T09:00:00Z')",
                params![attachment, item, filename, filename],
            )
            .expect("insert attachment");
        }

        let mut filenames = item_attachment_filenames(&conn, "item-a").expect("list filenames");
        filenames.sort();
        assert_eq!(filenames, vec!["item-a-transcript.txt", "item-a.wav"]);
    }
}
