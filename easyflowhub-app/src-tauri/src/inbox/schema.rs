//! Inbox schema ownership: table creation and additive migrations.
//!
//! The Inbox tables live in the shared `notes.db` and extend it additively.
//! All schema changes and migrations stay in this file so no command or
//! service ever re-implements migration logic.

use rusqlite::{Connection, OptionalExtension, Result as SqliteResult};

pub(crate) fn init_db(conn: &mut Connection) -> SqliteResult<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS inbox_items (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file', 'audio')),
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT 'manager',
            context TEXT NOT NULL DEFAULT '',
            captured_at TEXT NOT NULL,
            archived_at TEXT DEFAULT NULL,
            converted_note_id TEXT DEFAULT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_inbox_items_captured_at
            ON inbox_items(captured_at DESC);
        CREATE INDEX IF NOT EXISTS idx_inbox_items_archived_at
            ON inbox_items(archived_at, captured_at DESC);

        CREATE TABLE IF NOT EXISTS inbox_attachments (
            id TEXT PRIMARY KEY,
            item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK (kind IN ('image', 'file', 'audio', 'transcript')),
            name TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            filename TEXT NOT NULL UNIQUE,
            size_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            source_path TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_inbox_attachments_item_id
            ON inbox_attachments(item_id, created_at);
        "#,
    )?;
    migrate_capture_kinds_schema(conn)?;
    migrate_attachment_source_path(conn)?;
    migrate_deleted_at(conn)
}

/// Add the Inbox trash marker to databases created before Inbox items were
/// integrated with the shared trash view.
fn migrate_deleted_at(conn: &Connection) -> SqliteResult<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('inbox_items') WHERE name = 'deleted_at'",
            [],
            |row| row.get::<_, i32>(0).map(|count| count > 0),
        )
        .unwrap_or(false);
    if !has_column {
        conn.execute(
            "ALTER TABLE inbox_items ADD COLUMN deleted_at TEXT DEFAULT NULL",
            [],
        )?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_inbox_items_deleted_at ON inbox_items(deleted_at DESC)",
        [],
    )?;
    Ok(())
}

/// Add the `source_path` column additively for databases created before the
/// canonical-source link existed (F10). This is a pure additive change
/// (nullable column, no backfill needed), so it does not require the table
/// rebuild used for CHECK-constraint changes.
fn migrate_attachment_source_path(conn: &Connection) -> SqliteResult<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('inbox_attachments') WHERE name = 'source_path'",
            [],
            |row| row.get::<_, i32>(0).map(|count| count > 0),
        )
        .unwrap_or(false);
    if has_column {
        return Ok(());
    }
    conn.execute(
        "ALTER TABLE inbox_attachments ADD COLUMN source_path TEXT",
        [],
    )?;
    Ok(())
}

