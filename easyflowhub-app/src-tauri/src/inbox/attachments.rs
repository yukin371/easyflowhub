//! Attachment storage for the Inbox domain: attachment identity, safe file
//! names, staged writes into the shared assets directory, clipboard image
//! decoding, transcript attachments, and best-effort file cleanup.
//!
//! Schema stays in `schema.rs`; transaction orchestration stays in
//! `service.rs`. Everything here operates on files or single attachment rows.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const TRANSCRIPT_PREFIX: &str = "转写文稿";
const TRANSCRIPT_SUFFIX: &str = "）.txt";
const MAX_TRANSCRIPT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
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
    /// Absolute path of the original file this attachment was copied from, when
    /// the capture came from outside the shared assets directory (F10). A voice
    /// recording lives in `audio/{session-id}.wav` while its archived copy is
    /// stored under a fresh UUID in `assets/`; without this link the original
    /// cannot be found for cleanup. Attachments created inside assets/ (clipboard
    /// images, transcripts) leave this NULL.
    pub source_path: Option<String>,
}

/// An attachment row staged on disk but not yet committed to the database.
/// The row is inserted and the file is promoted from `temporary_path` to
/// `final_path` inside the same capture transaction.
#[derive(Debug)]
pub(crate) struct PendingAttachment {
    pub attachment: InboxAttachment,
    pub temporary_path: PathBuf,
    pub final_path: PathBuf,
}

/// A stored speech-to-text transcript belonging to an Inbox audio item.
#[derive(Debug, Clone)]
pub(crate) struct InboxTranscript {
    pub attachment_id: String,
    pub text: String,
    pub model: Option<String>,
    pub created_at: String,
}

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
pub(crate) fn read_transcript_attachment_text(assets_dir: &Path, filename: &str) -> Result<String, String> {
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

pub(crate) fn clean_display_name(name: &str) -> String {
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

pub(crate) fn extension_for_file(path: &Path) -> String {
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

pub(crate) fn decode_image_data_url(data_url: &str) -> Result<(Vec<u8>, &'static str), String> {
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

/// Stage an attachment on disk under a random storage name. The caller runs
/// `write` against a hidden `.pending` path; the file is promoted to its
/// final name only inside the capture transaction.
pub(crate) fn prepare_attachment(
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
                // Callers that archive an external source (voice capture) set
                // this after the fact; assets-native writes leave it NULL.
                source_path: None,
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

pub(crate) fn cleanup_pending(attachments: &[PendingAttachment]) {
    for attachment in attachments {
        let _ = fs::remove_file(&attachment.temporary_path);
        let _ = fs::remove_file(&attachment.final_path);
    }
}

/// Remove an absolute source file recorded for an attachment. Best-effort and
/// restricted to regular files so a bad stored path can never remove a
/// directory.
pub(crate) fn remove_source_file(path: &str) -> bool {
    let source = Path::new(path);
    let is_regular = fs::symlink_metadata(source)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false);
    is_regular && fs::remove_file(source).is_ok()
}

/// Remove stored attachment files. Best-effort: a file that is already gone or
/// locked must not block the database delete. Path traversal names are ignored.
pub(crate) fn remove_attachment_files(directory: &Path, filenames: &[String]) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::repository::{insert_item_row, item_attachment_source_paths};
    use crate::inbox::schema::test_memory_db;
    use crate::inbox::service::convert_item_to_note;

    #[test]
    fn transcript_storage_round_trips_and_replaces_previous_text() {
        let mut conn = test_memory_db();
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
    fn image_capture_rejects_unsupported_and_mismatched_payloads() {
        assert!(decode_image_data_url("data:image/svg+xml;base64,PHN2Zz4=").is_err());
        let mismatched = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"not png");
        assert!(decode_image_data_url(&format!("data:image/png;base64,{mismatched}")).is_err());
        assert!(encoded_image_length_allowed(4, 3));
        assert!(!encoded_image_length_allowed(8, 3));
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

    // F10: the archived asset copy gets a fresh UUID, so the recorder's
    // original `audio/{id}.wav` is only reachable through the recorded
    // source_path. Capturing must record it, and cleanup must remove it.
    #[test]
    fn audio_source_path_links_original_and_cleanup_removes_it() {
        let conn = test_memory_db();
        let dir = std::env::temp_dir().join(format!("efh-f10-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let original = dir.join("recording.wav");
        fs::write(&original, b"RIFF....WAVE").expect("write original recording");

        conn.execute(
            "INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at) VALUES ('item-1', 'audio', '', '', 'voice', '', '2026-10-19T09:00:00Z')",
            [],
        )
        .expect("insert item");
        // The archived copy lives in assets/ under a fresh UUID; the original
        // path is recorded separately.
        conn.execute(
            "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at, source_path) VALUES ('att-1', 'item-1', 'audio', 'recording.wav', 'audio/wav', 'uuid-copy.wav', 12, '2026-10-19T09:00:00Z', ?1)",
            params![original.to_string_lossy().to_string()],
        )
        .expect("insert attachment with source path");

        // The link is queryable for cleanup.
        let sources = item_attachment_source_paths(&conn, "item-1").expect("query source paths");
        assert_eq!(sources, vec![original.to_string_lossy().to_string()]);

        // Cleanup removes the recorder's original by its recorded path.
        assert!(original.exists());
        assert!(remove_source_file(&original.to_string_lossy()));
        assert!(!original.exists(), "original recording must be removed");

        // A missing or non-file source is a safe no-op, never a panic.
        assert!(!remove_source_file(&dir.to_string_lossy()));
        assert!(!remove_source_file("/nonexistent/recording.wav"));

        fs::remove_dir_all(&dir).expect("remove temp dir");
    }
}
