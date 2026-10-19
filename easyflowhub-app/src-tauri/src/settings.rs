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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TranslationSettings {
    pub target_language: String,
    pub model: String,
}

impl Default for TranslationSettings {
    fn default() -> Self {
        Self {
            target_language: "zh-CN".to_string(),
            model: String::new(),
        }
    }
}

/// Transcription preferences for post-recording speech-to-text.
///
/// Only preferences are persisted; provider credentials and routing remain
/// owned by Relay (see `docs/decisions/ADR-0008-voice-transcription-and-relay.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TranscriptionSettings {
    pub model: String,
    /// Optional language hint sent to the provider; empty means auto-detect.
    pub language: String,
}

impl Default for TranscriptionSettings {
    fn default() -> Self {
        Self {
            model: String::new(),
            language: String::new(),
        }
    }
}

/// Developer / diagnostics preferences.
///
/// `debug_mode` only controls verbosity and whether native logs are appended to
/// `logs/app.log`; it never enables remote access or bypasses safety checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DeveloperSettings {
    pub debug_mode: bool,
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self { debug_mode: false }
    }
}

/// Launcher trigger configuration.
///
/// `keyboard_shortcut` uses the `global-hotkey` syntax accepted by
/// `tauri-plugin-global-shortcut`, for example `Ctrl+Alt+Space`.
/// `mouse_middle` and `keyboard_or_mouse_middle` use the Windows low-level
/// mouse hook in addition to the keyboard registration when applicable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherTriggerSettings {
    #[serde(default = "default_launcher_enabled")]
    pub enabled: bool,
    #[serde(default = "default_launcher_trigger")]
    pub trigger: String,
    #[serde(default = "default_launcher_shortcut")]
    pub keyboard_shortcut: String,
    #[serde(default)]
    pub excluded_apps: Vec<String>,
}

fn default_launcher_enabled() -> bool {
    true
}

fn default_launcher_trigger() -> String {
    "keyboard_or_mouse_middle".to_string()
}

fn default_launcher_shortcut() -> String {
    "Ctrl+Alt+Space".to_string()
}

impl Default for LauncherTriggerSettings {
    fn default() -> Self {
        Self {
            enabled: default_launcher_enabled(),
            trigger: default_launcher_trigger(),
            keyboard_shortcut: default_launcher_shortcut(),
            excluded_apps: Vec::new(),
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
    #[serde(default)]
    pub launcher_trigger: LauncherTriggerSettings,
    #[serde(default)]
    pub translation: TranslationSettings,
    #[serde(default)]
    pub transcription: TranscriptionSettings,
    #[serde(default)]
    pub developer: DeveloperSettings,
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

/// Return the executable name owning the current foreground window.
///
/// This is intentionally kept in the native layer so shortcut handlers can
/// make the exclusion decision before showing the launcher. The process name
/// (for example `code.exe`) is stable enough for user-maintained exclusion
/// lists, while avoiding a dependency on window titles or localized text.
#[cfg(target_os = "windows")]
pub fn foreground_process_name() -> Option<String> {
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };
    use windows::{core::PWSTR, Win32::Foundation::CloseHandle};

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }

    let mut process_id = 0u32;
    if unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) } == 0 || process_id == 0 {
        return None;
    }

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
    let mut buffer = [0u16; 32_768];
    let mut length = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let _ = unsafe { CloseHandle(process) };

    if result.is_err() || length == 0 {
        return None;
    }

    String::from_utf16(&buffer[..length as usize])
        .ok()
        .and_then(|path| path.rsplit(['\\', '/']).next().map(str::to_string))
}

#[cfg(not(target_os = "windows"))]
pub fn foreground_process_name() -> Option<String> {
    None
}

/// Check whether the current foreground process is in an exclusion list.
/// Entries may be written with or without the `.exe` suffix and are matched
/// case-insensitively after trimming whitespace.
pub fn is_foreground_app_excluded(excluded_apps: &[String]) -> bool {
    let Some(process_name) = foreground_process_name() else {
        return false;
    };
    let process_name = process_name.to_ascii_lowercase();
    let process_stem = process_name.strip_suffix(".exe").unwrap_or(&process_name);

    excluded_apps.iter().any(|entry| {
        let entry = entry.trim().to_ascii_lowercase();
        if entry.is_empty() {
            return false;
        }
        let entry = entry.strip_suffix(".exe").unwrap_or(&entry);
        entry == process_name || entry == process_stem
    })
}

/// Native helper used by settings UI when the user wants to add the current
/// foreground application to the exclusion list.
#[tauri::command]
pub fn get_foreground_process_name() -> Option<String> {
    foreground_process_name()
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

    // Load launcher trigger settings
    if let Ok(value) = conn.query_row(
        "SELECT value FROM settings WHERE key = 'launcher_trigger'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(parsed) = serde_json::from_str::<LauncherTriggerSettings>(&value) {
            settings.launcher_trigger = parsed;
        }
    }

    if let Ok(value) = conn.query_row(
        "SELECT value FROM settings WHERE key = 'translation'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(parsed) = serde_json::from_str::<TranslationSettings>(&value) {
            settings.translation = parsed;
        }
    }

    if let Ok(value) = conn.query_row(
        "SELECT value FROM settings WHERE key = 'transcription'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(parsed) = serde_json::from_str::<TranscriptionSettings>(&value) {
            settings.transcription = parsed;
        }
    }

    if let Ok(value) = conn.query_row(
        "SELECT value FROM settings WHERE key = 'developer'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(parsed) = serde_json::from_str::<DeveloperSettings>(&value) {
            settings.developer = parsed;
        }
    }

    Ok(GetSettingsResponse {
        ok: true,
        settings,
    })
}

