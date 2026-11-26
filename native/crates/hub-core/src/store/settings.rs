//! The `settings` table: one JSON document per domain key.
//!
//! Two shapes live in the same table and must not be confused:
//!
//! * the eight `AppSettings` domains (`quick_note`, `trash`, `editor`,
//!   `launcher_trigger`, `todo`, `translation`, `transcription`, `developer`), each a
//!   serialised `AppSettings` fragment;
//! * `autostart`, which `settings.rs` writes on its own precisely so that a full
//!   settings write cannot clobber it.
//!
//! So nothing here ever rewrites the whole table. A native "save settings" that
//! replaced every key would silently reset autostart, and the symptom ("it keeps
//! forgetting my preference") would be blamed on the wrong component.

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use super::{Result, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRow {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

impl Store {
    /// Every row, ordered by key so the record is stable across runs.
    pub fn settings(&self) -> Result<Vec<SettingRow>> {
        let mut stmt = self
            .connection()
            .prepare("SELECT key, value, updated_at FROM settings ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok(SettingRow {
                key: row.get(0)?,
                value: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn setting(&self, key: &str) -> Result<Option<SettingRow>> {
        Ok(self
            .connection()
            .query_row(
                "SELECT key, value, updated_at FROM settings WHERE key = ?1",
                [key],
                |row| {
                    Ok(SettingRow {
                        key: row.get(0)?,
                        value: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Upsert one key, leaving every other row alone. Same statement as
    /// `settings.rs::write_setting_json`.
    pub fn write_setting_json<T: Serialize>(
        &mut self,
        key: &str,
        value: &T,
        at: &str,
    ) -> Result<()> {
        let encoded = serde_json::to_string(value)
            .map_err(|error| super::StoreError::Missing(format!("serialise {key}: {error}")))?;
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, encoded, at],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{init_schema, Rebuild, DB_FILE};
    use rusqlite::Connection;
    use std::path::PathBuf;

    fn store() -> Store {
        let mut conn = Connection::open_in_memory().expect("memory db");
        init_schema(&mut conn, Rebuild::Defer).expect("schema");
        Store {
            conn,
            path: PathBuf::from(DB_FILE),
        }
    }

    #[test]
    fn writing_one_key_leaves_the_others_untouched() {
        // Regression guard for the autostart clobber described in the module docs.
        let mut store = store();
        store
            .write_setting_json("autostart", &true, "2026-11-25T00:00:00+00:00")
            .expect("autostart");
        store
            .write_setting_json("editor", &serde_json::json!({"fontSize": 14}), "2026-11-25T00:00:00+00:00")
            .expect("editor");

        store
            .write_setting_json("editor", &serde_json::json!({"fontSize": 16}), "2026-11-25T01:00:00+00:00")
            .expect("editor again");

        let autostart = store.setting("autostart").expect("read").expect("present");
        assert_eq!(autostart.value, "true");
        let editor = store.setting("editor").expect("read").expect("present");
        assert_eq!(editor.value, r#"{"fontSize":16}"#);
        assert_eq!(editor.updated_at, "2026-11-25T01:00:00+00:00");
    }

    #[test]
    fn rows_come_back_sorted_by_key() {
        let mut store = store();
        for key in ["todo", "autostart", "editor"] {
            store
                .write_setting_json(key, &1, "2026-11-25T00:00:00+00:00")
                .expect("write");
        }
        let keys: Vec<String> = store
            .settings()
            .expect("list")
            .into_iter()
            .map(|row| row.key)
            .collect();
        assert_eq!(keys, ["autostart", "editor", "todo"]);
    }
}
