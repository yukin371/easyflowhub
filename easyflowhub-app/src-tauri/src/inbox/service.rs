//! Inbox capture / conversion / deletion use cases.
//!
//! Commands in `commands.rs` stay thin: all validation, staging, transaction
//! orchestration and resource-identity decisions (F10) live here so no command
//! re-implements them.

use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult, TransactionBehavior};
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

/// Soft-delete an Inbox item. Attachments stay available for recovery.
pub(crate) fn trash_item(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let conn = repository::get_connection(app)?;
    trash_item_with_conn(&conn, id)
}

fn trash_item_with_conn(conn: &Connection, id: &str) -> Result<(), String> {
    if repository::mark_deleted(conn, id)? {
        Ok(())
    } else {
        Err("找不到未删除的收集项。".to_string())
    }
}

/// Restore a trashed Inbox item while preserving its archive state and files.
pub(crate) fn restore_item_from_trash(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let conn = repository::get_connection(app)?;
    restore_item_from_trash_with_conn(&conn, id)
}

fn restore_item_from_trash_with_conn(conn: &Connection, id: &str) -> Result<(), String> {
    if repository::restore_deleted(conn, id)? {
        Ok(())
    } else {
        Err("找不到回收站中的收集项。".to_string())
    }
}

/// List Inbox trash and apply the same retention setting used by note trash.
pub(crate) fn list_trash_items(app: &tauri::AppHandle) -> Result<Vec<InboxItem>, String> {
    if let Ok(settings) = crate::settings::get_settings(app.clone()) {
        let _ = cleanup_expired_trash(app, settings.settings.trash.retention_days);
    }
    let conn = repository::get_connection(app)?;
    repository::list_trash_items(&conn)
}

/// Permanently remove a trashed Inbox item and its unreferenced files.
pub(crate) fn permanently_delete_item(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let mut conn = repository::get_connection(app)?;
    let assets_dir = repository::assets_dir(app)?;
    let recording_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取录音目录失败: {error}"))?
        .join("audio");
    purge_trash_items(
        &mut conn,
        &assets_dir,
        &recording_dir,
        TrashPurgeFilter::Item(id),
    )?;
    Ok(())
}

/// Permanently remove all Inbox trash using the same resource rules as a
/// single-item purge.
pub(crate) fn empty_trash(app: &tauri::AppHandle) -> Result<usize, String> {
    let mut conn = repository::get_connection(app)?;
    let assets_dir = repository::assets_dir(app)?;
    let recording_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取录音目录失败: {error}"))?
        .join("audio");
    purge_trash_items(
        &mut conn,
        &assets_dir,
        &recording_dir,
        TrashPurgeFilter::All,
    )
}

fn cleanup_expired_trash(app: &tauri::AppHandle, retention_days: u32) -> Result<usize, String> {
    if retention_days == 0 {
        return Ok(0);
    }
    let mut conn = repository::get_connection(app)?;
    let assets_dir = repository::assets_dir(app)?;
    let recording_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取录音目录失败: {error}"))?
        .join("audio");
    let cutoff = (Utc::now() - ChronoDuration::days(retention_days as i64)).to_rfc3339();
    purge_trash_items(
        &mut conn,
        &assets_dir,
        &recording_dir,
        TrashPurgeFilter::OlderThan(&cutoff),
    )
}

