use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Result as SqliteResult, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;
use uuid::Uuid;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_pinned: bool,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NoteInput {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub is_pinned: Option<bool>,
    pub deleted_at: Option<String>,
    /// Compare-and-swap guard (F05). When set, the update only applies if
    /// the stored `updated_at` still equals this value; a mismatch means
    /// another window wrote the note since it was read, and the save is
    /// rejected instead of silently overwriting that write. Omitting the
    /// field keeps the previous last-write-wins behaviour, so existing
    /// callers are unaffected.
    #[serde(default)]
    pub expected_updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListNotesResponse {
    pub ok: bool,
    pub count: usize,
    pub notes: Vec<Note>,
}

#[derive(Debug, Serialize)]
pub struct GetNoteResponse {
    pub ok: bool,
    pub note: Option<Note>,
}

#[derive(Debug, Serialize)]
pub struct SaveNoteResponse {
    pub ok: bool,
    pub note: Note,
}

#[derive(Debug, Serialize)]
pub struct DeleteNoteResponse {
    pub ok: bool,
    pub deleted_id: String,
}

#[derive(Debug, Serialize)]
pub struct SearchNotesResponse {
    pub ok: bool,
    pub count: usize,
    pub query: String,
    pub notes: Vec<Note>,
}

// ============================================================================
// Database Management
// ============================================================================

/// Bring a connection to the schema this build expects.
///
/// The DDL and every migration live in `hub-core::store` now, because the native
/// shell opens the same file. This stays as a named seam for the in-memory databases
/// the unit tests build, so no test has to know the ladder's module path.
///
/// Test-only: production connections come from `crate::db::connection`, which runs
/// the same ladder plus the connection policy a bare `init_db` would not.
#[cfg(test)]
pub(crate) fn init_db(conn: &mut Connection) -> SqliteResult<()> {
    hub_core::store::init_schema(conn, hub_core::store::Rebuild::Run).map(|_| ())
}

/// Get or create a database connection.
pub(crate) fn get_connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    crate::db::connection(app)
}

// ============================================================================
// Commands
// ============================================================================

/// List all notes (excluding deleted), sorted by pinned first, then by updated_at descending
#[tauri::command]
pub fn list_notes(app: tauri::AppHandle) -> Result<ListNotesResponse, String> {
    let conn = get_connection(&app)?;
    let notes = list_notes_with_conn(&conn)?;
    Ok(ListNotesResponse {
        ok: true,
        count: notes.len(),
        notes,
    })
}

/// The list query, on a caller-supplied connection.
///
/// Split out of the command so it can run against a database this process did not
/// create -- that is what the S3 compatibility test needs: another implementation's
/// file, read by this exact code path instead of a copy of the SQL.
fn list_notes_with_conn(conn: &Connection) -> Result<Vec<Note>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at
            FROM notes
            WHERE deleted_at IS NULL
            ORDER BY is_pinned DESC, updated_at DESC
            "#,
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let mapped = stmt
        .query_map([], |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tags: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                is_pinned: row.get::<_, i32>(6)? != 0,
                deleted_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("Failed to query notes: {}", e))?;
    let notes = mapped
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|e| format!("Failed to collect notes: {}", e))?;
    Ok(notes)
}

/// Get a single note by ID
#[tauri::command]
pub fn get_note(app: tauri::AppHandle, id: String) -> Result<GetNoteResponse, String> {
    let conn = get_connection(&app)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at
            FROM notes
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let result = stmt
        .query_row([&id], |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tags: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                is_pinned: row.get::<_, i32>(6)? != 0,
                deleted_at: row.get(7)?,
            })
        });

    match result {
        Ok(note) => Ok(GetNoteResponse {
            ok: true,
            note: Some(note),
        }),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(GetNoteResponse {
            ok: true,
            note: None,
        }),
        Err(e) => Err(format!("Failed to get note: {}", e)),
    }
}

