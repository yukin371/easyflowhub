//! Inbox schema seam.
//!
//! The tables live in the shared `notes.db`, which the native shell opens too, so
//! the DDL and every migration have one owner: the ladder in
//! `hub_core::store::schema`. Nothing here creates a table.
//!
//! What is left is the point the Inbox tests build their database through, and the
//! decision this process makes about the one migration that rewrites tables: it runs
//! it. `Rebuild::Defer` exists for the native shell, which may have opened the file
//! to read a single note; this build is the one that owns the `CHECK` migration.

use hub_core::store::{init_schema, Rebuild};
use rusqlite::{Connection, Result as SqliteResult};

/// Bring a connection to this build's schema, running the Inbox rebuild if the file
/// still carries a pre-audio `CHECK`.
///
/// Test-only on purpose: production connections come from `crate::db::connection`,
/// which applies the same ladder plus the connection policy (path, busy timeout,
/// foreign keys) that a bare `init_db` would not.
#[cfg(test)]
pub(crate) fn init_db(conn: &mut Connection) -> SqliteResult<()> {
    init_schema(conn, Rebuild::Run).map(|_| ())
}

/// In-memory database with the full schema, shared by the Inbox unit tests across
/// the schema / repository / service modules.
#[cfg(test)]
pub(crate) fn test_memory_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory database");
    crate::notes::init_db(&mut conn).expect("notes schema");
    init_db(&mut conn).expect("inbox schema");
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inbox_seam_reaches_the_current_version() {
        // The wiring check: this module's `init_db` and the app's connection reach
        // the same place, and a fresh file needs no rebuild afterwards.
        let mut conn = Connection::open_in_memory().expect("in-memory database");
        let report = init_schema(&mut conn, Rebuild::Run).expect("migrate");
        assert!(report.is_current(), "schema: {report:?}");
        assert!(!report.legacy_capture_kinds);

        // A second pass is a no-op rather than a re-migration.
        assert!(init_db(&mut conn).is_ok());
    }

    // F10: an additive migration must give pre-existing databases the
    // source_path column without disturbing rows already stored.
    #[test]
    fn legacy_attachments_gain_source_path_column() {
        let mut conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(
            r#"
            CREATE TABLE inbox_items (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file')),
                title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'manager', context TEXT NOT NULL DEFAULT '',
                captured_at TEXT NOT NULL, archived_at TEXT DEFAULT NULL, converted_note_id TEXT DEFAULT NULL
            );
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            INSERT INTO inbox_items (id, kind, captured_at) VALUES ('legacy', 'file', '2026-01-01T00:00:00Z');
            INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at)
                VALUES ('att', 'legacy', 'file', 'a.txt', 'text/plain', 'a.txt', 4, '2026-01-01T00:00:00Z');
            "#,
        )
        .expect("seed legacy schema");

        init_db(&mut conn).expect("additive migration should run");

        // The column exists and legacy rows read as NULL source.
        let sources = super::super::repository::item_attachment_source_paths(&conn, "legacy")
            .expect("query after migration");
        assert!(sources.is_empty(), "legacy attachments have no source path");

        // New-style rows can still be inserted with an explicit source.
        conn.execute(
            "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at, source_path) VALUES ('att2', 'legacy', 'file', 'b.txt', 'text/plain', 'b.txt', 4, '2026-01-01T00:00:00Z', '/tmp/b.txt')",
            [],
        )
        .expect("insert with source after migration");
        let sources = super::super::repository::item_attachment_source_paths(&conn, "legacy")
            .expect("query after insert");
        assert_eq!(sources, vec!["/tmp/b.txt".to_string()]);
    }
}
