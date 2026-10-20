//! Inbox capture / conversion / deletion use cases.
//!
//! Commands in `commands.rs` stay thin: all validation, staging, transaction
//! orchestration and resource-identity decisions (F10) live here so no command
//! re-implements them.

use chrono::Utc;
use rusqlite::{params, Connection, Result as SqliteResult};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tauri::Manager;
use url::Url;
use uuid::Uuid;

use super::attachments::{
    cleanup_pending, decode_image_data_url, extension_for_file, prepare_attachment,
    read_transcript_attachment_text, remove_attachment_files, remove_source_file,
};
use super::repository::{
    self, get_item, insert_item_row, item_attachment_filenames, item_attachment_source_paths,
    item_converted_note_id, InboxItem,
};

const MAX_TEXT_CHARS: usize = 1_000_000;
const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;

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

pub(crate) fn is_http_url(value: &str) -> bool {
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

/// Validate a capture request, stage every attachment on disk and commit the
/// item plus its attachment rows in one transaction. Attachment files are
/// promoted from their `.pending` staging names only after the rows are
/// written, and any failure removes both staged files and rows.
pub(crate) fn capture_item(
    app: &tauri::AppHandle,
    input: CaptureInboxItemInput,
) -> Result<InboxItem, String> {
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

    let assets_dir = repository::assets_dir(app)?;
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
            Ok(mut attachment) => {
                // Record where the archived copy came from (F10). The voice
                // recorder owns `audio/{session-id}.wav`; the assets copy is a
                // fresh UUID, so without this link the original is unreachable
                // for cleanup. Only the assets copy is stored in `filename`.
                attachment.attachment.source_path =
                    Some(path.to_string_lossy().into_owned());
                pending.push(attachment);
            }
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

    let mut conn = match repository::get_connection(app) {
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
                        (id, item_id, kind, name, mime_type, filename, size_bytes, created_at, source_path)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                    params![
                        attachment.id,
                        attachment.item_id,
                        attachment.kind,
                        attachment.name,
                        attachment.mime_type,
                        attachment.filename,
                        attachment.size_bytes as i64,
                        attachment.created_at,
                        attachment.source_path
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

    get_item(&conn, &id)?
        .ok_or_else(|| "Inbox 项目保存后无法重新读取。".to_string())
}

/// Convert an Inbox item into a note. The note insert and the
/// `converted_note_id` link happen in the same transaction; converting twice
/// returns the original note instead of creating a copy.
pub(crate) fn convert_item_to_note(
    conn: &mut Connection,
    id: &str,
    assets_dir: &Path,
) -> Result<String, String> {
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

/// Permanently delete an Inbox item and its attachments.
///
/// Files are kept when the item was already converted to a note: the note body
/// references them through `asset:` links, so removing them would break the
/// note. Unconverted items also drop the recorder's raw `.wav` (same filename)
/// so the audio directory does not grow without bound.
pub(crate) fn delete_item(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let mut conn = repository::get_connection(app)?;
    let assets = repository::assets_dir(app)?;
    let recording_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取录音目录失败: {error}"))?
        .join("audio");
    let filenames = item_attachment_filenames(&conn, id)?;
    let source_paths = item_attachment_source_paths(&conn, id)?;
    let converted_note_id = item_converted_note_id(&conn, id)?;

    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to begin Inbox delete: {error}"))?;
    transaction
        .execute("DELETE FROM inbox_attachments WHERE item_id = ?", [id])
        .map_err(|error| format!("Failed to delete Inbox attachments: {error}"))?;
    let removed = transaction
        .execute("DELETE FROM inbox_items WHERE id = ?", [id])
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
        // Remove the recorder's originals too (F10). The assets copy above is a
        // fresh UUID and cannot reach `audio/{session-id}.wav`; the recorded
        // source_path is the only link back to the original recording.
        for source_path in &source_paths {
            remove_source_file(source_path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::attachments::{clean_display_name, extension_for_file};
    use crate::inbox::schema::test_memory_db;
    use std::path::Path as StdPath;

    #[test]
    fn conversion_creates_a_note_and_persists_an_idempotent_link() {
        let mut conn = test_memory_db();
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
    fn urls_and_attachment_names_are_restricted_to_safe_values() {
        assert!(is_http_url("https://example.com/path"));
        assert!(!is_http_url("http://?x"));
        assert!(!is_http_url("https:///path"));
        assert!(!is_http_url("javascript:alert(1)"));
        assert!(!is_http_url("https://example.com unsafe"));
        assert_eq!(clean_display_name("..\\secret.txt\n"), "..secret.txt");
        assert_eq!(extension_for_file(StdPath::new("report.tar.gz")), "gz");
    }
}