/// Save a note (create or update)
#[tauri::command]
pub fn save_note(app: tauri::AppHandle, note: NoteInput) -> Result<SaveNoteResponse, String> {
    let conn = get_connection(&app)?;
    save_note_with_conn(&conn, note)
}

/// Core save logic, separated from the Tauri handle so the compare-and-swap
/// behaviour (F05) can be exercised against an in-memory database in tests.
fn save_note_with_conn(conn: &Connection, note: NoteInput) -> Result<SaveNoteResponse, String> {
    let now = Utc::now().to_rfc3339();

    // Check if note exists
    let exists: bool = conn
        .query_row("SELECT 1 FROM notes WHERE id = ?", [&note.id], |_| Ok(true))
        .unwrap_or(false);

    let merged_note = if exists {
        let existing = get_note_helper(&conn, &note.id)?;
        Note {
            id: existing.id,
            title: note.title.unwrap_or(existing.title),
            content: note.content.unwrap_or(existing.content),
            tags: note.tags.unwrap_or(existing.tags),
            created_at: existing.created_at,
            updated_at: now.clone(),
            is_pinned: note.is_pinned.unwrap_or(existing.is_pinned),
            deleted_at: existing.deleted_at,
        }
    } else {
        Note {
            id: if note.id.is_empty() {
                Uuid::new_v4().to_string()
            } else {
                note.id
            },
            title: note.title.unwrap_or_default(),
            content: note.content.unwrap_or_default(),
            tags: note.tags.unwrap_or_default(),
            created_at: note.created_at.unwrap_or_else(|| now.clone()),
            updated_at: now.clone(),
            is_pinned: note.is_pinned.unwrap_or(false),
            deleted_at: None,
        }
    };

    // A brand-new empty note is not persisted: autosave fires shortly after a
    // note window opens, and an empty shell in the list would be noise. An
    // *existing* note must still be allowed to become empty — skipping the
    // write here used to leave the stale text in SQLite, so clearing the
    // editor only looked like it worked and the old content came back on the
    // next open.
    if !exists && merged_note.content.trim().is_empty() && merged_note.title.trim().is_empty() {
        return Ok(SaveNoteResponse {
            ok: true,
            note: merged_note,
        });
    }

    if exists {
        // Update existing note. With a compare-and-swap guard the UPDATE is
        // conditioned on the stored revision; zero affected rows then means
        // the note changed since the caller read it (F05).
        let affected = match note.expected_updated_at.as_deref() {
            Some(expected) => conn
                .execute(
                    r#"
                    UPDATE notes
                    SET title = ?, content = ?, tags = ?, updated_at = ?, is_pinned = ?
                    WHERE id = ? AND updated_at = ?
                    "#,
                    rusqlite::params![
                        merged_note.title,
                        merged_note.content,
                        merged_note.tags,
                        now,
                        merged_note.is_pinned as i32,
                        merged_note.id,
                        expected
                    ],
                )
                .map_err(|e| format!("Failed to update note: {}", e))?,
            None => conn
                .execute(
                    r#"
                    UPDATE notes
                    SET title = ?, content = ?, tags = ?, updated_at = ?, is_pinned = ?
                    WHERE id = ?
                    "#,
                    rusqlite::params![
                        merged_note.title,
                        merged_note.content,
                        merged_note.tags,
                        now,
                        merged_note.is_pinned as i32,
                        merged_note.id
                    ],
                )
                .map_err(|e| format!("Failed to update note: {}", e))?,
        };

        if affected == 0 && note.expected_updated_at.is_some() {
            return Err(format!(
                "note_conflict: {} was modified by another window since it was read",
                merged_note.id
            ));
        }

        // Fetch the updated note
        let updated_note = get_note_helper(&conn, &merged_note.id)?;
        Ok(SaveNoteResponse {
            ok: true,
            note: updated_note,
        })
    } else {
        // Create new note
        conn.execute(
            r#"
            INSERT INTO notes (id, title, content, tags, created_at, updated_at, is_pinned)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            rusqlite::params![
                merged_note.id,
                merged_note.title,
                merged_note.content,
                merged_note.tags,
                merged_note.created_at,
                merged_note.updated_at,
                merged_note.is_pinned as i32
            ],
        )
        .map_err(|e| format!("Failed to insert note: {}", e))?;

        Ok(SaveNoteResponse {
            ok: true,
            note: merged_note,
        })
    }
}

