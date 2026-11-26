//! Direct access to the product's `notes.db`.
//!
//! This is the S3 question in one module: can the native shell read and write the
//! database the Tauri build already owns, without changing its contract?
//!
//! Three rules are load-bearing, and each has a measured reason behind it:
//!
//! * **The DDL is copied, not re-invented.** `notes.rs` in the Tauri crate evolves
//!   the schema by probing for columns (`pragma_table_info`) instead of tracking a
//!   version number -- the live database has `user_version = 0`. A native side that
//!   invented its own migrations would eventually disagree with that probe, so
//!   [`notes_ddl`] is the same text and [`tests::the_legacy_migrations_are_all_mirrored`]
//!   fails when the other crate grows a column we do not know about.
//! * **`IMMEDIATE` transactions.** The Tauri side takes the write lock up front
//!   (`TransactionBehavior::Immediate`); a deferred transaction that later upgrades
//!   can deadlock against another process. Same choice here.
//! * **`busy_timeout` is not optional.** Two processes on one file is the normal
//!   case (the resident shell plus the Tauri app during the migration). Without the
//!   timeout, a momentary lock becomes a hard error in the middle of a save.
//! * **The schema version is measured, not stamped.** `schema.rs` keeps the
//!   migrations as a ladder of rungs and reports which ones are on the file; see
//!   that module for why `PRAGMA user_version` is the wrong place for the number.
//!   [`Store::open`] refuses a connection whose ladder did not end at
//!   [`SCHEMA_VERSION`], so "we mirror the Tauri migrations" is checked on every
//!   open instead of being asserted in a comment.
//!
//! Nothing here changes `journal_mode`, `user_version`, `page_size` or `encoding`.
//! Switching to WAL would rewrite the file header and leave `-wal`/`-shm` siblings,
//! which is a product-level decision (backup scripts, file sync, "copy the db" all
//! change meaning) and not something a spike may decide on the user's behalf.

mod attachments;
mod notes;
mod schema;
mod settings;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

pub use attachments::{StoredAttachment, ATTACHMENT_DIR};
pub use notes::{NoteRow, NoteWrite};
pub use schema::{init_schema, schema, Schema, NOTES_DDL, SCHEMA_VERSION, SETTINGS_DDL};
pub use settings::SettingRow;

/// The `identifier` from `easyflowhub-app/src-tauri/tauri.conf.json`.
///
/// Tauri's `app_data_dir()` on Windows resolves to `%APPDATA%\{identifier}`, so the
/// native shell can find the same directory from the environment alone. That is an
/// assumption about how Tauri builds the path, not something this crate can prove
/// by itself -- the S3 gate prints the resolved root, the file size and the mtime,
/// so a wrong guess shows up as "empty database" instead of a silent second store.
pub const DEFAULT_APP_DIR: &str = "com.yukin.easyflowhub";

pub const DB_FILE: &str = "notes.db";

/// A fresh note id.
///
/// Same crate the other build uses to mint ids, so a row's id looks identical no
/// matter which process wrote it -- anything that keys on the shape of an id (log
/// greps, the asset naming convention) keeps working across the two.
pub fn new_note_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// How long a writer waits for the other process before giving up, matching
/// `notes.rs::get_connection`.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    /// The database is there but a row we require is not.
    Missing(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Sqlite(error) => write!(f, "sqlite: {error}"),
            StoreError::Io(error) => write!(f, "io: {error}"),
            StoreError::Missing(what) => write!(f, "missing: {what}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Sqlite(error)
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        StoreError::Io(error)
    }
}

/// The shared database. One instance is one connection; the product opens a fresh
/// one per command, and so do we, because holding a connection open across a
/// hotspot reveal is exactly the kind of thing that turns a lock into a stall.
pub struct Store {
    conn: Connection,
    path: PathBuf,
}