/// Rebuild the additive Inbox tables once for databases created before the
/// audio or transcript kinds existed. SQLite cannot alter a CHECK constraint
/// in place, so we rename the old tables aside, recreate the new schema, copy
/// data over, and drop the renamed copies.
///
/// The whole rebuild runs inside a single transaction so a partial failure
/// (for example a CHECK violation on the data copy, a disk-full rename, or a
/// crash mid-insert) cannot leave the database with both old and new tables
/// half-populated — the rollback restores the original schema and the next
/// call to `init_db` retries the migration against intact legacy data.
fn migrate_capture_kinds_schema(conn: &mut Connection) -> SqliteResult<()> {
    let item_sql: Option<String> = conn
        .query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'inbox_items'", [], |row| row.get(0))
        .optional()?;
    let attachment_sql: Option<String> = conn
        .query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'inbox_attachments'", [], |row| row.get(0))
        .optional()?;
    let needs_items = item_sql.as_deref().is_some_and(|sql| !sql.contains("'audio'"));
    let needs_attachments = attachment_sql
        .as_deref()
        .is_some_and(|sql| !sql.contains("'audio'") || !sql.contains("'transcript'"));
    if !needs_items && !needs_attachments {
        return Ok(());
    }

    // `foreign_keys` is a connection-level pragma and is not rolled back with
    // the transaction, so the closure always restores it after a failure.
    let migration = conn.transaction().and_then(|tx| {
        tx.execute_batch(
            r#"
            PRAGMA foreign_keys = OFF;
            ALTER TABLE inbox_attachments RENAME TO inbox_attachments_legacy;
            ALTER TABLE inbox_items RENAME TO inbox_items_legacy;
            CREATE TABLE inbox_items (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file', 'audio')),
                title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'manager', context TEXT NOT NULL DEFAULT '',
                captured_at TEXT NOT NULL, archived_at TEXT DEFAULT NULL, converted_note_id TEXT DEFAULT NULL
            );
            INSERT INTO inbox_items SELECT id, kind, title, content, source, context, captured_at, archived_at, converted_note_id FROM inbox_items_legacy;
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY, item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK (kind IN ('image', 'file', 'audio', 'transcript')),
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL, source_path TEXT
            );
            INSERT INTO inbox_attachments SELECT id, item_id, kind, name, mime_type, filename, size_bytes, created_at, NULL FROM inbox_attachments_legacy;
            DROP TABLE inbox_attachments_legacy;
            DROP TABLE inbox_items_legacy;
            CREATE INDEX IF NOT EXISTS idx_inbox_items_captured_at ON inbox_items(captured_at DESC);
            CREATE INDEX IF NOT EXISTS idx_inbox_items_archived_at ON inbox_items(archived_at, captured_at DESC);
            CREATE INDEX IF NOT EXISTS idx_inbox_attachments_item_id ON inbox_attachments(item_id, created_at);
            PRAGMA foreign_keys = ON;
            "#,
        )?;
        tx.commit()
    });

    if migration.is_err() {
        // The transaction was rolled back, but the connection-level pragma
        // change was not — flip it back so the rest of the app keeps the
        // foreign-key enforcement it expects.
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON;");
    }
    migration
}