/// Helper to get a note from an existing connection
fn get_note_helper(conn: &Connection, id: &str) -> Result<Note, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at FROM notes WHERE id = ?",
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    stmt.query_row([id], |row| {
        note_from_row(row)
    })
    .map_err(|e| format!("Failed to get note: {}", e))
}

fn note_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        tags: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        is_pinned: row.get::<_, i32>(6)? != 0,
        deleted_at: row.get(7)?,
    })
}

/// Delete a note by ID
#[tauri::command]
pub fn delete_note(app: tauri::AppHandle, id: String) -> Result<DeleteNoteResponse, String> {
    let conn = get_connection(&app)?;

    conn.execute("DELETE FROM notes WHERE id = ?", [&id])
        .map_err(|e| format!("Failed to delete note: {}", e))?;

    Ok(DeleteNoteResponse {
        ok: true,
        deleted_id: id,
    })
}

/// Search notes by title, content or tags (excluding deleted)
#[tauri::command]
pub fn search_notes(
    app: tauri::AppHandle,
    query: String,
) -> Result<SearchNotesResponse, String> {
    let conn = get_connection(&app)?;

    let search_pattern = format!("%{}%", query);

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at
            FROM notes
            WHERE deleted_at IS NULL AND (title LIKE ? OR content LIKE ? OR tags LIKE ?)
            ORDER BY is_pinned DESC, updated_at DESC
            LIMIT 100
            "#,
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let notes = stmt
        .query_map(rusqlite::params![&search_pattern, &search_pattern, &search_pattern], |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tags: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                is_pinned: row.get::<_, i32>(6)? != 0,
                deleted_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("Failed to search notes: {}", e))?
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|e| format!("Failed to collect notes: {}", e))?;

    let count = notes.len();
    Ok(SearchNotesResponse {
        ok: true,
        count,
        query,
        notes,
    })
}

/// Get the assets directory path for images
pub(crate) fn assets_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let assets_dir = app_data.join("assets");
    fs::create_dir_all(&assets_dir).map_err(|e| format!("Failed to create assets dir: {}", e))?;
    Ok(assets_dir)
}

/// Response for save_image command
#[derive(Debug, Serialize)]
pub struct SaveImageResponse {
    pub ok: bool,
    pub filename: String,
    pub path: String,
}

fn extension_from_path(path: &Path) -> &str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "png",
        Some("jpg") | Some("jpeg") => "jpg",
        Some("gif") => "gif",
        Some("webp") => "webp",
        Some("svg") => "svg",
        Some("bmp") => "bmp",
        _ => "png",
    }
}

fn persist_image_bytes(app: &tauri::AppHandle, image_data: &[u8], ext: &str) -> Result<SaveImageResponse, String> {
    let filename = format!("{}.{}", Uuid::new_v4(), ext);
    let assets_dir = assets_path(app)?;
    let file_path = assets_dir.join(&filename);

    fs::write(&file_path, image_data).map_err(|e| format!("Failed to write image: {}", e))?;

    Ok(SaveImageResponse {
        ok: true,
        filename,
        path: file_path.to_string_lossy().to_string(),
    })
}

/// Save an image to assets directory
#[tauri::command]
pub fn save_image(app: tauri::AppHandle, data_url: String) -> Result<SaveImageResponse, String> {
    // Parse data URL: data:image/png;base64,xxxxx
    let parts: Vec<&str> = data_url.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err("Invalid data URL format".to_string());
    }

    let header = parts[0];
    let base64_data = parts[1];

    // Extract mime type
    let mime_type = header
        .strip_prefix("data:")
        .and_then(|s| s.strip_suffix(";base64"))
        .unwrap_or("image/png");

    let ext = match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "png",
    };

    // Decode base64
    let image_data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, base64_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    persist_image_bytes(&app, &image_data, ext)
}