/// Persist only the native Launcher trigger settings. This is used by the
/// Launcher settings bridge so changing the trigger does not require sending
/// unrelated editor and note settings back through the command boundary.
pub fn save_launcher_trigger_settings(
    app: &tauri::AppHandle,
    launcher_trigger: &LauncherTriggerSettings,
) -> Result<(), String> {
    let conn = get_connection(app)?;
    let now = Utc::now().to_rfc3339();
    let value = serde_json::to_string(launcher_trigger)
        .map_err(|e| format!("Failed to serialize launcher trigger settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('launcher_trigger', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![value, now],
    )
    .map_err(|e| format!("Failed to save launcher trigger settings: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, TranscriptionSettings, TranslationSettings};

    #[test]
    fn legacy_settings_default_translation_preferences() {
        let settings: AppSettings = serde_json::from_str("{}").expect("legacy settings should load");

        assert_eq!(settings.translation.target_language, "zh-CN");
        assert!(settings.translation.model.is_empty());
    }

    #[test]
    fn legacy_settings_default_transcription_preferences() {
        let settings: AppSettings = serde_json::from_str("{}").expect("legacy settings should load");

        assert!(settings.transcription.model.is_empty());
        assert!(settings.transcription.language.is_empty());
    }

    #[test]
    fn transcription_settings_accept_partial_saved_values() {
        let settings: TranscriptionSettings =
            serde_json::from_str(r#"{"model":"whisper-1"}"#).expect("partial settings should load");

        assert_eq!(settings.model, "whisper-1");
        assert!(settings.language.is_empty());
    }

    #[test]
    fn transcription_settings_round_trip_without_provider_secrets() {
        let mut settings = AppSettings::default();
        settings.transcription.model = "my-relay-transcribe-model".to_string();
        settings.transcription.language = "zh".to_string();

        let saved = serde_json::to_string(&settings).expect("settings should serialize");
        let restored: AppSettings = serde_json::from_str(&saved).expect("settings should reload");

        assert_eq!(restored.transcription.model, "my-relay-transcribe-model");
        assert_eq!(restored.transcription.language, "zh");
        assert!(!saved.contains("api_key"));
    }

    #[test]
    fn translation_settings_accept_partial_saved_values() {
        let settings: TranslationSettings =
            serde_json::from_str(r#"{"model":"gpt-4o-mini"}"#).expect("partial settings should load");

        assert_eq!(settings.target_language, "zh-CN");
        assert_eq!(settings.model, "gpt-4o-mini");
    }

    #[test]
    fn translation_settings_round_trip_without_provider_secrets() {
        let mut settings = AppSettings::default();
        settings.translation.target_language = "en".to_string();
        settings.translation.model = "my-relay-model".to_string();

        let saved = serde_json::to_string(&settings).expect("settings should serialize");
        let restored: AppSettings = serde_json::from_str(&saved).expect("settings should reload");

        assert_eq!(restored.translation.target_language, "en");
        assert_eq!(restored.translation.model, "my-relay-model");
        assert!(!saved.contains("api_key"));
    }
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

    // Upsert launcher trigger settings
    let launcher_trigger_json = serde_json::to_string(&settings.launcher_trigger)
        .map_err(|e| format!("Failed to serialize launcher trigger settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('launcher_trigger', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![launcher_trigger_json, now],
    )
    .map_err(|e| format!("Failed to save launcher trigger settings: {}", e))?;

    // Upsert translation preferences. Provider credentials remain owned by Relay.
    let translation_json = serde_json::to_string(&settings.translation)
        .map_err(|e| format!("Failed to serialize translation settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('translation', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![translation_json, now],
    )
    .map_err(|e| format!("Failed to save translation settings: {}", e))?;

    // Upsert transcription preferences. Provider credentials remain owned by Relay.
    let transcription_json = serde_json::to_string(&settings.transcription)
        .map_err(|e| format!("Failed to serialize transcription settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('transcription', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![transcription_json, now],
    )
    .map_err(|e| format!("Failed to save transcription settings: {}", e))?;

    let developer_json = serde_json::to_string(&settings.developer)
        .map_err(|e| format!("Failed to serialize developer settings: {}", e))?;
    conn.execute(
        r#"
        INSERT INTO settings (key, value, updated_at) VALUES ('developer', ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        rusqlite::params![developer_json, now],
    )
    .map_err(|e| format!("Failed to save developer settings: {}", e))?;

    Ok(UpdateSettingsResponse {
        ok: true,
        settings,
    })
}
