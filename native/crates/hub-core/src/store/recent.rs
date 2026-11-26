//! `file_search_recent`: the file-search panel's cache of recently opened files.
//!
//! It is deliberately **not** a rung. A rung is built by *opening* the database, and
//! the live file does not have this table; creating it on open would turn "read one
//! note" into a write and break the S3 gate's promise that an open leaves the file
//! alone. The DDL lives here -- one definition for both processes, which is the point
//! of this module -- and the table appears the first time the panel needs it.
//!
//! `ahead` report: the table is not one of the three the ladder owns, so a column
//! added to it by a newer build is not reported. That is fine for a cache: the worst
//! case is a cached row that is refetched.

use rusqlite::{Connection, Result};

pub const FILE_SEARCH_RECENT_DDL: &str =
    "CREATE TABLE IF NOT EXISTS file_search_recent (path TEXT PRIMARY KEY, opened_at INTEGER NOT NULL);";

/// Create the cache table if this is the first time anything needed it.
pub fn ensure_file_search_recent(conn: &Connection) -> Result<()> {
    conn.execute_batch(FILE_SEARCH_RECENT_DDL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{init_schema, schema, Rebuild, SCHEMA_VERSION};

    #[test]
    fn the_recent_table_is_not_part_of_the_ladder() {
        // Creating it must not move the measured version, and its columns must not
        // show up as something a newer build left behind.
        let mut conn = Connection::open_in_memory().expect("memory db");
        init_schema(&mut conn, Rebuild::Defer).expect("schema");
        ensure_file_search_recent(&conn).expect("create");
        let report = schema(&conn).expect("measure");
        assert_eq!(report.version, SCHEMA_VERSION);
        assert!(report.ahead.is_empty(), "ahead: {:?}", report.ahead);
    }

    #[test]
    fn ensuring_twice_leaves_one_table() {
        let mut conn = Connection::open_in_memory().expect("memory db");
        init_schema(&mut conn, Rebuild::Defer).expect("schema");
        ensure_file_search_recent(&conn).expect("first");
        ensure_file_search_recent(&conn).expect("second");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'file_search_recent'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(count, 1);
    }
}
