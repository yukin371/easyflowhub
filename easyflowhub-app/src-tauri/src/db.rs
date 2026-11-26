//! The one place a `notes.db` connection is opened.
//!
//! Four modules used to open the file themselves: each repeated the path, some
//! set a busy timeout, one enabled foreign keys, and each ran its own subset of
//! the migrations. Two of those subsets could interleave -- a notes-only
//! connection never created the Inbox tables, so a command reading Inbox data had
//! to re-open the file through the Inbox module to be sure.
//!
//! The schema itself now lives in `hub-core::store`, which is also the native
//! shell's schema. What is left on this side is connection *policy*, and it is
//! policy applied to every connection: where the file is, how long to wait for a
//! lock, whether foreign keys are enforced, and refusing to hand out a connection
//! whose ladder did not reach the top.

use std::path::PathBuf;
use std::time::Duration;

use hub_core::store::{init_schema, Rebuild};
use rusqlite::Connection;
use tauri::Manager;

/// How long a statement waits for another process to release the write lock.
///
/// Two writers on one file is the normal case -- this app plus the native shell --
/// and without this a momentary lock turns into a failed save.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// `<app_data>/notes.db`, with the directory created if it is missing.
fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&app_data)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;
    Ok(app_data.join(hub_core::store::DB_FILE))
}

/// Open the shared database with the schema brought to this build's version.
///
/// `Rebuild::Run` because this is the process that owns that migration: the
/// `CHECK` rebuild renames both Inbox tables, and doing it here means the native
/// shell can keep refusing (it only reports). The timeout is set before the
/// migration for the same reason a lock can be held by the other process that is
/// also starting up.
pub(crate) fn connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;
    let mut conn = Connection::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|e| format!("Failed to configure database lock timeout: {}", e))?;
    // Connection policy, not schema. `ON DELETE CASCADE` on inbox attachments is
    // inert without it, and the pragma is per connection, so it belongs on every
    // connection this app hands out rather than in one module's setup.
    conn.execute_batch("PRAGMA foreign_keys = ON")
        .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;

    let report = init_schema(&mut conn, Rebuild::Run)
        .map_err(|e| format!("Failed to initialize database: {}", e))?;
    // A ladder that stopped short is not a file this build can read correctly.
    // Failing here names the rung; the alternative is a command that later fails
    // with "no such column", far from the cause.
    if let Some(rung) = report.missing.first() {
        return Err(format!(
            "Failed to initialize database: schema rung `{rung}` is still missing after migrating"
        ));
    }
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use hub_core::store::{init_schema, Rebuild, SCHEMA_VERSION};

    #[test]
    fn a_fresh_file_reaches_the_version_this_build_expects() {
        // The wiring check for the delegate: the app's `connection` cannot run
        // against a real app data directory here, but the ladder it calls can, and
        // the rung names in the error message above come from this same report.
        let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory database");
        let report = init_schema(&mut conn, Rebuild::Run).expect("migrate");
        assert!(report.is_current(), "schema: {report:?}");
        assert_eq!(report.version, SCHEMA_VERSION);
        assert!(report.missing.is_empty());
        assert!(!report.legacy_capture_kinds);
    }
}
