//! Direct access to the product's `notes.db`.
//!
//! This is the S3 question in one module: can the native shell read and write the
//! database the Tauri build already owns, without changing its contract?
//!
//! Four rules are load-bearing, and each has a measured reason behind it:
//!
//! * **This crate owns the DDL; the Tauri build owns the connection.** The schema
//!   used to exist twice -- once in `notes.rs` as column probes, once here -- kept
//!   aligned by a test that parsed their source. There is now one writer
//!   ([`init_schema`]), called by both processes, and the guard points the other way:
//!   [`tests::the_tauri_build_owns_no_ddl`] fails if a `CREATE`, an `ALTER` or a
//!   `PRAGMA user_version` reappears in that crate's production code, because that is
//!   how a second author of the same file would start.
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
//!   [`SCHEMA_VERSION`], so "the ladder reached the top" is checked on every open
//!   instead of being asserted in a comment.
//!
//! Nothing here changes `journal_mode`, `user_version`, `page_size` or `encoding`.
//! Switching to WAL would rewrite the file header and leave `-wal`/`-shm` siblings,
//! which is a product-level decision (backup scripts, file sync, "copy the db" all
//! change meaning) and not something a spike may decide on the user's behalf.

mod attachments;
mod inbox;
mod notes;
mod recent;
mod schema;
mod settings;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

