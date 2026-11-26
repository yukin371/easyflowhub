//! The schema as a ladder of rungs, with the version *derived* from the file.
//!
//! The obvious thing to do here is `PRAGMA user_version = 6` and be done. Three
//! measurements say no, and each one is a failure mode that shows up weeks later:
//!
//! * **`user_version` is somebody else's field.** The live database is at
//!   `user_version = 0` because the Tauri build never wrote it, and the S3 gate
//!   asserts it is still 0 after a native write. Stamping it here would be a silent
//!   contract change: the next person would write a `user_version`-driven migration
//!   for a database whose actual migrator is a column probe.
//! * **A stamped number is a cache with two writers.** If Tauri adds a rung and the
//!   native store keeps the old stamp, the number is worse than nothing -- it says
//!   "you are current" on a file that is not. A fact that only one of the two
//!   processes maintains cannot be trusted by either.
//! * **A number without a ladder is decoration.** What actually matters is "which
//!   rungs are on this file", and that question has a direct, checkable answer:
//!   probe for the rung's own artifact. [`Schema`] is exactly that answer.
//!
//! So [`SCHEMA_VERSION`] is the rung this build produces, and `version` in a
//! [`Schema`] report is *measured*: the highest rung whose probe passes, counting
//! from the bottom, so a file with the notes migrations but no Inbox reports 3
//! rather than 6. [`Schema::is_current`] is then a real invariant -- after
//! [`init_schema`] it must hold, and [`super::Store::open`] refuses to hand out a
//! connection when it does not.
//!
//! Two consequences worth stating:
//!
//! * **Rungs are idempotent and self-healing.** A rung's probe is not just "does
//!   the column exist" but "does the column and its index exist", so a database
//!   whose index was dropped by a bad tool is repaired by opening it. The old code
//!   only created the index in the branch that added the column, which meant a
//!   dropped index was invisible forever.
//! * **A file ahead of us is reported, not "migrated".** [`Schema::ahead`] lists
//!   columns in the tables we own that this build does not know about -- the shape
//!   of a newer Tauri release. Writing is still safe (every statement names its
//!   columns), so this is a report, not a refusal; the point is that a downgrade is
//!   visible instead of being guessed at.
//!
//! The one Tauri migration this module does **not** run is the `CHECK`-constraint
//! rebuild of the Inbox tables (`inbox/schema.rs::migrate_capture_kinds_schema`).
//! SQLite cannot alter a `CHECK` in place, so that migration renames both tables,
//! recreates them and copies the data -- the most dangerous DDL in the product. It
//! is detected ([`Schema::legacy_capture_kinds`]) rather than attempted: a native
//! shell that rewrote two tables on open would be taking a risk the Tauri build
//! already takes on its own next launch, and taking it in the process that only
//! wanted to read a note is the wrong trade.

use rusqlite::{Connection, OptionalExtension};

use super::Result;

/// The highest rung this build knows how to build.
///
/// Adding a rung means: append it to [`RUNGS`], give it a `version` of `+1`, and
/// teach [`accepts`] about any column it adds. The drift test in `store/mod.rs`
/// fails when `easyflowhub-app/src-tauri` grows a migration this list does not have.
pub const SCHEMA_VERSION: u32 = 6;

/// What a file looks like right now, measured rather than remembered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    /// Highest rung whose probe passes, counting from the bottom. `0` means the
    /// database has none of our tables.
    pub version: u32,
    /// Rungs that are not on the file, in ladder order. Empty is the healthy state.
    pub missing: Vec<&'static str>,
    /// `table.column` for every column in a table we own that this build does not
    /// know about: the fingerprint of a *newer* build having been here.
    pub ahead: Vec<String>,
    /// The Inbox tables still carry a `CHECK` that predates the audio/transcript
    /// kinds. Reported, never rebuilt here; see the module docs.
    pub legacy_capture_kinds: bool,
}

impl Schema {
    pub fn is_current(&self) -> bool {
        self.version == SCHEMA_VERSION && self.missing.is_empty()
    }
}

/// Columns of each table this module owns, in the order the *current* DDL declares
/// them (which is what a fresh file physically gets).
///
/// A 0.1.x file has `tags` and `deleted_at` appended at the end instead, because
/// `ALTER TABLE` appends; that is fine and not worth equalising -- every statement
/// in this crate names its columns, so physical order is not part of the contract.
/// The drift test in `store/mod.rs` pins these lists against the Tauri DDL.
pub const NOTES_COLUMNS: &[&str] = &[
    "id",
    "title",
    "content",
    "tags",
    "created_at",
    "updated_at",
    "is_pinned",
    "deleted_at",
];

