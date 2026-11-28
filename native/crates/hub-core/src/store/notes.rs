//! Note rows, read and written exactly the way the Tauri build does.
//!
//! The column list is written out on every statement on purpose. The live table's
//! physical column order is `id,title,content,created_at,updated_at,is_pinned,tags,
//! deleted_at` because `tags` and `deleted_at` were added by `ALTER TABLE`, which
//! appends. `INSERT INTO notes VALUES (...)` would therefore land `tags` in the
//! `created_at` slot on a database migrated from a newer build -- a corruption that
//! only shows up as garbled timestamps much later.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{Result, Store};

/// One row of `notes`, including soft-deleted ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_pinned: bool,
    pub deleted_at: Option<String>,
}

impl NoteRow {
    const COLUMNS: &'static str =
        "id, title, content, tags, created_at, updated_at, is_pinned, deleted_at";

    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            tags: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
            is_pinned: row.get::<_, i64>(6)? != 0,
            deleted_at: row.get(7)?,
        })
    }
}

/// The body of a note, ready to write.
///
/// `is_pinned` and `deleted_at` are deliberately **not** here. They belong to the
/// manager -- the list pin and the trash -- and a quick-note autosave that carried
/// them would overwrite both with whatever the note window happens to think.
pub struct NoteWrite {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
}

impl Store {
    /// Insert or update a note's text. Returns `true` when the row was created.
    ///
    /// Two columns are left untouched on purpose, and both are load-bearing:
    ///
    /// * `is_pinned` is the **list** pin that `ORDER BY is_pinned DESC` sorts on.
    ///   The quick note's toolbar bookmark toggles *window z-order* (parity Q9) --
    ///   two features that share a name and nothing else. Writing the window state
    ///   here would make every note typed in the shell jump to the top of the
    ///   manager's list, and the symptom ("the list reordered itself") would be
    ///   blamed on the sort.
    /// * `deleted_at` keeps a trashed note trashed. An autosave from a window that
    ///   was open before the delete must not resurrect the row.
    ///
    /// `created_at` is not updated either: it is the note's identity in time, and
    /// every save would otherwise rewrite it.
    pub fn save_note_body(&mut self, write: &NoteWrite) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Inside the transaction, after the write lock: a `SELECT` beforehand could
        // see "absent" and then lose the race to another process's insert.
        let created = tx
            .query_row("SELECT 1 FROM notes WHERE id = ?1", [&write.id], |_| Ok(()))
            .optional()?
            .is_none();
        tx.execute(
            "INSERT INTO notes (id, title, content, tags, created_at, updated_at, is_pinned, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 content = excluded.content,
                 tags = excluded.tags,
                 updated_at = excluded.updated_at",
            params![
                write.id,
                write.title,
                write.content,
                write.tags,
                write.created_at,
                write.updated_at,
            ],
        )?;
        tx.commit()?;
        Ok(created)
    }
}

