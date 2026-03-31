use chrono::Utc;
use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickNoteSettings {
    pub width: u32,
    pub height: u32,
}

impl Default for QuickNoteSettings {
    fn default() -> Self {
        Self {
            width: 400,
            height: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashSettings {
    pub retention_days: u32,
}

impl Default for TrashSettings {
    fn default() -> Self {
        Self { retention_days: 30 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSettings {
    pub undo_steps: u32,
    pub cursor_style: String,
    pub cursor_color: String,
    pub cursor_trail: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            undo_steps: 100,
            cursor_style: "accent".to_string(),
            cursor_color: "#4f5a43".to_string(),
            cursor_trail: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoSettings {
    pub done_retention_hours: u32,
}

impl Default for TodoSettings {
    fn default() -> Self {
        Self {
            done_retention_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    #[serde(default)]
    pub quick_note: QuickNoteSettings,
    #[serde(default)]
    pub trash: TrashSettings,
    #[serde(default)]
    pub editor: EditorSettings,
    #[serde(default)]
    pub todo: TodoSettings,
}

#[derive(Debug, Serialize)]
pub struct GetSettingsResponse {
    pub ok: bool,
    pub settings: AppSettings,
}

#[derive(Debug, Serialize)]
pub struct UpdateSettingsResponse {
    pub ok: bool,
    pub settings: AppSettings,
}

// ============================================================================
// Database Management
// ============================================================================

/// Get the database file path (shared with notes module)
fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    fs::create_dir_all(&app_data).map_err(|e| format!("Failed to create app data dir: {}", e))?;
    Ok(app_data.join("notes.db"))
}

/// Initialize the settings table
fn init_settings_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
        [],
    )?;
    Ok(())
}

/// Get or create a database connection
fn get_connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;
    init_settings_table(&conn).map_err(|e| format!("Failed to initialize settings table: {}", e))?;
    Ok(conn)
}

// ============================================================================
// Commands
// ============================================================================

/// Get application settings
#[tauri::command]
pub fn get_settings(app: tauri::AppHandle) -> Result<GetSettingsResponse, String> {
    let conn = get_connection(&app)?;

    let mut settings = AppSettings::default();

    // Load quick_note settings
    if let Ok(value) = conn.query_row(
        "SELECT value FROM settings WHERE key = 'quick_note'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(parsed) = serde_json::from_str::<QuickNoteSettings>(&value) {
            settings.quick_note = parsed;
        }
    }

    // Load trash settings
    if let Ok(value) = conn.query_row(
        "SELECT value FROM settings WHERE key = 'trash'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(parsed) = serde_json::from_str::<TrashSettings>(&value) {
            settings.trash = parsed;
        }
    }

    // Load editor settings
    if let Ok(value) = conn.query_row(
        "SELECT value FROM settings WHERE key = 'editor'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(parsed) = serde_json::from_str::<EditorSettings>(&value) {
            settings.editor = parsed;
        }
    }

    // Load todo settings
    if let Ok(value) = conn.query_row(
        "SELECT value FROM settings WHERE key = 'todo'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(parsed) = serde_json::from_str::<TodoSettings>(&value) {
            settings.todo = parsed;
        }
    }

    Ok(GetSettingsResponse {
        ok: true,
        settings,
    })
}

/// Update application settings
#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<UpdateSettingsResponse, String> {
    let conn = get_connection(&app)?;
    let now = Utc::now().to_rfc3339();

    // Upsert quick_note settings
    let quick_note_json = serde_json::to_string(&settings.quick_note)
        .map_err(|e| format!("Failed to serialize quick_note settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('quick_note', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![quick_note_json, now],
    )
    .map_err(|e| format!("Failed to save quick_note settings: {}", e))?;

    // Upsert trash settings
    let trash_json = serde_json::to_string(&settings.trash)
        .map_err(|e| format!("Failed to serialize trash settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('trash', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![trash_json, now],
    )
    .map_err(|e| format!("Failed to save trash settings: {}", e))?;

    // Upsert editor settings
    let editor_json = serde_json::to_string(&settings.editor)
        .map_err(|e| format!("Failed to serialize editor settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('editor', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![editor_json, now],
    )
    .map_err(|e| format!("Failed to save editor settings: {}", e))?;

    // Upsert todo settings
    let todo_json = serde_json::to_string(&settings.todo)
        .map_err(|e| format!("Failed to serialize todo settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('todo', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![todo_json, now],
    )
    .map_err(|e| format!("Failed to save todo settings: {}", e))?;

    Ok(UpdateSettingsResponse {
        ok: true,
        settings,
    })
}