impl Store {
    /// Open (or create) the database at an explicit path and bring the schema to
    /// the shape the Tauri build expects.
    ///
    /// Fails rather than hands out a half-migrated connection: if a rung could not
    /// be built, every later query would answer with a shape nobody declared, and
    /// the failure would surface as "column not found" inside a save instead of
    /// here, where the cause is still visible.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = Connection::open(&path)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        let report = init_schema(&conn)?;
        if let Some(rung) = report.missing.first() {
            return Err(StoreError::Missing(format!(
                "schema rung `{rung}` (at {} of {SCHEMA_VERSION} after migrating {})",
                report.version,
                path.display()
            )));
        }
        Ok(Self { conn, path })
    }

    /// Open `{root}/notes.db`, where `root` is the app data directory.
    pub fn open_in(root: impl AsRef<Path>) -> Result<Self> {
        Self::open(root.as_ref().join(DB_FILE))
    }

    /// Resolve the app data directory the way the Tauri build does, then open it.
    pub fn open_default() -> Result<Self> {
        Self::open_in(default_root()?)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
    }

    /// Directory holding attachment files (`assets/` next to the database).
    pub fn attachment_dir(&self) -> PathBuf {
        self.root().join(ATTACHMENT_DIR)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Which rungs this file has, and what is on it that this build does not know.
    ///
    /// Re-measured on demand rather than cached from `open`: another process can
    /// migrate the file while we are running, and a cached "we are current" would
    /// then be a claim about a database that no longer exists.
    pub fn schema(&self) -> Result<Schema> {
        schema::schema(&self.conn)
    }

    /// Version of the SQLite actually linked into this binary. Archived on every
    /// run: the native and Tauri workspaces have separate lockfiles, so the bundled
    /// SQLite can drift, and a drift is only harmless if someone notices it.
    pub fn sqlite_version(&self) -> String {
        rusqlite::version().to_string()
    }

    /// `sqlite_master` serialised in a stable order. Comparing this before and after
    /// a native write is what "no schema break" means concretely -- a text diff is
    /// checkable, "we did not run DDL" is not.
    pub fn schema_fingerprint(&self) -> Result<String> {
        let mut stmt = self.conn.prepare(
            "SELECT type, name, tbl_name, COALESCE(sql, '')
             FROM sqlite_master
             ORDER BY type, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(format!(
                "{}|{}|{}|{}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out.join("\n"))
    }

    /// `PRAGMA integrity_check`. Anything other than `ok` means the file itself is
    /// damaged, which is the one failure mode no amount of application logic covers.
    pub fn integrity_ok(&self) -> Result<bool> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        Ok(result == "ok")
    }

    /// Contract values that must survive a native write untouched.
    pub fn contract(&self) -> Result<Contract> {
        Ok(Contract {
            journal_mode: self
                .conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))?,
            user_version: self
                .conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))?,
            page_size: self
                .conn
                .query_row("PRAGMA page_size", [], |row| row.get(0))?,
            encoding: self
                .conn
                .query_row("PRAGMA encoding", [], |row| row.get(0))?,
        })
    }

    /// Row counts per table that exists. `file_search_recent` is created lazily by a
    /// feature the user may never have triggered, so absence is reported as `None`
    /// rather than treated as zero. The same goes for a `deleted_at` column that an
    /// older build never added: a report about the data must not fail because one
    /// optional column is missing.
    pub fn counts(&self) -> Result<Counts> {
        Ok(Counts {
            notes: self.count("notes")?,
            notes_deleted: if self.table_exists("notes")?
                && schema::column_exists(&self.conn, "notes", "deleted_at")?
            {
                self.count_where("notes", "deleted_at IS NOT NULL")?
            } else {
                None
            },
            settings: self.count("settings")?,
            inbox_items: self.count("inbox_items")?,
            inbox_attachments: self.count("inbox_attachments")?,
            file_search_recent: self.count("file_search_recent")?,
        })
    }

    fn count(&self, table: &str) -> Result<Option<i64>> {
        if !self.table_exists(table)? {
            return Ok(None);
        }
        let sql = format!("SELECT COUNT(*) FROM {table}");
        Ok(Some(self.conn.query_row(&sql, [], |row| row.get(0))?))
    }

    fn count_where(&self, table: &str, predicate: &str) -> Result<Option<i64>> {
        if !self.table_exists(table)? {
            return Ok(None);
        }
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE {predicate}");
        Ok(Some(self.conn.query_row(&sql, [], |row| row.get(0))?))
    }

    fn table_exists(&self, table: &str) -> Result<bool> {
        schema::table_exists(&self.conn, table)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    pub journal_mode: String,
    pub user_version: i64,
    pub page_size: i64,
    pub encoding: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    pub notes: Option<i64>,
    pub notes_deleted: Option<i64>,
    pub settings: Option<i64>,
    pub inbox_items: Option<i64>,
    pub inbox_attachments: Option<i64>,
    pub file_search_recent: Option<i64>,
}

