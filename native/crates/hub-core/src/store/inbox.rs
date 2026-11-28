//! The Inbox's trash: soft-deleted capture items, and what "permanent" means for
//! them.
//!
//! ## Why this is not a second copy of `notes.rs`
//!
//! A capture item is not a note: it has a `kind` from a fixed set, an optional
//! attachment, and a delete that has to take the *file* with the row. The row
//! operations are the same shape as the note ones (`deleted_at IS NOT NULL`), but
//! the purge is not, and hiding that difference behind a shared helper would make
//! the one interesting part invisible.
//!
//! ## Permanent means the bytes too
//!
//! `purge` deletes rows and then the attachment files those rows named. Row first,
//! file second, and best effort on the file: a file that survives its row is a few
//! kilobytes nobody can reach, while a row that survives its file is an entry that
//! looks restorable and is not. The same asymmetry the attachment writer already
//! chose (`store_attachment` keeps the row and drops the leftover file).
//!
//! The filenames are read *before* the delete, inside the same transaction, because
//! after the cascade there is nothing left to tell us which files were ours.

use rusqlite::{params, TransactionBehavior};

use super::{Result, Store};

/// One trashed capture item, as the trash list needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRow {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub captured_at: String,
    pub deleted_at: Option<String>,
    /// Names of the files under `assets/`, from `inbox_attachments`.
    pub attachments: Vec<String>,
}