/// Copy an image from a local filesystem path to assets directory
#[tauri::command]
pub fn save_image_from_path(app: tauri::AppHandle, file_path: String) -> Result<SaveImageResponse, String> {
    let source_path = PathBuf::from(&file_path);
    let image_data = fs::read(&source_path)
        .map_err(|e| format!("Failed to read image from path {}: {}", file_path, e))?;
    let ext = extension_from_path(&source_path);
    persist_image_bytes(&app, &image_data, ext)
}

/// Create a new empty note
#[tauri::command]
pub fn create_note(app: tauri::AppHandle) -> Result<SaveNoteResponse, String> {
    let now = Utc::now().to_rfc3339();
    let note = Note {
        id: Uuid::new_v4().to_string(),
        title: String::new(),
        content: String::new(),
        tags: String::new(),
        created_at: now.clone(),
        updated_at: now,
        is_pinned: false,
        deleted_at: None,
    };

    let _ = app;
    Ok(SaveNoteResponse { ok: true, note })
}

/// Return the active todo inbox, creating it atomically when it does not exist.
///
/// The transaction takes SQLite's immediate write lock before checking the
/// title, so two manager windows cannot both observe an empty inbox and create
/// duplicate notes. Existing content and metadata are returned unchanged.
#[tauri::command]
pub fn ensure_todo_inbox_note(app: tauri::AppHandle) -> Result<SaveNoteResponse, String> {
    let mut conn = get_connection(&app)?;
    ensure_todo_inbox_with_conn(&mut conn)
}

fn ensure_todo_inbox_with_conn(conn: &mut Connection) -> Result<SaveNoteResponse, String> {
    const TODO_INBOX_TITLE: &str = "待办箱";
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| format!("Failed to begin todo inbox transaction: {}", e))?;

    let existing = tx
        .query_row(
            "SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at
             FROM notes
             WHERE deleted_at IS NULL AND TRIM(title) = ?
             ORDER BY updated_at DESC, id ASC
             LIMIT 1",
            [TODO_INBOX_TITLE],
            note_from_row,
        )
        .optional()
        .map_err(|e| format!("Failed to find todo inbox: {}", e))?;

    if let Some(note) = existing {
        tx.commit()
            .map_err(|e| format!("Failed to commit todo inbox lookup: {}", e))?;
        return Ok(SaveNoteResponse { ok: true, note });
    }

    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO notes (id, title, content, tags, created_at, updated_at, is_pinned)
         VALUES (?, ?, '', '', ?, ?, 0)",
        rusqlite::params![id, TODO_INBOX_TITLE, now, now],
    )
    .map_err(|e| format!("Failed to create todo inbox: {}", e))?;

    let note = get_note_helper(&tx, &id)?;
    tx.commit()
        .map_err(|e| format!("Failed to commit todo inbox creation: {}", e))?;
    Ok(SaveNoteResponse { ok: true, note })
}

/// Toggle pin status of a note
#[tauri::command]
pub fn toggle_pin_note(app: tauri::AppHandle, id: String) -> Result<SaveNoteResponse, String> {
    let conn = get_connection(&app)?;

    // Get current pin status
    let current_pinned: bool = conn
        .query_row("SELECT is_pinned FROM notes WHERE id = ?", [&id], |row| {
            Ok(row.get::<_, i32>(0)? != 0)
        })
        .map_err(|e| format!("Failed to get note: {}", e))?;

    // Toggle
    let new_pinned = !current_pinned;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE notes SET is_pinned = ?, updated_at = ? WHERE id = ?",
        rusqlite::params![new_pinned as i32, now, id],
    )
    .map_err(|e| format!("Failed to update note: {}", e))?;

    let note = get_note_helper(&conn, &id)?;
    Ok(SaveNoteResponse { ok: true, note })
}