/// In-memory database with the notes schema plus the Inbox schema, shared by
/// the Inbox unit tests across the schema / repository / service modules.
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
    fn inbox_schema_is_additive_and_idempotent_for_existing_notes() {
        let mut conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(
            "CREATE TABLE notes (id TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL, updated_at TEXT NOT NULL, is_pinned INTEGER NOT NULL DEFAULT 0); INSERT INTO notes (id, title, content, created_at, updated_at) VALUES ('old', 'Old note', 'preserved', '2026-01-01', '2026-01-02');",
        )
        .expect("legacy note schema");

        crate::notes::init_db(&mut conn).expect("migrate existing notes");
        init_db(&mut conn).expect("create inbox tables");
        init_db(&mut conn).expect("repeat migration");

        let note: (String, String) = conn
            .query_row("SELECT title, content FROM notes WHERE id = 'old'", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("old note remains");
        assert_eq!(note, ("Old note".to_string(), "preserved".to_string()));
        let inbox_table_count: i32 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'inbox_items'", [], |row| row.get(0))
            .expect("inbox table exists");
        assert_eq!(inbox_table_count, 1);
        let deleted_at_exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('inbox_items') WHERE name = 'deleted_at'",
                [],
                |row| row.get(0),
            )
            .expect("trash marker exists");
        assert_eq!(deleted_at_exists, 1);
    }

    #[test]
    fn legacy_inbox_tables_gain_audio_and_transcript_kinds() {
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
            INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at)
                VALUES ('legacy', 'text', 'Legacy', 'keep me', 'manager', 'manual', '2026-01-01T00:00:00Z');
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            "#,
        )
        .expect("legacy inbox schema");

        init_db(&mut conn).expect("migrate legacy inbox tables");

        let content: String = conn
            .query_row("SELECT content FROM inbox_items WHERE id = 'legacy'", [], |row| row.get(0))
            .expect("legacy item remains");
        assert_eq!(content, "keep me");
        let deleted_at: Option<String> = conn
            .query_row("SELECT deleted_at FROM inbox_items WHERE id = 'legacy'", [], |row| row.get(0))
            .expect("legacy item has no deletion timestamp");
        assert!(deleted_at.is_none());
        conn.execute(
            "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at) VALUES ('t1', 'legacy', 'transcript', '转写文稿.txt', 'text/plain', 't1.txt', 4, '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("transcript kind accepted after migration");
    }

    #[test]
    fn inbox_migration_rolls_back_on_failure_and_retries() {
        // F03: a partial migration must not leave the database in a state
        // where the legacy tables are gone and the new tables are empty.
        // Inject a row that violates the new CHECK constraint, attempt the
        // migration, then verify rollback preserved the legacy tables and
        // a second call (after removing the bad row) succeeds.
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
            INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at)
                VALUES ('kept', 'text', 'Kept', 'survive rollback', 'manager', 'manual', '2026-01-01T00:00:00Z');
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
                -- The legacy schema below cannot store the value that breaks
                -- the migration; we will inject it into the legacy table
                -- through a different channel.
                kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            "#,
        )
        .expect("legacy inbox schema");

        // Temporarily allow an 'invalid' kind by dropping the CHECK so the
        // bad row can land in the legacy table, then re-add a strict CHECK
        // on the new schema via the migration's INSERT step to force a
        // constraint failure during the data copy.
        conn.execute_batch("DROP TABLE inbox_attachments;")
            .expect("drop legacy attachments");
        conn.execute_batch(
            r#"
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at)
                VALUES ('bad', 'kept', 'invalid-kind', 'boom.png', 'image/png', 'boom.png', 1, '2026-01-01T00:00:00Z');
            "#,
        )
        .expect("seed bad row in legacy attachments");

        // Migration should fail (CHECK on inbox_attachments.kind rejects
        // 'invalid-kind'), roll back, and leave the legacy schema intact.
        let migration = init_db(&mut conn);
        assert!(migration.is_err(), "migration should fail on invalid kind");

        // Legacy tables and data must survive the rollback.
        let content: String = conn
            .query_row("SELECT content FROM inbox_items WHERE id = 'kept'", [], |row| row.get(0))
            .expect("legacy item survives rollback");
        assert_eq!(content, "survive rollback");
        let bad_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM inbox_attachments WHERE id = 'bad'", [], |row| row.get(0))
            .expect("legacy attachments survive rollback");
        assert_eq!(bad_count, 1);
        // No new table should have been created either.
        let new_sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'inbox_attachments'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("query legacy attachments sql");
        assert!(
            new_sql.as_deref().is_none_or(|s| !s.contains("'audio'")),
            "migration must not have left a new-schema inbox_attachments around"
        );
        // foreign_keys pragma must be back to ON after the failed migration.
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("query foreign_keys pragma");
        assert_eq!(fk, 1, "foreign_keys must be re-enabled after rollback");

        // Remove the offending row and retry — the second call should succeed
        // and finish the migration that the first call had to abort.
        conn.execute("DELETE FROM inbox_attachments WHERE id = 'bad'", [])
            .expect("remove bad row");
        // Re-attach the legacy CHECK so the migration detects it as legacy.
        conn.execute_batch("DROP TABLE inbox_attachments;")
            .expect("drop unconstrained table");
        conn.execute_batch(
            r#"
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            "#,
        )
        .expect("restore legacy check");

        init_db(&mut conn).expect("retry migration after data fix");

        let content_after: String = conn
            .query_row("SELECT content FROM inbox_items WHERE id = 'kept'", [], |row| row.get(0))
            .expect("legacy item is present after retry");
        assert_eq!(content_after, "survive rollback");
        conn.execute(
            "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at) VALUES ('t1', 'kept', 'transcript', '转写文稿.txt', 'text/plain', 't1.txt', 4, '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("transcript kind accepted after retry");
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
