//! The Inbox `CHECK` rebuild: the one migration that rewrites tables.
//!
//! SQLite cannot alter a `CHECK` constraint in place, so a database created before
//! audio and transcript attachments existed has to be rebuilt: rename both tables
//! aside, create them again, copy the rows, drop the copies. This is the most
//! dangerous DDL in the product, which is why [`super::Rebuild`] exists -- the caller
//! says whether this process is allowed to do it.
//!
//! Three details are load-bearing, each one a failure that was already there:
//!
//! * **The copy names its columns, by intersection.** The 0.2.0 version wrote
//!   `SELECT id, item_id, ..., NULL FROM inbox_attachments_legacy`: positional, with
//!   a literal `NULL` where `source_path` goes. `source_path` was therefore blanked,
//!   and `inbox_items.deleted_at` was not in the target list at all, so trash markers
//!   were dropped too. Reading both column lists first and copying only the names
//!   that exist on both sides makes "a column was added later" irrelevant -- and it
//!   removes the positional order as a thing the rebuild has to preserve.
//! * **`foreign_keys` is switched outside the transaction.** The pragma is a no-op
//!   inside one (SQLite ignores it while a transaction is open), so the old `OFF`
//!   never took effect: the rebuild ran with the cascade armed and depended on the
//!   drop order. Here the previous value is read, cleared before the transaction, and
//!   restored afterwards whatever happened -- which also makes the rollback test
//!   assert something real instead of "it was never off".
//! * **Indexes are created after the legacy tables are dropped.** `ALTER TABLE ...
//!   RENAME` moves an index along with its table, so creating
//!   `idx_inbox_items_captured_at` while `inbox_items_legacy` still holds that name
//!   would silently skip and then disappear with the drop.
//!
//! A failure rolls the whole rebuild back (the transaction is `IMMEDIATE`, so the
//! write lock is taken up front): the legacy tables stay exactly as they were and the
//! next open retries, instead of a file left with half a schema.

use rusqlite::{Connection, TransactionBehavior};

use super::{
    columns, legacy_capture_kinds, table_exists, Result, INBOX_ATTACHMENT_COLUMNS,
    INBOX_INDEXES_DDL, INBOX_ITEM_COLUMNS, INBOX_TABLES_DDL,
};

/// Rebuild both Inbox tables if their `CHECK` constraints predate audio/transcript.
///
/// A file without Inbox tables has nothing to rebuild -- the rungs that follow create
/// them in the current shape -- so this is a no-op on a fresh database and on every
/// open of a file that is already current.
pub(super) fn capture_kinds(conn: &mut Connection) -> Result<()> {
    if !table_exists(conn, "inbox_items")? || !table_exists(conn, "inbox_attachments")? {
        return Ok(());
    }
    if !legacy_capture_kinds(conn)? {
        return Ok(());
    }

    // Read before the rename: after it, these names refer to the legacy copies.
    let items = copy_list(conn, "inbox_items", INBOX_ITEM_COLUMNS)?;
    let attachments = copy_list(conn, "inbox_attachments", INBOX_ATTACHMENT_COLUMNS)?;

    let foreign_keys: i64 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
    if foreign_keys != 0 {
        conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    }

    let outcome = rebuild_in_transaction(conn, &items, &attachments);

    if foreign_keys != 0 {
        // Deliberately not `?`: a failure to restore the pragma must not replace the
        // migration error that explains why nothing was committed.
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON;");
    }
    outcome
}

fn rebuild_in_transaction(
    conn: &mut Connection,
    items: &[String],
    attachments: &[String],
) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(
        // Children first. With `legacy_alter_table` off (the default) the rename
        // rewrites the child's `REFERENCES` clause to name the renamed parent, so the
        // child has to go before the parent it now points at.
        "ALTER TABLE inbox_attachments RENAME TO inbox_attachments_legacy;
         ALTER TABLE inbox_items RENAME TO inbox_items_legacy;",
    )?;
    tx.execute_batch(INBOX_TABLES_DDL)?;
    copy(&tx, "inbox_items", items)?;
    copy(&tx, "inbox_attachments", attachments)?;
    tx.execute_batch(
        "DROP TABLE inbox_attachments_legacy;
         DROP TABLE inbox_items_legacy;",
    )?;
    tx.execute_batch(INBOX_INDEXES_DDL)?;
    tx.commit()
}