pub use attachments::{StoredAttachment, ATTACHMENT_DIR};
pub use inbox::InboxRow;
pub use notes::{NoteRow, NoteWrite, TODO_INBOX_TITLE, TRASH_RETENTION_DAYS};
pub use recent::{ensure_file_search_recent, FILE_SEARCH_RECENT_DDL};
pub use schema::{init_schema, schema, Rebuild, Schema, NOTES_DDL, SCHEMA_VERSION, SETTINGS_DDL};
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
    /// The write was refused for a reason the caller can act on: a destination that
    /// is already occupied, an id that would escape its directory, a manifest that
    /// is not the shape it claims. Distinct from [`StoreError::Io`] because "this
    /// would overwrite your file" is not a disk failure and must not read as one.
    Refused(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Sqlite(error) => write!(f, "sqlite: {error}"),
            StoreError::Io(error) => write!(f, "io: {error}"),
            StoreError::Missing(what) => write!(f, "missing: {what}"),
            StoreError::Refused(why) => write!(f, "refused: {why}"),
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
        // A first run has no app data directory yet, and SQLite reports that as
        // "unable to open database file" -- which the shell turns into a
        // memory-only session. Every note written in it is gone when the window
        // closes, and nothing on screen says so. `settings::save` already creates
        // its parent; the database does the same for the same reason.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut conn = Connection::open(&path)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        // `Defer`: a shell that is here to read or write one note has no business
        // renaming two Inbox tables. The legacy CHECK is reported, not rewritten.
        let report = init_schema(&mut conn, Rebuild::Defer)?;
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
        Ok(schema::schema(&self.conn)?)
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

    /// The retention window, applied to every table that has one.
    ///
    /// One method rather than two calls at the call site: "保留 30 天" is a promise
    /// about the trash, not about a table, and a sweep that covered notes while
    /// ignoring captures would be a leak with a reassuring sentence over it -- which
    /// is exactly the failure the notes-only version was written to fix.
    pub fn purge_expired(&mut self, now: &str, days: i64) -> Result<Sweep> {
        let notes = self.purge_expired_trash(now, days)?;
        let inbox = self.purge_expired_inbox(now, days)?;
        Ok(Sweep { notes, inbox })
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
        Ok(schema::table_exists(&self.conn, table)?)
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

/// What one retention sweep took, per table with a retention window.
///
/// Reported as a pair rather than a sum: the log line and the status sentence both
/// say how much of each, so "cleaned 3" cannot be read as "cleaned 3 notes" when it
/// was one note and two captures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sweep {
    pub notes: usize,
    pub inbox: usize,
}

impl Sweep {
    pub fn total(self) -> usize {
        self.notes + self.inbox
    }
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
        let mut conn = Connection::open_in_memory().expect("memory db");
        conn.busy_timeout(BUSY_TIMEOUT).expect("timeout");
        init_schema(&mut conn, Rebuild::Defer).expect("schema");
        Store {
            conn,
            path: PathBuf::from(":memory:"),
        }
    }

    #[test]
    fn schema_creation_is_idempotent() {
        // The product opens a connection per command, so this runs dozens of times
        // per session on an already-migrated file.
        let mut store = memory();
        init_schema(&mut store.conn, Rebuild::Defer).expect("second pass");
        init_schema(&mut store.conn, Rebuild::Defer).expect("third pass");
        assert!(store.integrity_ok().expect("integrity"));
    }

    #[test]
    fn a_legacy_table_without_tags_or_deleted_at_is_migrated_in_place() {
        // This is the shape a 0.1.x build left behind, taken from the migration
        // tests in notes.rs.
        let mut conn = Connection::open_in_memory().expect("memory db");
        conn.execute_batch(
            "CREATE TABLE notes (id TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '',
             content TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL, is_pinned INTEGER NOT NULL DEFAULT 0);
             INSERT INTO notes VALUES ('old', 'Old note', 'preserved', '2026-01-01', '2026-01-02', 0);",
        )
        .expect("legacy schema");
        init_schema(&mut conn, Rebuild::Defer).expect("migrate");
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
    fn a_missing_app_data_directory_is_created_instead_of_failing() {
        // The first run of a fresh install: `%APPDATA%\com.yukin.easyflowhub` does
        // not exist yet. SQLite answers "unable to open database file" and the
        // shell silently falls back to memory, so the check that matters is that
        // a note written after this open is still there for the next process.
        let root = std::env::temp_dir().join("hub-core-fresh-root").join("nested");
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join("hub-core-fresh-root"));
        let store = Store::open_in(&root).expect("a fresh root opens");
        assert_eq!(store.path(), root.join(DB_FILE).as_path());
        drop(store);

        let reopened = Store::open_in(&root).expect("and reopens");
        assert!(reopened.integrity_ok().expect("integrity"));
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join("hub-core-fresh-root"));
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
    fn the_declared_columns_are_the_ones_our_ddl_creates() {
        // The other half of the ownership check: the lists `ahead_columns` compares
        // against must describe the file this crate builds, in the order a fresh file
        // physically gets, or a downgrade would be reported on a healthy database.
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

    /// The ownership check, pointing the other way.
    ///
    /// While both processes carried a copy of the schema the useful question was "do
    /// our column lists still match theirs", and this test read their source to answer
    /// it. `init_schema` is now the only writer, so the question inverts: does the
    /// other crate still hold a `CREATE`, an `ALTER` or a `PRAGMA user_version` that
    /// would make a second author? Test fixtures are exempt -- building a
    /// pre-migration table is how a migration gets proven -- and they are skipped item
    /// by item rather than by truncating the file, because `settings.rs` keeps
    /// production commands *below* its test module.
    #[test]
    fn the_tauri_build_owns_no_ddl() {
        const TAURI_SRC: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../easyflowhub-app/src-tauri/src"
        );
        // Every way a second owner could reappear. `PRAGMA user_version` is in the
        // list because stamping the number is the other half of owning the schema.
        const FORBIDDEN: &[&str] = &[
            "CREATE TABLE",
            "CREATE INDEX",
            "CREATE UNIQUE INDEX",
            "CREATE VIEW",
            "CREATE TRIGGER",
            "ALTER TABLE",
            "DROP TABLE",
            "DROP INDEX",
            "PRAGMA user_version",
        ];

        // Canonicalised so a failure names the file the way an editor does.
        let root = std::fs::canonicalize(TAURI_SRC)
            .unwrap_or_else(|error| panic!("{TAURI_SRC}: {error}"));
        let files = rust_sources(&root);
        // A path that resolves to nothing would make this pass for the wrong reason.
        assert!(
            files.len() > 20,
            "{}: only {} Rust files found, the guard is looking at the wrong place",
            root.display(),
            files.len()
        );

        let mut offenders = Vec::new();
        for file in &files {
            let source = std::fs::read_to_string(file)
                .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
            for (number, line) in production_lines(&source) {
                for needle in FORBIDDEN {
                    if line.contains(needle) {
                        offenders.push(format!("{}:{number}: {needle}", file.display()));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "the Tauri build must not own DDL; `hub-core::store::schema` is the only writer:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn the_ddl_scan_skips_test_items_and_nothing_else() {
        // The guard's own blind spot: a skip that eats too much would hide real DDL.
        let source = "\
fn production() {}

#[cfg(test)]
mod tests {
    const FIXTURE: &str = \"CREATE TABLE legacy (id TEXT)\";
}

fn after_the_tests() {}
";
        let production: Vec<&str> = production_lines(source).into_iter().map(|(_, l)| l).collect();
        assert!(production.iter().any(|line| line.contains("fn production")));
        // Production code below a test module is still production code.
        assert!(production.iter().any(|line| line.contains("fn after_the_tests")));
        assert!(!production.iter().any(|line| line.contains("CREATE TABLE")));
    }

    /// Every `.rs` under `dir`, recursively.
    fn rust_sources(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(next) = pending.pop() {
            let entries = std::fs::read_dir(&next)
                .unwrap_or_else(|error| panic!("{}: {error}", next.display()));
            for entry in entries {
                let path = entry.expect("directory entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }
        out
    }

    /// The lines of `source` that are production code, with their 1-based numbers.
    fn production_lines(source: &str) -> Vec<(usize, &str)> {
        let lines: Vec<&str> = source.split('\n').collect();
        let mut out = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            if lines[index].trim() == "#[cfg(test)]" {
                index = skip_cfg_test_item(&lines, index);
                continue;
            }
            out.push((index + 1, lines[index]));
            index += 1;
        }
        out
    }

    /// First line after the `#[cfg(test)]` item whose attribute is at `start`.
    ///
    /// Brace counting. The bodies are test code full of SQL strings, but every brace
    /// inside one of those strings is balanced, so the count closes on the item's own
    /// `}` rather than on a quote.
    fn skip_cfg_test_item(lines: &[&str], start: usize) -> usize {
        let mut index = start + 1;
        // Further attributes belong to the same item.
        while index < lines.len() && lines[index].trim_start().starts_with("#[") {
            index += 1;
        }
        if index == lines.len() {
            return index;
        }
        if !lines[index].contains('{') {
            // `mod schema;` or `pub(crate) use ...;`
            while index < lines.len() && !lines[index].trim_end().ends_with(';') {
                index += 1;
            }
            return index + 1;
        }
        let mut depth = 0i32;
        while index < lines.len() {
            for byte in lines[index].bytes() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
            }
            index += 1;
            if depth <= 0 {
                break;
            }
        }
        index
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