/// `%APPDATA%\com.yukin.easyflowhub`.
pub fn default_root() -> Result<PathBuf> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| StoreError::Missing("APPDATA is not set".to_string()))?;
    Ok(PathBuf::from(appdata).join(DEFAULT_APP_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Store {
        let conn = Connection::open_in_memory().expect("memory db");
        conn.busy_timeout(BUSY_TIMEOUT).expect("timeout");
        init_schema(&conn).expect("schema");
        Store {
            conn,
            path: PathBuf::from(":memory:"),
        }
    }

    #[test]
    fn schema_creation_is_idempotent() {
        // The product opens a connection per command, so this runs dozens of times
        // per session on an already-migrated file.
        let store = memory();
        init_schema(&store.conn).expect("second pass");
        init_schema(&store.conn).expect("third pass");
        assert!(store.integrity_ok().expect("integrity"));
    }

    #[test]
    fn a_legacy_table_without_tags_or_deleted_at_is_migrated_in_place() {
        // This is the shape a 0.1.x build left behind, taken from the migration
        // tests in notes.rs.
        let conn = Connection::open_in_memory().expect("memory db");
        conn.execute_batch(
            "CREATE TABLE notes (id TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '',
             content TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL, is_pinned INTEGER NOT NULL DEFAULT 0);
             INSERT INTO notes VALUES ('old', 'Old note', 'preserved', '2026-01-01', '2026-01-02', 0);",
        )
        .expect("legacy schema");
        init_schema(&conn).expect("migrate");
        let content: String = conn
            .query_row("SELECT content FROM notes WHERE id = 'old'", [], |row| row.get(0))
            .expect("row survives migration");
        assert_eq!(content, "preserved");
        let tags: String = conn
            .query_row("SELECT tags FROM notes WHERE id = 'old'", [], |row| row.get(0))
            .expect("tags defaulted");
        assert_eq!(tags, "");
    }

    #[test]
    fn the_contract_values_are_untouched_by_our_ddl() {
        let store = memory();
        let contract = store.contract().expect("contract");
        // An in-memory database reports `memory` for journal_mode; the assertion that
        // matters is that we never send `PRAGMA journal_mode = ...`.
        assert_eq!(contract.user_version, 0);
        assert_eq!(contract.encoding, "UTF-8");
        assert_eq!(contract.page_size, 4096);
    }

    #[test]
    fn the_legacy_migrations_are_all_mirrored() {
        // Drift guard, against both shapes the Tauri build grows a column in:
        //   * `CREATE TABLE` (one column per line, `source_path` arrived this way);
        //   * `ALTER TABLE ... ADD COLUMN` (the 0.1.x migrations).
        // Either one landing upstream without landing here means the two processes
        // disagree about the same file, and the one that opened it first wins.
        const NOTES: &str = include_str!("../../../../../easyflowhub-app/src-tauri/src/notes.rs");
        const INBOX: &str =
            include_str!("../../../../../easyflowhub-app/src-tauri/src/inbox/schema.rs");

        for (source, table, declared) in [
            (NOTES, "notes", schema::NOTES_COLUMNS),
            (INBOX, "inbox_items", schema::INBOX_ITEM_COLUMNS),
            (INBOX, "inbox_attachments", schema::INBOX_ATTACHMENT_COLUMNS),
        ] {
            let upstream = upstream_columns(source, table);
            assert_eq!(
                upstream, declared,
                "{table}: hub-core does not declare the columns the Tauri build creates"
            );
        }

        // And the file the DDL claims to produce really has them, in that order.
        let store = memory();
        for (table, declared) in [
            ("notes", schema::NOTES_COLUMNS),
            ("inbox_items", schema::INBOX_ITEM_COLUMNS),
            ("inbox_attachments", schema::INBOX_ATTACHMENT_COLUMNS),
        ] {
            let actual = schema::columns(&store.conn, table).expect("probe");
            assert_eq!(actual, declared, "{table}: physical column order differs");
        }
    }

    /// Columns a table gets in `notes.rs` / `inbox/schema.rs`, in physical order: the
    /// ones the base `CREATE TABLE` names, then the ones an `ALTER` appends.
    fn upstream_columns(source: &str, table: &str) -> Vec<&'static str> {
        // Leaked on purpose: the assertion compares against `&'static str` tables, and
        // a test process leaking a few dozen bytes is cheaper than a second type.
        let source: &'static str = Box::leak(source.to_string().into_boxed_str());
        let mut out: Vec<&'static str> = Vec::new();

        let marker = format!("CREATE TABLE IF NOT EXISTS {table} (");
        if let Some(start) = source.find(&marker) {
            let body = &source[start + marker.len()..];
            if let Some(end) = body.find(");") {
                for line in body[..end].lines() {
                    let line = line.trim();
                    let Some(first) = line.split_whitespace().next() else {
                        continue;
                    };
                    // Table-level constraints, not columns.
                    if matches!(first, "CHECK" | "FOREIGN" | "PRIMARY" | "UNIQUE") {
                        continue;
                    }
                    let name = first.trim_end_matches(',');
                    if !name.is_empty() {
                        out.push(name);
                    }
                }
            }
        }

        let alter = format!("ALTER TABLE {table} ADD COLUMN ");
        for line in source.lines() {
            let Some(rest) = line.split(&alter).nth(1) else {
                continue;
            };
            if let Some(name) = rest.split_whitespace().next() {
                out.push(name);
            }
        }

        // `tags` is both in the base statement and in an `ALTER` (the 0.1.x path), so
        // the union keeps the first position and drops the repeat.
        let mut unique: Vec<&'static str> = Vec::new();
        for name in out {
            if !unique.contains(&name) {
                unique.push(name);
            }
        }
        unique
    }

    #[test]
    fn missing_tables_are_reported_as_absent_rather_than_zero() {
        let conn = Connection::open_in_memory().expect("memory db");
        conn.execute_batch(NOTES_DDL).expect("notes only");
        let store = Store {
            conn,
            path: PathBuf::from(":memory:"),
        };
        let counts = store.counts().expect("counts");
        assert_eq!(counts.file_search_recent, None);
    }
}