impl Store {
    /// Every note, ordered the way the product lists them.
    ///
    /// `include_deleted` exists because the trash view needs the soft-deleted rows;
    /// the default list must not show them.
    pub fn notes(&self, include_deleted: bool) -> Result<Vec<NoteRow>> {
        let filter = if include_deleted {
            ""
        } else {
            "WHERE deleted_at IS NULL"
        };
        let sql = format!(
            "SELECT {} FROM notes {filter} ORDER BY is_pinned DESC, updated_at DESC",
            NoteRow::COLUMNS
        );
        let mut stmt = self.connection().prepare(&sql)?;
        let rows = stmt.query_map([], NoteRow::from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn note(&self, id: &str) -> Result<Option<NoteRow>> {
        let sql = format!("SELECT {} FROM notes WHERE id = ?1", NoteRow::COLUMNS);
        Ok(self
            .connection()
            .query_row(&sql, [id], NoteRow::from_row)
            .optional()?)
    }

    /// Insert or update by primary key.
    ///
    /// Returns `true` when a row was created. No compare-and-swap here: the spike's
    /// question is whether the file stays compatible, and adding CAS would make a
    /// failing write look like a conflict. The product's CAS lives in `save_note_*`
    /// and is not being ported in this phase.
    pub fn upsert_note(&mut self, note: &NoteRow) -> Result<bool> {
        let inserted = self.note(&note.id)?.is_none();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO notes (id, title, content, tags, created_at, updated_at, is_pinned, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 content = excluded.content,
                 tags = excluded.tags,
                 updated_at = excluded.updated_at,
                 is_pinned = excluded.is_pinned,
                 deleted_at = excluded.deleted_at",
            params![
                note.id,
                note.title,
                note.content,
                note.tags,
                note.created_at,
                note.updated_at,
                note.is_pinned as i64,
                note.deleted_at,
            ],
        )?;
        tx.commit()?;
        Ok(inserted)
    }

    /// Soft delete: sets `deleted_at`, keeps the row and its content.
    ///
    /// The trash view depends on being able to restore, so a hard `DELETE` here
    /// would be a product regression rather than a shortcut.
    pub fn soft_delete_note(&mut self, id: &str, at: &str) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE notes SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, at],
        )?;
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Set or clear the *list* pin (not the window's z-order bookmark).
    ///
    /// The manager is the only writer: a quick note deliberately never touches this
    /// column, because its bookmark means "keep this window on top" and writing it
    /// here would reorder the list every time a note was opened.
    pub fn set_note_pin(&mut self, id: &str, pinned: bool) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE notes SET is_pinned = ?2 WHERE id = ?1",
            params![id, pinned as i64],
        )?;
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Take a note out of the trash.
    pub fn restore_note(&mut self, id: &str) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute("UPDATE notes SET deleted_at = NULL WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Delete the row for good.
    ///
    /// Only reachable from the trash, where the user asked for it twice: the
    /// shipped product keeps deletes reversible until this button, and a
    /// `DELETE` anywhere else would be a data loss the UI never warned about.
    pub fn purge_note(&mut self, id: &str) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute("DELETE FROM notes WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Take many notes out of the trash in one transaction.
    ///
    /// Returns how many rows actually changed, not how many were asked for: the
    /// list a click was based on is one frame old, and a row another window already
    /// restored must not be counted as restored again. The `deleted_at IS NOT NULL`
    /// guard is what makes the count mean "was in the trash, is not now" -- without
    /// it a live note's `deleted_at` would be set to `NULL` (a no-op) and still be
    /// counted, so "已恢复 3 条" could name a row that was never in the trash.
    pub fn restore_notes(&mut self, ids: &[String]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut changed = 0;
        {
            let mut statement =
                tx.prepare("UPDATE notes SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL")?;
            for id in ids {
                changed += statement.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Delete every row that is in the trash. 清空回收站.
    ///
    /// No id list and no selection: the trash is the set, and the dialog that asks
    /// for this says how big it is. Deleting a `WHERE`-clause's worth of rows is
    /// still one transaction, so a crash halfway leaves the trash as it was.
    pub fn purge_trashed(&mut self) -> Result<usize> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute("DELETE FROM notes WHERE deleted_at IS NOT NULL", [])?;
        tx.commit()?;
        Ok(changed)
    }

    /// Delete what has been in the trash longer than the retention window.
    ///
    /// The panel has been saying "保留 30 天" since it grew a description, and until
    /// now nothing was doing it -- a trash that only grows is a disk leak with a
    /// reassuring sentence over it. Comparing stamps **as text** is sound only
    /// because [`crate::clock`] writes one fixed-width UTC shape and the Tauri build
    /// writes the same one (`to_rfc3339`, `+00:00`, nine digits); a row with an
    /// unreadable stamp sorts *below* a cutoff in that shape, i.e. it is kept, which
    /// is the direction that costs nothing.
    ///
    /// `now` is passed in rather than read here so the cutoff is testable: a
    /// function that proves "40 days is gone, 20 is kept" by waiting is not a test.
    pub fn purge_expired_trash(&mut self, now: &str, days: i64) -> Result<usize> {
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
        let changed = tx.execute(
            "DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
            [cutoff],
        )?;
        tx.commit()?;
        Ok(changed)
    }

    /// The note that holds the todo list.
    ///
    /// Todos are **not a table**. The product stores them as Markdown checkbox lines
    /// inside a note titled `待办箱`, located by trimmed title and newest update.
    /// Building a table for them would be a contract change, so this mirrors the
    /// lookup instead.
    pub fn todo_inbox(&self) -> Result<Option<NoteRow>> {
        let sql = format!(
            "SELECT {} FROM notes
             WHERE deleted_at IS NULL AND TRIM(title) = ?1
             ORDER BY updated_at DESC, id ASC
             LIMIT 1",
            NoteRow::COLUMNS
        );
        Ok(self
            .connection()
            .query_row(&sql, [TODO_INBOX_TITLE], NoteRow::from_row)
            .optional()?)
    }

    /// Number of Markdown checkbox lines in a body.
    ///
    /// Reported, not gated: the authoritative parser is the frontend's
    /// `todoParser`. This exists so the record can state "the note was found and
    /// contains N lines that look like todos" instead of just "found".
    pub fn count_todo_lines(content: &str) -> (usize, usize) {
        let mut open = 0;
        let mut done = 0;
        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("- [ ]") || trimmed.starts_with("* [ ]") {
                open += 1;
            } else if trimmed.starts_with("- [x]")
                || trimmed.starts_with("* [x]")
                || trimmed.starts_with("- [X]")
                || trimmed.starts_with("* [X]")
            {
                done += 1;
            }
        }
        (open, done)
    }
}

pub const TODO_INBOX_TITLE: &str = "待办箱";

/// How long a deleted note stays recoverable.
///
/// One number, in one place, because two things read it: the sweep that actually
/// deletes ([`Store::purge_expired_trash`]) and the sentence in the trash panel that
/// promises it. A panel that says 30 while the sweep uses 14 is worse than no
/// sentence at all.
pub const TRASH_RETENTION_DAYS: i64 = 30;

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

    fn note(id: &str, content: &str) -> NoteRow {
        NoteRow {
            id: id.to_string(),
            title: String::new(),
            content: content.to_string(),
            tags: String::new(),
            created_at: "2026-11-25T00:00:00+00:00".to_string(),
            updated_at: "2026-11-25T00:00:00+00:00".to_string(),
            is_pinned: false,
            deleted_at: None,
        }
    }

    #[test]
    fn an_upsert_creates_then_updates_and_reports_which() {
        let mut store = store();
        assert!(store.upsert_note(&note("n1", "first")).expect("create"));

        let mut updated = note("n1", "second");
        updated.updated_at = "2026-11-25T01:00:00+00:00".to_string();
        assert!(!store.upsert_note(&updated).expect("update"));

        let stored = store.note("n1").expect("read").expect("present");
        assert_eq!(stored.content, "second");
        assert_eq!(stored.updated_at, "2026-11-25T01:00:00+00:00");
    }

    #[test]
    fn cjk_content_survives_the_round_trip_unchanged() {
        let mut store = store();
        let body = "天地玄黄，宇宙洪荒。\n\t第二行 with ASCII and emoji 🐟";
        store.upsert_note(&note("n1", body)).expect("write");
        let stored = store.note("n1").expect("read").expect("present");
        assert_eq!(stored.content, body);
        assert_eq!(stored.content.as_bytes(), body.as_bytes());
    }

    #[test]
    fn soft_deleted_notes_leave_the_list_but_stay_readable() {
        let mut store = store();
        store.upsert_note(&note("n1", "keep")).expect("write");
        store.upsert_note(&note("n2", "trash me")).expect("write");
        assert!(store.soft_delete_note("n2", "2026-11-25T02:00:00+00:00").expect("delete"));

        assert_eq!(store.notes(false).expect("list").len(), 1);
        assert_eq!(store.notes(true).expect("all").len(), 2);

        let trashed = store.note("n2").expect("read").expect("present");
        assert_eq!(trashed.content, "trash me");
        assert!(trashed.deleted_at.is_some());
    }

    #[test]
    fn pinned_notes_sort_first_then_by_update_time() {
        let mut store = store();
        store.upsert_note(&note("old", "a")).expect("write");
        let mut pinned = note("pin", "b");
        pinned.is_pinned = true;
        pinned.updated_at = "2020-01-01T00:00:00+00:00".to_string();
        store.upsert_note(&pinned).expect("write");
        let mut newest = note("new", "c");
        newest.updated_at = "2026-11-25T03:00:00+00:00".to_string();
        store.upsert_note(&newest).expect("write");

        let ids: Vec<String> = store
            .notes(false)
            .expect("list")
            .into_iter()
            .map(|n| n.id)
            .collect();
        assert_eq!(ids, ["pin", "new", "old"]);
    }

    #[test]
    fn the_todo_inbox_is_found_by_trimmed_title_and_newest_update() {
        let mut store = store();
        let mut older = note("a", "- [ ] one");
        older.title = "待办箱".to_string();
        older.updated_at = "2026-11-20T00:00:00+00:00".to_string();
        store.upsert_note(&older).expect("write");

        let mut newer = note("b", "- [x] done\n- [ ] open");
        newer.title = " 待办箱 ".to_string();
        newer.updated_at = "2026-11-24T00:00:00+00:00".to_string();
        store.upsert_note(&newer).expect("write");

        let inbox = store.todo_inbox().expect("lookup").expect("found");
        assert_eq!(inbox.id, "b");
        assert_eq!(Store::count_todo_lines(&inbox.content), (1, 1));
    }

    #[test]
    fn a_soft_deleted_todo_inbox_is_not_returned() {
        let mut store = store();
        let mut inbox = note("a", "- [ ] one");
        inbox.title = "待办箱".to_string();
        store.upsert_note(&inbox).expect("write");
        store.soft_delete_note("a", "2026-11-25T00:00:00+00:00").expect("delete");
        assert!(store.todo_inbox().expect("lookup").is_none());
    }

    #[test]
    fn batch_restore_reports_what_it_actually_changed() {
        // The count is what the panel says out loud ("已恢复 2 条"), so it has to mean
        // rows that were in the trash. A live note in the list must not be counted --
        // its `deleted_at` is already NULL and setting it to NULL changes nothing.
        let mut store = store();
        store.upsert_note(&note("a", "gone")).expect("write");
        store.upsert_note(&note("b", "gone too")).expect("write");
        store.upsert_note(&note("c", "still here")).expect("write");
        store.soft_delete_note("a", "2026-11-25T02:00:00+00:00").expect("delete");
        store.soft_delete_note("b", "2026-11-25T02:00:00+00:00").expect("delete");

        let asked = ["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(store.restore_notes(&asked).expect("restore"), 2);
        assert_eq!(store.notes(true).expect("all").len(), 3);
        assert!(store.notes(false).expect("live").iter().all(|row| row.deleted_at.is_none()));

        // Second call: nothing left to restore, and the count says so rather than
        // reporting the size of the request.
        assert_eq!(store.restore_notes(&asked).expect("again"), 0);
        assert_eq!(store.restore_notes(&[]).expect("empty"), 0);
    }

    #[test]
    fn empty_trash_takes_every_trashed_row_and_no_live_one() {
        let mut store = store();
        store.upsert_note(&note("live", "keep me")).expect("write");
        for id in ["t1", "t2", "t3"] {
            store.upsert_note(&note(id, "trash")).expect("write");
            store.soft_delete_note(id, "2026-11-25T02:00:00+00:00").expect("delete");
        }
        assert_eq!(store.purge_trashed().expect("purge"), 3);
        assert_eq!(store.notes(true).expect("all").len(), 1);
        assert!(store.note("live").expect("read").is_some());
        assert_eq!(store.purge_trashed().expect("again"), 0);
    }

    #[test]
    fn the_retention_sweep_deletes_only_what_is_past_the_window() {
        // "保留 30 天" was a sentence with nothing behind it. The cutoff is text, so
        // the test uses the same shape the writer produces (`+00:00`, nine digits).
        let mut store = store();
        store.upsert_note(&note("stale", "40 days in the trash")).expect("write");
        store.upsert_note(&note("fresh", "20 days in the trash")).expect("write");
        store.upsert_note(&note("live", "never deleted")).expect("write");
        store
            .soft_delete_note("stale", "2026-10-19T00:00:00.000000000+00:00")
            .expect("delete");
        store
            .soft_delete_note("fresh", "2026-11-08T00:00:00.000000000+00:00")
            .expect("delete");

        let purged = store
            .purge_expired_trash("2026-11-28T00:00:00.000000000+00:00", TRASH_RETENTION_DAYS)
            .expect("sweep");
        assert_eq!(purged, 1, "only the row past 30 days leaves");
        assert!(store.note("stale").expect("read").is_none());
        assert!(store.note("fresh").expect("read").is_some(), "recent trash stays");
        assert!(store.note("live").expect("read").is_some(), "the list is not the trash");

        // A zero or negative window would mean "delete everything immediately"; it is
        // refused rather than honoured, because that setting does not exist yet.
        assert_eq!(
            store
                .purge_expired_trash("2026-11-28T00:00:00.000000000+00:00", 0)
                .expect("zero"),
            0
        );
        assert!(store.note("fresh").expect("read").is_some());
    }

    #[test]
    fn an_unreadable_now_keeps_everything() {
        // The sweep is destructive and runs on a tab click, so a clock it cannot read
        // must be a no-op -- the alternative is deleting rows because `now` parsed as
        // the epoch.
        let mut store = store();
        store.upsert_note(&note("t", "trash")).expect("write");
        store
            .soft_delete_note("t", "2026-11-25T00:00:00.000000000+00:00")
            .expect("delete");
        assert_eq!(store.purge_expired_trash("not a date", 30).expect("sweep"), 0);
        assert!(store.note("t").expect("read").is_some());
    }

    fn write(id: &str, content: &str, updated_at: &str) -> NoteWrite {
        NoteWrite {
            id: id.to_string(),
            title: format!("title of {id}"),
            content: content.to_string(),
            tags: String::new(),
            created_at: "2026-11-25T00:00:00+00:00".to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn a_body_save_creates_then_updates_and_reports_which() {
        let mut store = store();
        assert!(store.save_note_body(&write("n1", "first", "2026-11-25T00:00:00+00:00")).expect("create"));
        assert!(!store.save_note_body(&write("n1", "second", "2026-11-25T01:00:00+00:00")).expect("update"));

        let stored = store.note("n1").expect("read").expect("present");
        assert_eq!(stored.content, "second");
        assert_eq!(stored.updated_at, "2026-11-25T01:00:00+00:00");
        // Only the body columns move; identity stays put.
        assert_eq!(stored.created_at, "2026-11-25T00:00:00+00:00");
    }

    #[test]
    fn a_body_save_never_moves_the_list_pin() {
        // The regression this guards: the quick note's bookmark is window z-order,
        // and `is_pinned` is what `ORDER BY is_pinned DESC` sorts on. A save that
        // wrote the window state would silently reorder the manager's list.
        let mut store = store();
        let mut pinned = note("n1", "body");
        pinned.is_pinned = true;
        store.upsert_note(&pinned).expect("seed pinned row");

        store
            .save_note_body(&write("n1", "edited in the note window", "2026-11-25T01:00:00+00:00"))
            .expect("save");

        let stored = store.note("n1").expect("read").expect("present");
        assert!(stored.is_pinned, "the list pin was cleared by an autosave");
        assert_eq!(stored.content, "edited in the note window");
    }

    #[test]
    fn the_manager_can_pin_restore_and_purge_without_touching_the_body() {
        let mut store = store();
        store.upsert_note(&note("n1", "body")).expect("seed");

        assert!(store.set_note_pin("n1", true).expect("pin"));
        assert!(store.note("n1").expect("read").expect("present").is_pinned);
        assert!(store.set_note_pin("n1", false).expect("unpin"));
        assert!(!store.note("n1").expect("read").expect("present").is_pinned);

        // The pin is a list property: the text must not move with it.
        assert_eq!(store.note("n1").expect("read").expect("present").content, "body");

        assert!(store.soft_delete_note("n1", "2026-11-26T00:00:00+00:00").expect("trash"));
        assert_eq!(store.notes(false).expect("list").len(), 0);
        assert!(store.restore_note("n1").expect("restore"));
        assert_eq!(store.notes(false).expect("list").len(), 1);

        assert!(store.purge_note("n1").expect("purge"));
        assert!(store.note("n1").expect("read").is_none(), "the row must be gone");
    }

    #[test]
    fn a_body_save_does_not_resurrect_a_trashed_note() {
        let mut store = store();
        store.save_note_body(&write("n1", "before", "2026-11-25T00:00:00+00:00")).expect("create");
        store.soft_delete_note("n1", "2026-11-25T02:00:00+00:00").expect("trash");

        // A window that was still open saves after the delete: the text updates it,
        // but the row stays in the trash.
        store
            .save_note_body(&write("n1", "typed after the delete", "2026-11-25T03:00:00+00:00"))
            .expect("late save");

        assert_eq!(store.notes(false).expect("list").len(), 0, "it came back to the list");
        let stored = store.note("n1").expect("read").expect("present");
        assert!(stored.deleted_at.is_some(), "the trash marker was cleared");
        assert_eq!(stored.content, "typed after the delete");
    }
}