pub const INBOX_ITEM_COLUMNS: &[&str] = &[
    "id",
    "kind",
    "title",
    "content",
    "source",
    "context",
    "captured_at",
    "archived_at",
    "converted_note_id",
    "deleted_at",
];

pub const INBOX_ATTACHMENT_COLUMNS: &[&str] = &[
    "id",
    "item_id",
    "kind",
    "name",
    "mime_type",
    "filename",
    "size_bytes",
    "created_at",
    "source_path",
];

/// Take the schema to [`SCHEMA_VERSION`] by running whichever rungs are missing.
///
/// Idempotent by construction: on a current file every probe passes and nothing is
/// written, so opening a database dozens of times per session (one connection per
/// command is the product's own pattern) costs a handful of `pragma_table_info`
/// calls and no page writes.
pub fn init_schema(conn: &Connection) -> Result<Schema> {
    for rung in RUNGS {
        if !(rung.probe)(conn)? {
            (rung.apply)(conn)?;
        }
    }
    // `settings` is not a rung: it has no versioned columns, and a file without it
    // is not "behind" -- it simply has no preferences yet. Creating it here keeps
    // the statement in one place, exactly as `settings.rs` does.
    conn.execute_batch(SETTINGS_DDL)?;
    schema(conn)
}

/// Measure the file. Cheap enough to call on every open, and the only honest source
/// for `version`.
pub fn schema(conn: &Connection) -> Result<Schema> {
    let mut version = 0;
    let mut missing = Vec::new();
    for rung in RUNGS {
        if (rung.probe)(conn)? {
            // Contiguity is the point: rung 5 on a file without rung 4 says nothing
            // about how far the ladder has actually been climbed.
            if missing.is_empty() {
                version = rung.version;
            }
        } else {
            missing.push(rung.name);
        }
    }
    Ok(Schema {
        version,
        missing,
        ahead: ahead_columns(conn)?,
        legacy_capture_kinds: legacy_capture_kinds(conn)?,
    })
}

/// One step of the ladder: what it needs to be visible, and how to build it.
struct Rung {
    version: u32,
    name: &'static str,
    probe: fn(&Connection) -> Result<bool>,
    apply: fn(&Connection) -> Result<()>,
}

const RUNGS: &[Rung] = &[
    Rung {
        version: 1,
        name: "notes",
        probe: notes_table,
        apply: notes_tables,
    },
    Rung {
        version: 2,
        name: "notes.tags",
        probe: notes_tags,
        apply: add_notes_tags,
    },
    Rung {
        version: 3,
        name: "notes.deleted_at",
        probe: notes_deleted_at,
        apply: add_notes_deleted_at,
    },
    Rung {
        version: 4,
        name: "inbox",
        probe: inbox_tables_exist,
        apply: inbox_tables,
    },
    Rung {
        version: 5,
        name: "inbox_items.deleted_at",
        probe: inbox_deleted_at,
        apply: add_inbox_deleted_at,
    },
    Rung {
        version: 6,
        name: "inbox_attachments.source_path",
        probe: inbox_source_path,
        apply: add_inbox_source_path,
    },
];

/// The exact DDL the Tauri build runs, plus its column-probe migrations.
///
/// Kept as one string so a reader can diff it against `notes.rs` and see it is the
/// same statement; the drift test in `store/mod.rs` parses that file so a column
/// added there but not here fails the build instead of silently producing a
/// narrower schema.
pub const NOTES_DDL: &str = r#"
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
"#;

pub const SETTINGS_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
"#;

/// Inbox tables, as `inbox/schema.rs` creates them -- `source_path` included, in the
/// same position, so a fresh file has the column order the Tauri rebuild's
/// positional `INSERT ... SELECT` expects.
const INBOX_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS inbox_items (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file', 'audio')),
    title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT 'manager', context TEXT NOT NULL DEFAULT '',
    captured_at TEXT NOT NULL, archived_at TEXT DEFAULT NULL,
    converted_note_id TEXT DEFAULT NULL
);

CREATE TABLE IF NOT EXISTS inbox_attachments (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('image', 'file', 'audio', 'transcript')),
    name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL,
    source_path TEXT
);

CREATE INDEX IF NOT EXISTS idx_inbox_items_captured_at ON inbox_items(captured_at DESC);
CREATE INDEX IF NOT EXISTS idx_inbox_items_archived_at ON inbox_items(archived_at, captured_at DESC);
CREATE INDEX IF NOT EXISTS idx_inbox_attachments_item_id ON inbox_attachments(item_id, created_at);
"#;

fn notes_table(conn: &Connection) -> Result<bool> {
    table_exists(conn, "notes")
}

fn notes_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(NOTES_DDL)?;
    Ok(())
}

fn notes_tags(conn: &Connection) -> Result<bool> {
    Ok(column_exists(conn, "notes", "tags")? && index_exists(conn, "idx_notes_tags")?)
}