/// `INSERT INTO t (c1, c2, …) SELECT c1, c2, … FROM t_legacy`, for the columns both
/// sides have, in the target's order.
fn copy(tx: &rusqlite::Transaction<'_>, table: &str, columns: &[String]) -> Result<()> {
    let list = columns.join(", ");
    tx.execute_batch(&format!(
        "INSERT INTO {table} ({list}) SELECT {list} FROM {table}_legacy"
    ))
}

/// The target columns that also exist on the legacy table, in the target's order.
fn copy_list(conn: &Connection, table: &str, target: &[&str]) -> Result<Vec<String>> {
    let legacy = columns(conn, table)?;
    Ok(target
        .iter()
        .filter(|column| legacy.iter().any(|existing| existing == *column))
        .map(|column| (*column).to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::super::{columns as table_columns, init_schema, Rebuild, SCHEMA_VERSION};
    use super::*;

    /// A 0.2.0 database: both Inbox tables, neither with audio/transcript.
    fn legacy_db() -> Connection {
        let conn = Connection::open_in_memory().expect("memory db");
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE notes (id TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL, is_pinned INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE inbox_items (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file')),
                title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'manager', context TEXT NOT NULL DEFAULT '',
                captured_at TEXT NOT NULL, archived_at TEXT DEFAULT NULL,
                converted_note_id TEXT DEFAULT NULL
            );
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at)
                VALUES ('legacy', 'text', 'Legacy', 'keep me', 'manager', 'manual', '2026-01-01T00:00:00Z');
            "#,
        )
        .expect("legacy schema");
        conn
    }

    fn table_sql(conn: &Connection, table: &str) -> String {
        conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .expect("table sql")
    }

    #[test]
    fn legacy_inbox_tables_gain_audio_and_transcript_kinds() {
        let mut conn = legacy_db();
        let report = init_schema(&mut conn, Rebuild::Run).expect("rebuild");

        assert_eq!(report.version, SCHEMA_VERSION);
        assert!(report.is_current(), "version {} / {:?}", report.version, report.missing);
        assert!(!report.legacy_capture_kinds, "the CHECK is still the old one");

        let content: String = conn
            .query_row("SELECT content FROM inbox_items WHERE id = 'legacy'", [], |row| row.get(0))
            .expect("legacy item remains");
        assert_eq!(content, "keep me");
        let deleted_at: Option<String> = conn
            .query_row("SELECT deleted_at FROM inbox_items WHERE id = 'legacy'", [], |row| row.get(0))
            .expect("trash marker column exists");
        assert!(deleted_at.is_none(), "a legacy row was not trashed");

        // The point of the migration: the new kinds are now accepted.
        conn.execute(
            "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at) \
             VALUES ('t1', 'legacy', 'transcript', '转写文稿.txt', 'text/plain', 't1.txt', 4, '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("transcript kind accepted after rebuild");

        // And a second open is a real no-op: the probe no longer sees a legacy CHECK.
        let again = init_schema(&mut conn, Rebuild::Run).expect("second pass");
        assert!(again.is_current());
    }

    #[test]
    fn a_rebuild_keeps_the_columns_that_were_added_after_the_check() {
        // The exact shape that lost data: `deleted_at` and `source_path` arrived after
        // the audio kinds, and the positional copy blanked both.
        let mut conn = legacy_db();
        conn.execute_batch(
            "ALTER TABLE inbox_items ADD COLUMN deleted_at TEXT DEFAULT NULL;
             ALTER TABLE inbox_attachments ADD COLUMN source_path TEXT;
             UPDATE inbox_items SET deleted_at = '2026-02-02T00:00:00Z' WHERE id = 'legacy';
             INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at, source_path)
                 VALUES ('att', 'legacy', 'image', 'a.png', 'image/png', 'a.png', 8, '2026-01-01T00:00:00Z', 'C:/docs/a.png');",
        )
        .expect("columns from later releases");

        let report = init_schema(&mut conn, Rebuild::Run).expect("rebuild");
        assert!(!report.legacy_capture_kinds);

        let trashed: Option<String> = conn
            .query_row("SELECT deleted_at FROM inbox_items WHERE id = 'legacy'", [], |row| row.get(0))
            .expect("trash marker survived");
        assert_eq!(trashed.as_deref(), Some("2026-02-02T00:00:00Z"));
        let source: Option<String> = conn
            .query_row("SELECT source_path FROM inbox_attachments WHERE id = 'att'", [], |row| row.get(0))
            .expect("source path survived");
        assert_eq!(source.as_deref(), Some("C:/docs/a.png"));
    }

    #[test]
    fn deferring_reports_the_legacy_check_and_does_not_rewrite_the_tables() {
        let mut conn = legacy_db();

        let report = init_schema(&mut conn, Rebuild::Defer).expect("defer");

        assert!(report.legacy_capture_kinds, "the old CHECK must be reported");
        // The additive rungs did run -- that is the difference between deferring the
        // rebuild and skipping the migration -- but the table was not recreated with
        // the new CHECK, and no renamed copy is lying around.
        let sql = table_sql(&conn, "inbox_items");
        assert!(!sql.contains("'audio'"), "Defer rewrote a table a reader must not rewrite");
        let leftovers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE '%_legacy'",
                [],
                |row| row.get(0),
            )
            .expect("count leftover tables");
        assert_eq!(leftovers, 0);
        assert!(table_columns(&conn, "inbox_items")
            .expect("columns")
            .iter()
            .any(|column| column == "deleted_at"));
        assert!(init_schema(&mut conn, Rebuild::Defer).is_ok());
    }

    #[test]
    fn inbox_migration_rolls_back_on_failure_and_retries() {
        // F03: a partial migration must not leave the database in a state where the
        // legacy tables are gone and the new tables are empty.
        let mut conn = legacy_db();
        conn.execute_batch(
            "DROP TABLE inbox_attachments;
             CREATE TABLE inbox_attachments (
                 id TEXT PRIMARY KEY, item_id TEXT NOT NULL, kind TEXT NOT NULL,
                 name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                 size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
             );
             INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at)
                 VALUES ('bad', 'legacy', 'invalid-kind', 'boom.png', 'image/png', 'boom.png', 1, '2026-01-01T00:00:00Z');",
        )
        .expect("seed a row the new CHECK rejects");

        let migration = init_schema(&mut conn, Rebuild::Run);
        assert!(migration.is_err(), "the copy should have failed on 'invalid-kind'");

        // Legacy rows and schema survive the rollback.
        let content: String = conn
            .query_row("SELECT content FROM inbox_items WHERE id = 'legacy'", [], |row| row.get(0))
            .expect("legacy item survives rollback");
        assert_eq!(content, "keep me");
        let bad: i64 = conn
            .query_row("SELECT COUNT(*) FROM inbox_attachments WHERE id = 'bad'", [], |row| row.get(0))
            .expect("legacy attachment survives rollback");
        assert_eq!(bad, 1);
        assert!(
            !table_sql(&conn, "inbox_attachments").contains("'audio'"),
            "a new-schema table was left behind after the rollback"
        );
        // The pragma was actually switched off for the attempt, and restored.
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("query pragma");
        assert_eq!(fk, 1, "foreign_keys must be restored after a failed rebuild");

        // Fix the data, restore the legacy CHECK, retry: the second attempt succeeds.
        conn.execute_batch(
            "DELETE FROM inbox_attachments WHERE id = 'bad';
             DROP TABLE inbox_attachments;
             CREATE TABLE inbox_attachments (
                 id TEXT PRIMARY KEY, item_id TEXT NOT NULL,
                 kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                 name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                 size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
             );",
        )
        .expect("remove the offending row");
        let retried = init_schema(&mut conn, Rebuild::Run).expect("retry after the fix");
        assert!(retried.is_current());
        assert!(!retried.legacy_capture_kinds);
    }
}
