use chrono::Utc;
use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
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
fn init_db(conn: &Connection) -> SqliteResult<()> {
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
fn get_connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;
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
        // Update existing note
        conn.execute(
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
        .map_err(|e| format!("Failed to update note: {}", e))?;

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
    .map_err(|e| format!("Failed to get note: {}", e))
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
fn assets_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
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

    // Generate filename
    let filename = format!("{}.{}", Uuid::new_v4(), ext);

    // Save to assets directory
    let assets_dir = assets_path(&app)?;
    let file_path = assets_dir.join(&filename);

    fs::write(&file_path, &image_data).map_err(|e| format!("Failed to write image: {}", e))?;

    Ok(SaveImageResponse {
        ok: true,
        filename: filename.clone(),
        path: file_path.to_string_lossy().to_string(),  // 返回完整路径
    })
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