// ============================================================================
// Trash Commands
// ============================================================================

#[derive(Debug, Serialize)]
pub struct TrashNoteResponse {
    pub ok: bool,
    pub trashed_id: String,
}

#[derive(Debug, Serialize)]
pub struct RestoreNoteResponse {
    pub ok: bool,
    pub restored_id: String,
}

#[derive(Debug, Serialize)]
pub struct EmptyTrashResponse {
    pub ok: bool,
    pub deleted_count: usize,
}

/// Soft delete a single note (move to trash)
#[tauri::command]
pub fn trash_note(app: tauri::AppHandle, id: String) -> Result<TrashNoteResponse, String> {
    let conn = get_connection(&app)?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE notes SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
        rusqlite::params![now, id],
    )
    .map_err(|e| format!("Failed to trash note: {}", e))?;

    Ok(TrashNoteResponse {
        ok: true,
        trashed_id: id,
    })
}

/// Soft delete multiple notes (move to trash)
#[tauri::command]
pub fn trash_notes_batch(app: tauri::AppHandle, ids: Vec<String>) -> Result<TrashNoteResponse, String> {
    let conn = get_connection(&app)?;
    let now = Utc::now().to_rfc3339();

    for id in &ids {
        conn.execute(
            "UPDATE notes SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
            rusqlite::params![now, id],
        )
        .map_err(|e| format!("Failed to trash note {}: {}", id, e))?;
    }

    Ok(TrashNoteResponse {
        ok: true,
        trashed_id: ids.join(","),
    })
}

/// List all notes in trash
#[tauri::command]
pub fn list_trash(app: tauri::AppHandle) -> Result<ListNotesResponse, String> {
    // First, clean up old trash items based on retention setting
    if let Ok(settings_response) = crate::settings::get_settings(app.clone()) {
        let retention_days = settings_response.settings.trash.retention_days;
        let _ = cleanup_old_trash(&app, retention_days);
    }

    let conn = get_connection(&app)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at
            FROM notes
            WHERE deleted_at IS NOT NULL
            ORDER BY deleted_at DESC
            "#,
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let notes = stmt
        .query_map([], |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tags: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                is_pinned: row.get::<_, i32>(6)? != 0,
                deleted_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("Failed to query trash: {}", e))?
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|e| format!("Failed to collect notes: {}", e))?;

    Ok(ListNotesResponse {
        ok: true,
        count: notes.len(),
        notes,
    })
}

/// Restore a single note from trash
#[tauri::command]
pub fn restore_note(app: tauri::AppHandle, id: String) -> Result<RestoreNoteResponse, String> {
    let conn = get_connection(&app)?;

    conn.execute(
        "UPDATE notes SET deleted_at = NULL WHERE id = ? AND deleted_at IS NOT NULL",
        [&id],
    )
    .map_err(|e| format!("Failed to restore note: {}", e))?;

    Ok(RestoreNoteResponse {
        ok: true,
        restored_id: id,
    })
}

/// Restore multiple notes from trash
#[tauri::command]
pub fn restore_notes_batch(app: tauri::AppHandle, ids: Vec<String>) -> Result<RestoreNoteResponse, String> {
    let conn = get_connection(&app)?;

    for id in &ids {
        conn.execute(
            "UPDATE notes SET deleted_at = NULL WHERE id = ? AND deleted_at IS NOT NULL",
            [id],
        )
        .map_err(|e| format!("Failed to restore note {}: {}", id, e))?;
    }

    Ok(RestoreNoteResponse {
        ok: true,
        restored_id: ids.join(","),
    })
}

/// Permanently delete a single note
#[tauri::command]
pub fn permanent_delete_note(app: tauri::AppHandle, id: String) -> Result<DeleteNoteResponse, String> {
    let conn = get_connection(&app)?;

    conn.execute("DELETE FROM notes WHERE id = ? AND deleted_at IS NOT NULL", [&id])
        .map_err(|e| format!("Failed to permanently delete note: {}", e))?;

    Ok(DeleteNoteResponse {
        ok: true,
        deleted_id: id,
    })
}