fn add_notes_tags(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "notes", "tags")? {
        conn.execute(
            "ALTER TABLE notes ADD COLUMN tags TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_notes_tags ON notes(tags)",
        [],
    )?;
    Ok(())
}

fn notes_deleted_at(conn: &Connection) -> Result<bool> {
    Ok(column_exists(conn, "notes", "deleted_at")?
        && index_exists(conn, "idx_notes_deleted_at")?)
}

fn add_notes_deleted_at(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "notes", "deleted_at")? {
        conn.execute("ALTER TABLE notes ADD COLUMN deleted_at TEXT DEFAULT NULL", [])?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_notes_deleted_at ON notes(deleted_at)",
        [],
    )?;
    Ok(())
}

fn inbox_tables_exist(conn: &Connection) -> Result<bool> {
    Ok(table_exists(conn, "inbox_items")? && table_exists(conn, "inbox_attachments")?)
}

fn inbox_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(INBOX_DDL)?;
    Ok(())
}

fn inbox_deleted_at(conn: &Connection) -> Result<bool> {
    Ok(column_exists(conn, "inbox_items", "deleted_at")?
        && index_exists(conn, "idx_inbox_items_deleted_at")?)
}

fn add_inbox_deleted_at(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "inbox_items", "deleted_at")? {
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

fn inbox_source_path(conn: &Connection) -> Result<bool> {
    column_exists(conn, "inbox_attachments", "source_path")
}

fn add_inbox_source_path(conn: &Connection) -> Result<()> {
    conn.execute(
        "ALTER TABLE inbox_attachments ADD COLUMN source_path TEXT",
        [],
    )?;
    Ok(())
}

fn ahead_columns(conn: &Connection) -> Result<Vec<String>> {
    let mut ahead = Vec::new();
    for (table, known) in [
        ("notes", NOTES_COLUMNS),
        ("inbox_items", INBOX_ITEM_COLUMNS),
        ("inbox_attachments", INBOX_ATTACHMENT_COLUMNS),
    ] {
        if !table_exists(conn, table)? {
            continue;
        }
        for column in columns(conn, table)? {
            if !known.contains(&column.as_str()) {
                ahead.push(format!("{table}.{column}"));
            }
        }
    }
    Ok(ahead)
}

fn legacy_capture_kinds(conn: &Connection) -> Result<bool> {
    for (table, literal) in [
        ("inbox_items", "'audio'"),
        ("inbox_attachments", "'transcript'"),
    ] {
        let sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(text) = sql {
            if !text.contains(literal) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn index_exists(conn: &Connection, index: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
        [index],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub(super) fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let sql = format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1");
    let count: i64 = conn.query_row(&sql, [column], |row| row.get(0))?;
    Ok(count > 0)
}

/// Every column of `table` in physical order. Order matters: `ALTER TABLE` appends,
/// so "same columns" between a migrated file and a fresh one is only meaningful
/// positionally, and that is how the migration tests compare them.
pub fn columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let sql = format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Connection {
        let conn = Connection::open_in_memory().expect("memory db");
        init_schema(&conn).expect("schema");
        conn
    }

    #[test]
    fn a_fresh_file_is_at_the_current_version() {
        let conn = Connection::open_in_memory().expect("memory db");
        // Before init there is nothing, and "nothing" is version 0 -- not an error.
        let empty = schema(&conn).expect("measure");
        assert_eq!(empty.version, 0);
        assert!(!empty.is_current());

        let current = init_schema(&conn).expect("schema");
        assert_eq!(current.version, SCHEMA_VERSION);
        assert!(current.missing.is_empty(), "missing: {:?}", current.missing);
        assert!(current.ahead.is_empty(), "ahead: {:?}", current.ahead);
        assert!(current.is_current());
    }

    #[test]
    fn a_legacy_file_ends_with_the_same_columns_as_a_fresh_one() {
        // The 0.1.x shape: no `tags`, no `deleted_at`, no Inbox tables at all.
        let conn = Connection::open_in_memory().expect("memory db");
        conn.execute_batch(
            "CREATE TABLE notes (id TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '',
             content TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL, is_pinned INTEGER NOT NULL DEFAULT 0);
             INSERT INTO notes VALUES ('old', 'Old note', 'preserved', '2026-01-01', '2026-01-02', 0);
             CREATE TABLE inbox_items (
                 id TEXT PRIMARY KEY,
                 kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file')),
                 title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
                 source TEXT NOT NULL DEFAULT 'manager', context TEXT NOT NULL DEFAULT '',
                 captured_at TEXT NOT NULL, archived_at TEXT DEFAULT NULL,
                 converted_note_id TEXT DEFAULT NULL);
             CREATE TABLE inbox_attachments (
                 id TEXT PRIMARY KEY,
                 item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
                 kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                 name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                 size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL);",
        )
        .expect("legacy schema");

        let before = schema(&conn).expect("measure");
        assert_eq!(before.version, 1, "notes exists, nothing else: {:?}", before.missing);
        assert!(before.legacy_capture_kinds, "the old CHECK was not noticed");

        let after = init_schema(&conn).expect("migrate");
        assert_eq!(after.version, SCHEMA_VERSION);
        assert!(after.missing.is_empty(), "missing: {:?}", after.missing);
        // The same columns as a fresh file -- compared as a set for `notes` and
        // `inbox_items`, because a migrated file has `tags`/`deleted_at` appended at
        // the end while a fresh one has them where the DDL names them. That is
        // upstream behaviour too (`ALTER TABLE` appends), and nothing here depends on
        // position: every statement names its columns.
        let fresh = memory();
        for table in ["notes", "inbox_items"] {
            let mut migrated = columns(&conn, table).expect("migrated columns");
            let mut expected = columns(&fresh, table).expect("fresh columns");
            migrated.sort();
            expected.sort();
            assert_eq!(migrated, expected, "column set of {table} differs after migrating");
        }
        // `inbox_attachments` is the exception worth pinning in order: the Tauri
        // rebuild copies it with a positional `INSERT ... SELECT`, so the column a
        // migration adds has to land in the slot that statement expects.
        assert_eq!(
            columns(&conn, "inbox_attachments").expect("migrated columns"),
            columns(&fresh, "inbox_attachments").expect("fresh columns"),
            "inbox_attachments must keep the order the positional copy relies on"
        );
        let content: String = conn
            .query_row("SELECT content FROM notes WHERE id = 'old'", [], |row| row.get(0))
            .expect("row survives migration");
        assert_eq!(content, "preserved");
        // The rebuild stays Tauri's job: reporting it is the whole contract here.
        assert!(after.legacy_capture_kinds, "the CHECK is not this module's to rewrite");
    }

    #[test]
    fn a_rung_whose_index_went_missing_is_rebuilt_on_open() {
        // A tool that dropped the index used to leave this module permanently
        // behind: the index was only created in the branch that added the column.
        let conn = memory();
        conn.execute_batch("DROP INDEX idx_notes_deleted_at; DROP INDEX idx_inbox_items_deleted_at;")
            .expect("drop");

        let broken = schema(&conn).expect("measure");
        assert_eq!(broken.version, 2, "rung 3 is missing: {:?}", broken.missing);
        assert_eq!(broken.missing, ["notes.deleted_at", "inbox_items.deleted_at"]);

        let repaired = init_schema(&conn).expect("repair");
        assert!(repaired.is_current(), "version {} / {:?}", repaired.version, repaired.missing);
        assert!(index_exists(&conn, "idx_notes_deleted_at").expect("probe"));
        assert!(index_exists(&conn, "idx_inbox_items_deleted_at").expect("probe"));
    }

    #[test]
    fn a_column_this_build_does_not_know_is_reported_rather_than_dropped() {
        let conn = memory();
        conn.execute_batch("ALTER TABLE notes ADD COLUMN color TEXT;")
            .expect("newer build");

        let measured = schema(&conn).expect("measure");
        assert_eq!(measured.version, SCHEMA_VERSION, "an extra column is not a missing rung");
        assert_eq!(measured.ahead, ["notes.color"]);
        assert!(measured.is_current());

        // Opening it again must leave the stranger alone: dropping user data to
        // force the file back to "our" shape is not a migration.
        init_schema(&conn).expect("reopen");
        assert_eq!(columns(&conn, "notes").expect("columns").last().map(String::as_str), Some("color"));
    }

    #[test]
    fn opening_a_current_file_writes_nothing() {
        // The fingerprint is `sqlite_master`, which is what a byte-level comparison
        // of two databases actually moves; a no-op open must not touch it.
        let conn = memory();
        let before = schema_fingerprint(&conn);
        init_schema(&conn).expect("reopen");
        init_schema(&conn).expect("reopen again");
        assert_eq!(before, schema_fingerprint(&conn));
    }

    fn schema_fingerprint(conn: &Connection) -> String {
        let mut stmt = conn
            .prepare("SELECT type, name, tbl_name, COALESCE(sql, '') FROM sqlite_master ORDER BY type, name")
            .expect("prepare");
        let rows = stmt
            .query_map([], |row| {
                Ok(format!(
                    "{}|{}|{}|{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?
                ))
            })
            .expect("query");
        let mut out = Vec::new();
        for row in rows {
            out.push(row.expect("row"));
        }
        out.join("\n")
    }
}
