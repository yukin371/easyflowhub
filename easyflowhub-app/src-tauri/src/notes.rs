use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Result as SqliteResult, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
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

/// Get the database file path
fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    fs::create_dir_all(&app_data).map_err(|e| format!("Failed to create app data dir: {}", e))?;
    Ok(app_data.join("notes.db"))
}

/// Initialize the database schema
pub(crate) fn init_db(conn: &Connection) -> SqliteResult<()> {
    // 首先创建表（如果不存在）
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            is_pinned INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(is_pinned DESC);
        CREATE INDEX IF NOT EXISTS idx_notes_title ON notes(title);
        "#,
    )?;

    // 迁移：为已存在的表添加 tags 列
    // 检查 tags 列是否存在
    let has_tags_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'tags'",
            [],
            |row| row.get::<_, i32>(0).map(|count| count > 0),
        )
        .unwrap_or(false);

    if !has_tags_column {
        // 添加 tags 列
        conn.execute(
            "ALTER TABLE notes ADD COLUMN tags TEXT NOT NULL DEFAULT ''",
            [],
        )?;
        println!("Database migration: added tags column");
    }

    // 在确认 tags 列存在后，创建索引
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_notes_tags ON notes(tags)",
        [],
    )?;

    // 迁移：为已存在的表添加 deleted_at 列（软删除）
    let has_deleted_at_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'deleted_at'",
            [],
            |row| row.get::<_, i32>(0).map(|count| count > 0),
        )
        .unwrap_or(false);

    if !has_deleted_at_column {
        conn.execute(
            "ALTER TABLE notes ADD COLUMN deleted_at TEXT DEFAULT NULL",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_notes_deleted_at ON notes(deleted_at)",
            [],
        )?;
        println!("Database migration: added deleted_at column");
    }

    Ok(())
}

/// Get or create a database connection
pub(crate) fn get_connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| format!("Failed to configure database lock timeout: {}", e))?;
    init_db(&conn).map_err(|e| format!("Failed to initialize database: {}", e))?;
    Ok(conn)
}

// ============================================================================
// Commands
// ============================================================================

/// List all notes (excluding deleted), sorted by pinned first, then by updated_at descending
#[tauri::command]
pub fn list_notes(app: tauri::AppHandle) -> Result<ListNotesResponse, String> {
    let conn = get_connection(&app)?;

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
        .map_err(|e| format!("Failed to query notes: {}", e))?
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|e| format!("Failed to collect notes: {}", e))?;

    Ok(ListNotesResponse {
        ok: true,
        count: notes.len(),
        notes,
    })
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

    if merged_note.content.trim().is_empty() && merged_note.title.trim().is_empty() {
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

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        init_db(&conn).expect("notes schema");
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
        let setup = Connection::open(&path).expect("open setup database");
        init_db(&setup).expect("setup schema");
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
}