/// Empty the trash (permanently delete all trashed notes)
#[tauri::command]
pub fn empty_trash(app: tauri::AppHandle) -> Result<EmptyTrashResponse, String> {
    let conn = get_connection(&app)?;

    let count: usize = conn
        .query_row("SELECT COUNT(*) FROM notes WHERE deleted_at IS NOT NULL", [], |row| {
            row.get::<_, i32>(0).map(|c| c as usize)
        })
        .unwrap_or(0);

    conn.execute("DELETE FROM notes WHERE deleted_at IS NOT NULL", [])
        .map_err(|e| format!("Failed to empty trash: {}", e))?;

    Ok(EmptyTrashResponse {
        ok: true,
        deleted_count: count,
    })
}

/// Clean up old trash items based on retention days setting
/// This should be called periodically (e.g., when listing trash or app startup)
pub fn cleanup_old_trash(app: &tauri::AppHandle, retention_days: u32) -> Result<usize, String> {
    if retention_days == 0 {
        return Ok(0);
    }

    let conn = get_connection(app)?;

    // Calculate the cutoff date
    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    let cutoff_str = cutoff.to_rfc3339();

    // Delete notes that have been in trash longer than retention_days
    let deleted = conn
        .execute(
            "DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?",
            [&cutoff_str],
        )
        .map_err(|e| format!("Failed to cleanup old trash: {}", e))?;

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn memory_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory database");
        init_db(&mut conn).expect("notes schema");
        conn
    }

    fn input(id: &str, content: &str) -> NoteInput {
        NoteInput {
            id: id.to_string(),
            title: None,
            content: Some(content.to_string()),
            tags: None,
            created_at: None,
            updated_at: None,
            is_pinned: None,
            deleted_at: None,
            expected_updated_at: None,
        }
    }

    #[test]
    fn save_creates_then_updates_without_cas() {
        let conn = memory_db();
        let created = save_note_with_conn(&conn, input("n1", "first")).expect("create");
        assert_eq!(created.note.content, "first");

        let updated = save_note_with_conn(&conn, input("n1", "second")).expect("update");
        assert_eq!(updated.note.content, "second");
        assert_ne!(updated.note.updated_at, created.note.updated_at);
    }

    #[test]
    fn clearing_an_existing_note_persists_the_empty_content() {
        // Regression: clearing the editor must reach SQLite. Skipping the
        // write for empty content made the old text reappear on reopen.
        let conn = memory_db();
        let created = save_note_with_conn(&conn, input("n1", "keep me")).expect("create");
        assert_eq!(created.note.content, "keep me");

        let cleared = save_note_with_conn(&conn, input("n1", "")).expect("clear");
        assert!(cleared.note.content.is_empty());

        let stored = get_note_helper(&conn, "n1").expect("read back");
        assert!(stored.content.is_empty(), "stale content resurrected");
    }

    #[test]
    fn a_new_empty_note_is_not_persisted() {
        // Autosave can fire on an untouched new note; it must not leave an
        // empty shell in the list.
        let conn = memory_db();
        let saved = save_note_with_conn(&conn, input("blank", "")).expect("noop save");
        assert!(saved.note.content.is_empty());
        assert!(
            get_note_helper(&conn, "blank").is_err(),
            "empty note must not reach the notes table"
        );
    }

    #[test]
    fn cas_update_succeeds_with_matching_revision() {
        // F05: 读取后没有其他写入时，带 revision 的保存必须成功。
        let conn = memory_db();
        let created = save_note_with_conn(&conn, input("n1", "first")).expect("create");

        let mut cas = input("n1", "second");
        cas.expected_updated_at = Some(created.note.updated_at.clone());
        let updated = save_note_with_conn(&conn, cas).expect("cas update");
        assert_eq!(updated.note.content, "second");
    }

    #[test]
    fn cas_update_rejects_stale_revision() {
        // F05: 读取后另一窗口改写了笔记，旧 revision 的保存必须被拒绝，
        // 而不是静默覆盖那次写入。
        let conn = memory_db();
        let created = save_note_with_conn(&conn, input("n1", "first")).expect("create");

        // 另一窗口写入（不带 CAS，last-write-wins）。
        let _other = save_note_with_conn(&conn, input("n1", "other-window")).expect("other write");

        let mut stale = input("n1", "mine");
        stale.expected_updated_at = Some(created.note.updated_at.clone());
        let err = save_note_with_conn(&conn, stale).expect_err("stale cas must fail");
        assert!(err.starts_with("note_conflict:"), "unexpected error: {err}");

        // 另一窗口的写入必须完好保留。
        let stored = get_note_helper(&conn, "n1").expect("read back");
        assert_eq!(stored.content, "other-window");
    }

    #[test]
    fn cas_is_ignored_when_note_does_not_exist_yet() {
        // 新笔记没有旧 revision：CAS 字段只约束更新路径，创建路径不受
        // 影响，否则“保存到不存在的笔记”这种正常操作会被误伤。
        let conn = memory_db();
        let mut cas = input("ghost", "fresh");
        cas.expected_updated_at = Some("2026-10-19T00:00:00Z".to_string());
        let saved = save_note_with_conn(&conn, cas).expect("create with cas field");
        assert_eq!(saved.note.content, "fresh");
    }

    #[test]
    fn ensure_todo_inbox_reuses_active_note_without_overwriting_content() {
        let mut conn = memory_db();
        let mut inbox = input("inbox-1", "- [ ] keep this");
        inbox.title = Some("  待办箱  ".to_string());
        let created = save_note_with_conn(&conn, inbox).expect("create inbox");

        let ensured = ensure_todo_inbox_with_conn(&mut conn).expect("ensure inbox");
        assert_eq!(ensured.note.id, created.note.id);
        assert_eq!(ensured.note.title, "  待办箱  ");
        assert_eq!(ensured.note.content, "- [ ] keep this");
    }

    #[test]
    fn ensure_todo_inbox_creates_replacement_for_deleted_note() {
        let mut conn = memory_db();
        let mut deleted = input("deleted-inbox", "old");
        deleted.title = Some("待办箱".to_string());
        let created = save_note_with_conn(&conn, deleted).expect("create deleted inbox");
        conn.execute(
            "UPDATE notes SET deleted_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), created.note.id],
        )
        .expect("soft delete inbox");

        let ensured = ensure_todo_inbox_with_conn(&mut conn).expect("ensure replacement");
        assert_ne!(ensured.note.id, created.note.id);
        assert_eq!(ensured.note.title, "待办箱");
        assert!(ensured.note.content.is_empty());
    }

    #[test]
    fn ensure_todo_inbox_is_atomic_across_connections() {
        use std::sync::Arc;
        use std::thread;

        let path = std::env::temp_dir().join(format!("easyflowhub-notes-{}.db", Uuid::new_v4()));
        let mut setup = Connection::open(&path).expect("open setup database");
        init_db(&mut setup).expect("setup schema");
        drop(setup);
        let shared_path = Arc::new(path.clone());

        let workers = (0..2)
            .map(|_| {
                let path = Arc::clone(&shared_path);
                thread::spawn(move || {
                    let mut conn = Connection::open(path.as_ref()).expect("open worker database");
                    conn.busy_timeout(Duration::from_secs(5)).expect("busy timeout");
                    ensure_todo_inbox_with_conn(&mut conn).expect("ensure inbox");
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("worker completed");
        }

        let verify = Connection::open(&path).expect("open verify database");
        let count: i64 = verify
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE deleted_at IS NULL AND TRIM(title) = '待办箱'",
                [],
                |row| row.get(0),
            )
            .expect("count inboxes");
        assert_eq!(count, 1);

        drop(verify);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-journal"));
    }

    /// S3 compatibility: read and write a database the **native shell** produced.
    ///
    /// Ignored by default because it needs an artefact: `scripts/measure-native-store.ps1`
    /// copies the live database, lets the native bench write its probe rows, and only
    /// then runs `cargo test --lib -- --ignored`. Without `EFH_S3_DB` the test fails
    /// rather than passing quietly -- a compatibility test that skips is worse than
    /// no test, because the record then claims something nobody checked.
    #[test]
    #[ignore = "needs a native-written database; run scripts/measure-native-store.ps1"]
    fn reads_and_writes_a_database_produced_by_the_native_shell() {
        let path = std::env::var("EFH_S3_DB")
            .expect("EFH_S3_DB must point at the copy written by the native bench");
        let mut conn = Connection::open(&path).expect("open the native database");

        // 1. The real migrations must accept the file without changing it.
        init_db(&mut conn).expect("migrations run against a native-written database");

        // 1b. And the ladder must then report it finished. The native shell climbed
        //     the same six rungs, so `Rebuild::Run` should have found nothing to do --
        //     in particular no pre-audio CHECK left to rewrite, which is the one
        //     migration that renames tables and could silently eat rows.
        let report = hub_core::store::schema(&conn).expect("measure the native database");
        assert!(
            report.is_current(),
            "native database is not current after migrating: {report:?}"
        );
        assert!(
            !report.legacy_capture_kinds,
            "the Inbox CHECK is still the pre-audio one after `Rebuild::Run`"
        );

        // 2. A row the native shell inserted is readable through the product's own
        //    helper, field for field. `PROBE_BODY` plus the native-side update suffix.
        let probe = get_note_helper(&conn, "s3-probe-created").expect("native probe row");
        assert_eq!(probe.title, "S3 探针");
        assert_eq!(probe.content, "探针正文\t第二行\nemoji 🐟 与 ASCII（已更新）");
        assert_eq!(probe.tags, "s3,probe");
        assert_eq!(probe.updated_at, "2026-11-25T00:00:01+00:00");

        // 3. Soft delete written by the native shell hides the row from the product's
        //    list but keeps it retrievable.
        let listed = list_notes_with_conn(&conn).expect("list on the native database");
        assert!(listed.iter().any(|note| note.id == "s3-probe-created"));
        assert!(!listed.iter().any(|note| note.id == "s3-probe-deleted"));
        let trashed = get_note_helper(&conn, "s3-probe-deleted").expect("soft-deleted row stays");
        assert!(trashed.deleted_at.is_some());

        // 4. The attachment the native shell copied is where the row says it is.
        let (filename, size): (String, i64) = conn
            .query_row(
                "SELECT filename, size_bytes FROM inbox_attachments WHERE item_id = 's3-probe-item'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("native attachment row");
        assert_eq!(size, 1024);
        let assets = std::path::Path::new(&path)
            .parent()
            .expect("database has a parent")
            .join("assets");
        let stored = assets.join(&filename);
        assert!(stored.exists(), "{} missing", stored.display());
        let bytes = std::fs::read(&stored).expect("read attachment");
        assert_eq!(bytes.len(), 1024);
        assert_eq!(bytes[1], 1, "content pattern differs from what the probe wrote");

        // 5. The contract values survived the native write.
        let journal: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");
        assert_eq!(journal, "delete");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("user version");
        assert_eq!(version, 0);
        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity");
        assert_eq!(integrity, "ok");

        // 6. And the other direction: this side writes, the native reader reads it back
        //    in the gate's second pass.
        let mut write = input("s3-probe-created", "Tauri 改写：\n- [ ] 反向验证");
        write.expected_updated_at = Some(probe.updated_at.clone());
        let saved = save_note_with_conn(&conn, write).expect("tauri writes the native row");
        assert_eq!(saved.note.content, "Tauri 改写：\n- [ ] 反向验证");
        assert_ne!(saved.note.updated_at, probe.updated_at);
    }
}