impl Store {
    /// Everything in the Inbox's trash, newest delete first.
    ///
    /// The attachment names come along because the list says "附件：a.png" and a
    /// purge has to delete them; one query with a grouped `JOIN` answers both
    /// without a per-row lookup.
    pub fn trashed_inbox(&self) -> Result<Vec<InboxRow>> {
        let mut stmt = self.connection().prepare(
            "SELECT i.id, i.kind, i.title, i.content, i.captured_at, i.deleted_at,
                    COALESCE(GROUP_CONCAT(a.filename, char(31)), '')
             FROM inbox_items i
             LEFT JOIN inbox_attachments a ON a.item_id = i.id
             WHERE i.deleted_at IS NOT NULL
             GROUP BY i.id
             ORDER BY i.deleted_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let names: String = row.get(6)?;
            Ok(InboxRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                captured_at: row.get(4)?,
                deleted_at: row.get(5)?,
                // `char(31)` (unit separator) rather than a comma: a filename may
                // contain a comma, and splitting on one would invent an attachment.
                attachments: names
                    .split('\u{1f}')
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .collect(),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Put capture items back. Counts only the rows that were actually in the
    /// trash, so "已恢复 2 条" cannot be a lie about a row that was already live.
    pub fn restore_inbox(&mut self, ids: &[String]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut changed = 0;
        {
            let mut statement = tx.prepare(
                "UPDATE inbox_items SET deleted_at = NULL
                 WHERE id = ?1 AND deleted_at IS NOT NULL",
            )?;
            for id in ids {
                changed += statement.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Delete one capture item for good, attachments included.
    pub fn purge_inbox(&mut self, id: &str) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let names = attachment_names(&tx, "i.id = ?1", params![id])?;
        // Only a *trashed* row can be purged: a live capture that some other window
        // just restored must not disappear because this one was one frame behind.
        let changed = tx.execute(
            "DELETE FROM inbox_items WHERE id = ?1 AND deleted_at IS NOT NULL",
            [id],
        )?;
        tx.commit()?;
        if changed > 0 {
            self.drop_attachment_files(&names);
        }
        Ok(changed > 0)
    }

    /// Empty the Inbox's trash. Same rule as the notes' `purge_trashed`.
    pub fn purge_trashed_inbox(&mut self) -> Result<usize> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let names = attachment_names(&tx, "i.deleted_at IS NOT NULL", [])?;
        let changed = tx.execute("DELETE FROM inbox_items WHERE deleted_at IS NOT NULL", [])?;
        tx.commit()?;
        self.drop_attachment_files(&names);
        Ok(changed)
    }

    /// The retention sweep, for this table.
    ///
    /// `now` unreadable or `days <= 0` deletes nothing: this is the destructive path,
    /// and a guessed clock is not a reason to take someone's captures. Same rule as
    /// `notes::purge_expired_trash`, and the same cutoff arithmetic.
    pub fn purge_expired_inbox(&mut self, now: &str, days: i64) -> Result<usize> {
        if days <= 0 {
            return Ok(0);
        }
        let Some(ms) = crate::clock::unix_ms(now) else {
            return Ok(0);
        };
        let cutoff = crate::clock::iso_from_unix(ms / 1_000 - days * 86_400, 0);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // The join is on the item, so the files of an expired item are collected
        // before the cascade takes the rows that name them.
        let names = attachment_names(
            &tx,
            "i.deleted_at IS NOT NULL AND i.deleted_at < ?1",
            params![cutoff],
        )?;
        let changed = tx.execute(
            "DELETE FROM inbox_items WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
            [&cutoff],
        )?;
        tx.commit()?;
        self.drop_attachment_files(&names);
        Ok(changed)
    }

    /// Best-effort removal of the files behind rows that are already gone.
    fn drop_attachment_files(&self, names: &[String]) {
        for name in names {
            let _ = std::fs::remove_file(self.attachment_file(name));
        }
    }
}

/// Filenames of the attachments belonging to items matching `predicate`.
fn attachment_names(
    tx: &rusqlite::Transaction<'_>,
    predicate: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<String>> {
    let sql = format!(
        "SELECT a.filename FROM inbox_attachments a
         JOIN inbox_items i ON i.id = a.item_id
         WHERE {predicate}"
    );
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params, |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{init_schema, Rebuild, ATTACHMENT_DIR, DB_FILE};
    use rusqlite::Connection;
    use std::fs;

    const LONG_AGO: &str = "2026-09-01T00:00:00.000000000+00:00";
    const NOW: &str = "2026-11-28T12:00:00.000000000+00:00";

    fn store(tag: &str) -> Store {
        let mut conn = Connection::open_in_memory().expect("memory db");
        init_schema(&mut conn, Rebuild::Defer).expect("schema");
        let root = std::env::temp_dir().join(format!("hub-inbox-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(ATTACHMENT_DIR)).expect("assets");
        Store {
            conn,
            path: root.join(DB_FILE),
        }
    }

    fn item(store: &Store, id: &str, deleted: Option<&str>) {
        store
            .connection()
            .execute(
                "INSERT INTO inbox_items (id, kind, title, content, captured_at, deleted_at)
                 VALUES (?1, 'text', ?1, '正文', '2026-11-20T00:00:00+00:00', ?2)",
                params![id, deleted],
            )
            .expect("item");
    }

    fn attachment(store: &Store, id: &str, item: &str, filename: &str) -> std::path::PathBuf {
        store
            .connection()
            .execute(
                "INSERT INTO inbox_attachments
                     (id, item_id, kind, name, mime_type, filename, size_bytes, created_at)
                 VALUES (?1, ?2, 'image', '图片', 'image/png', ?3, 3, '2026-11-20T00:00:00+00:00')",
                params![id, item, filename],
            )
            .expect("attachment row");
        let path = store.attachment_file(filename);
        fs::write(&path, b"png").expect("attachment file");
        path
    }

    #[test]
    fn the_trash_lists_only_what_was_deleted_and_names_its_files() {
        let store = store("list");
        item(&store, "live", None);
        item(&store, "gone", Some(NOW));
        attachment(&store, "a1", "gone", "one.png");
        attachment(&store, "a2", "gone", "two.png");

        let rows = store.trashed_inbox().expect("list");
        assert_eq!(rows.len(), 1, "a live capture is not in the trash");
        assert_eq!(rows[0].id, "gone");
        assert_eq!(rows[0].kind, "text");
        let mut names = rows[0].attachments.clone();
        names.sort();
        assert_eq!(names, ["one.png", "two.png"]);
    }

    #[test]
    fn a_name_with_a_comma_survives_the_grouped_read() {
        // The list joins attachment names into one column; splitting on a comma
        // would turn one file into two entries that do not exist.
        let store = store("comma");
        item(&store, "gone", Some(NOW));
        attachment(&store, "a1", "gone", "a,b.png");
        let rows = store.trashed_inbox().expect("list");
        assert_eq!(rows[0].attachments, ["a,b.png"]);
    }

    #[test]
    fn restoring_counts_what_it_actually_changed() {
        let mut store = store("restore");
        item(&store, "gone", Some(NOW));
        item(&store, "live", None);
        let changed = store
            .restore_inbox(&["gone".to_string(), "live".to_string(), "nope".to_string()])
            .expect("restore");
        assert_eq!(changed, 1, "a live row and a missing row are not restorations");
        let deleted: Option<String> = store
            .connection()
            .query_row("SELECT deleted_at FROM inbox_items WHERE id = 'gone'", [], |row| {
                row.get(0)
            })
            .expect("row");
        assert!(deleted.is_none());
    }

    #[test]
    fn purging_one_takes_the_row_and_its_file_and_nothing_else() {
        let mut store = store("purge-one");
        item(&store, "gone", Some(NOW));
        item(&store, "keep", Some(NOW));
        let file = attachment(&store, "a1", "gone", "gone.png");
        let kept = attachment(&store, "a2", "keep", "keep.png");

        assert!(store.purge_inbox("gone").expect("purge"));
        assert!(!file.exists(), "the bytes must go with the row");
        assert!(kept.exists(), "another item's attachment is not ours to delete");
        let left: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM inbox_items WHERE id = 'gone'", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(left, 0);
    }

    #[test]
    fn a_live_capture_cannot_be_purged_by_a_stale_frame() {
        // Restoring it in another window and purging it from this one must not race
        // into "the row was live and now it is gone".
        let mut store = store("purge-live");
        item(&store, "live", None);
        assert!(!store.purge_inbox("live").expect("purge"));
        let left: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM inbox_items", [], |row| row.get(0))
            .expect("count");
        assert_eq!(left, 1);
    }

    #[test]
    fn emptying_takes_every_trashed_row_and_no_live_one() {
        let mut store = store("empty");
        item(&store, "live", None);
        for id in ["gone-a", "gone-b"] {
            item(&store, id, Some(NOW));
        }
        assert_eq!(store.purge_trashed_inbox().expect("empty"), 2);
        let rows = store.trashed_inbox().expect("list");
        assert!(rows.is_empty());
        let live: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE deleted_at IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(live, 1, "清空 is about the trash, not the Inbox");
    }

    #[test]
    fn the_retention_sweep_deletes_only_what_is_past_the_window() {
        let mut store = store("expire");
        item(&store, "old", Some(LONG_AGO));
        item(&store, "fresh", Some(NOW));
        item(&store, "live", None);
        let file = attachment(&store, "a1", "old", "old.png");

        assert_eq!(store.purge_expired_inbox(NOW, 30).expect("sweep"), 1);
        assert!(!file.exists(), "an expired attachment's file goes too");
        let left: Vec<String> = {
            let mut stmt = store
                .connection()
                .prepare("SELECT id FROM inbox_items ORDER BY id")
                .expect("prepare");
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .expect("query");
            rows.map(|row| row.expect("row")).collect()
        };
        assert_eq!(left, ["fresh", "live"]);
    }

    #[test]
    fn an_unreadable_now_or_a_zero_window_keeps_everything() {
        let mut store = store("refuse");
        item(&store, "old", Some(LONG_AGO));
        assert_eq!(store.purge_expired_inbox("not a date", 30).expect("sweep"), 0);
        assert_eq!(store.purge_expired_inbox(NOW, 0).expect("sweep"), 0);
        assert_eq!(store.trashed_inbox().expect("list").len(), 1);
    }
}