enum TrashPurgeFilter<'a> {
    Item(&'a str),
    All,
    OlderThan(&'a str),
}

struct PurgedItemFiles {
    filenames: Vec<String>,
    source_paths: Vec<String>,
    converted: bool,
}

fn purge_trash_items(
    conn: &mut Connection,
    assets_dir: &Path,
    recording_dir: &Path,
    filter: TrashPurgeFilter<'_>,
) -> Result<usize, String> {
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Failed to begin Inbox trash purge: {error}"))?;
    let item_ids = match filter {
        TrashPurgeFilter::Item(id) => {
            let deleted_at = transaction
                .query_row(
                    "SELECT deleted_at FROM inbox_items WHERE id = ?",
                    [id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(|error| format!("Failed to find Inbox trash item: {error}"))?;
            match deleted_at {
                Some(Some(_)) => vec![id.to_string()],
                Some(None) => return Err("收集项尚未移入回收站。".to_string()),
                None => return Err("找不到回收站中的收集项。".to_string()),
            }
        }
        TrashPurgeFilter::All => query_trashed_item_ids(&transaction, None)?,
        TrashPurgeFilter::OlderThan(cutoff) => query_trashed_item_ids(&transaction, Some(cutoff))?,
    };

    let mut purged_files = Vec::with_capacity(item_ids.len());
    for id in &item_ids {
        purged_files.push(PurgedItemFiles {
            filenames: item_attachment_filenames(&transaction, id)?,
            source_paths: item_attachment_source_paths(&transaction, id)?,
            converted: item_converted_note_id(&transaction, id)?.is_some(),
        });
        transaction
            .execute("DELETE FROM inbox_attachments WHERE item_id = ?", [id])
            .map_err(|error| format!("Failed to delete Inbox attachments: {error}"))?;
        transaction
            .execute(
                "DELETE FROM inbox_items WHERE id = ? AND deleted_at IS NOT NULL",
                [id],
            )
            .map_err(|error| format!("Failed to delete Inbox item: {error}"))?;
    }
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit Inbox trash purge: {error}"))?;

    for item in &purged_files {
        if !item.converted {
            remove_attachment_files(assets_dir, &item.filenames);
            remove_attachment_files(recording_dir, &item.filenames);
            // The assets copy has a fresh UUID; only source_path links back to
            // the recorder's original audio file.
            for source_path in &item.source_paths {
                remove_source_file(source_path);
            }
        }
    }
    Ok(item_ids.len())
}

fn query_trashed_item_ids(
    conn: &Connection,
    older_than: Option<&str>,
) -> Result<Vec<String>, String> {
    if let Some(cutoff) = older_than {
        let mut stmt = conn
            .prepare("SELECT id FROM inbox_items WHERE deleted_at IS NOT NULL AND deleted_at < ? ORDER BY deleted_at, id")
            .map_err(|error| format!("Failed to prepare expired Inbox trash query: {error}"))?;
        let rows = stmt
            .query_map([cutoff], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Failed to query expired Inbox trash: {error}"))?;
        rows
            .collect::<SqliteResult<Vec<_>>>()
            .map_err(|error| format!("Failed to collect expired Inbox trash: {error}"))
    } else {
        let mut stmt = conn
            .prepare("SELECT id FROM inbox_items WHERE deleted_at IS NOT NULL ORDER BY deleted_at, id")
            .map_err(|error| format!("Failed to prepare Inbox trash query: {error}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Failed to query Inbox trash: {error}"))?;
        rows
            .collect::<SqliteResult<Vec<_>>>()
            .map_err(|error| format!("Failed to collect Inbox trash: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::attachments::{clean_display_name, extension_for_file};
    use crate::inbox::schema::test_memory_db;
    use std::path::{Path as StdPath, PathBuf};

    fn test_directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("easyflowhub-inbox-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).expect("create test directory");
        path
    }

    fn insert_attachment(
        conn: &Connection,
        id: &str,
        item_id: &str,
        kind: &str,
        filename: &str,
        source_path: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at, source_path) VALUES (?, ?, ?, ?, ?, ?, 4, '2026-10-20T00:00:00Z', ?)",
            rusqlite::params![id, item_id, kind, filename, if kind == "image" { "image/png" } else { "audio/wav" }, filename, source_path],
        )
        .expect("insert test attachment");
    }

    fn insert_test_item(conn: &Connection, id: &str, kind: &str, title: &str, source: &str, context: &str) {
        insert_item_row(conn, id, kind, title, "", source, context, "2026-10-20T00:00:00Z")
            .expect("insert test item");
    }

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

    #[test]
    fn soft_delete_hides_item_but_keeps_attachments_and_restore_keeps_archive_state() {
        let conn = test_memory_db();
        let assets_dir = test_directory("restore-assets");
        let filename = "capture.png";
        fs::write(assets_dir.join(filename), b"image").expect("write image");
        insert_test_item(&conn, "item-restore", "image", "Captured image", "manager", "clipboard");
        insert_attachment(&conn, "att-restore", "item-restore", "image", filename, None);
        conn.execute(
            "UPDATE inbox_items SET archived_at = '2026-10-19T12:00:00Z' WHERE id = 'item-restore'",
            [],
        )
        .expect("archive item");

        trash_item_with_conn(&conn, "item-restore").expect("soft delete item");
        assert!(repository::list_items(&conn, false).expect("active items").is_empty());
        assert!(repository::list_items(&conn, true).expect("all active items").is_empty());
        let trashed = repository::list_trash_items(&conn).expect("list Inbox trash");
        assert_eq!(trashed.len(), 1);
        assert!(trashed[0].deleted_at.is_some());
        assert_eq!(trashed[0].attachments.len(), 1);
        assert!(assets_dir.join(filename).exists());

        restore_item_from_trash_with_conn(&conn, "item-restore").expect("restore item");
        let restored = repository::list_items(&conn, true).expect("restored active items");
        assert_eq!(restored.len(), 1);
        assert!(restored[0].deleted_at.is_none());
        assert_eq!(restored[0].archived_at.as_deref(), Some("2026-10-19T12:00:00Z"));
        assert_eq!(restored[0].attachments.len(), 1);
        assert!(assets_dir.join(filename).exists());
        let _ = fs::remove_dir_all(assets_dir);
    }

    #[test]
    fn single_purge_removes_unconverted_attachment_and_recording_source_files() {
        let mut conn = test_memory_db();
        let assets_dir = test_directory("purge-assets");
        let recording_dir = test_directory("purge-recordings");
        let source_dir = test_directory("purge-source");
        let filename = "stored.wav";
        let source_path = source_dir.join("original.wav");
        fs::write(assets_dir.join(filename), b"asset copy").expect("write asset copy");
        fs::write(recording_dir.join(filename), b"same-name recording").expect("write recording copy");
        fs::write(&source_path, b"original recording").expect("write recording source");
        insert_test_item(&conn, "item-purge", "audio", "Recording", "voice", "manager");
        insert_attachment(
            &conn,
            "att-purge",
            "item-purge",
            "audio",
            filename,
            Some(&source_path.to_string_lossy()),
        );
        trash_item_with_conn(&conn, "item-purge").expect("soft delete item");

        let purged = purge_trash_items(
            &mut conn,
            &assets_dir,
            &recording_dir,
            TrashPurgeFilter::Item("item-purge"),
        )
        .expect("permanently purge item");
        assert_eq!(purged, 1);
        assert!(!assets_dir.join(filename).exists());
        assert!(!recording_dir.join(filename).exists());
        assert!(!source_path.exists());
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM inbox_items WHERE id = 'item-purge'", [], |row| row.get(0))
            .expect("count purged item");
        assert_eq!(remaining, 0);
        let _ = fs::remove_dir_all(assets_dir);
        let _ = fs::remove_dir_all(recording_dir);
        let _ = fs::remove_dir_all(source_dir);
    }

    #[test]
    fn empty_trash_uses_purge_rules_and_keeps_converted_note_assets() {
        let mut conn = test_memory_db();
        let assets_dir = test_directory("empty-assets");
        let recording_dir = test_directory("empty-recordings");
        let source_dir = test_directory("empty-source");

        let unconverted_source = source_dir.join("unconverted.wav");
        fs::write(assets_dir.join("unconverted.wav"), b"asset copy").expect("write unconverted asset");
        fs::write(recording_dir.join("unconverted.wav"), b"recording copy").expect("write unconverted recording");
        fs::write(&unconverted_source, b"original recording").expect("write unconverted source");
        insert_test_item(&conn, "item-unconverted", "audio", "Unconverted audio", "voice", "manager");
        insert_attachment(
            &conn,
            "att-unconverted",
            "item-unconverted",
            "audio",
            "unconverted.wav",
            Some(&unconverted_source.to_string_lossy()),
        );

        fs::write(assets_dir.join("converted.png"), b"converted asset").expect("write converted asset");
        insert_test_item(&conn, "item-converted-trash", "image", "Converted image", "manager", "clipboard");
        insert_attachment(
            &conn,
            "att-converted",
            "item-converted-trash",
            "image",
            "converted.png",
            None,
        );
        let note_id = convert_item_to_note(&mut conn, "item-converted-trash", &assets_dir)
            .expect("convert item to note");

        trash_item_with_conn(&conn, "item-unconverted").expect("trash unconverted item");
        trash_item_with_conn(&conn, "item-converted-trash").expect("trash converted item");
        let purged = purge_trash_items(
            &mut conn,
            &assets_dir,
            &recording_dir,
            TrashPurgeFilter::All,
        )
        .expect("empty Inbox trash");

        assert_eq!(purged, 2);
        assert!(!assets_dir.join("unconverted.wav").exists());
        assert!(!recording_dir.join("unconverted.wav").exists());
        assert!(!unconverted_source.exists());
        assert!(assets_dir.join("converted.png").exists());
        let note_content: String = conn
            .query_row("SELECT content FROM notes WHERE id = ?", [&note_id], |row| row.get(0))
            .expect("converted note remains");
        assert!(note_content.contains("asset:converted.png"));
        let remaining_items: i64 = conn
            .query_row("SELECT COUNT(*) FROM inbox_items WHERE deleted_at IS NOT NULL", [], |row| row.get(0))
            .expect("count Inbox trash after empty");
        assert_eq!(remaining_items, 0);

        let _ = fs::remove_dir_all(assets_dir);
        let _ = fs::remove_dir_all(recording_dir);
        let _ = fs::remove_dir_all(source_dir);
    }
}
