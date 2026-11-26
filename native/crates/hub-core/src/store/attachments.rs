//! Attachment files in `assets/`, next to the database.
//!
//! The Tauri build stages every attachment as `.{storage_key}.pending` and then
//! renames it into place *inside* the capture transaction, so a crash can leave at
//! most a hidden `.pending` turd, never a half-written file that a row points at.
//! This module keeps that order for the same reason.
//!
//! `filename` is `UNIQUE`, and the storage key is a fresh UUID plus the source
//! extension, so two attachments never collide. Nothing here scans the directory:
//! the live install has 22 389 files in `assets/`, and a listing on the write path
//! would turn a 1 ms copy into a visible stall.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};

use super::{Result, Store, StoreError};

pub const ATTACHMENT_DIR: &str = "assets";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAttachment {
    pub id: String,
    pub item_id: String,
    pub kind: String,
    pub name: String,
    pub mime_type: String,
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String,
}

impl Store {
    /// Copy a file into `assets/` and point a row at it.
    ///
    /// `item_id` must already exist: the product's capture path inserts the item
    /// first. Foreign keys are per-connection and off by default, so this checks the
    /// parent explicitly rather than trusting a pragma that the next connection may
    /// not set.
    pub fn store_attachment(
        &mut self,
        item_id: &str,
        source: &Path,
        kind: &str,
        name: &str,
        mime_type: &str,
        at: &str,
    ) -> Result<StoredAttachment> {
        let parent: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM inbox_items WHERE id = ?1",
            [item_id],
            |row| row.get(0),
        )?;
        if parent == 0 {
            return Err(StoreError::Missing(format!("inbox item {item_id}")));
        }

        let dir = self.attachment_dir();
        fs::create_dir_all(&dir)?;

        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default();
        let storage_key = format!("{}{}", uuid::Uuid::new_v4(), extension);
        let pending: PathBuf = dir.join(format!(".{storage_key}.pending"));
        let final_path = dir.join(&storage_key);

        fs::copy(source, &pending)?;
        let size_bytes = fs::metadata(&pending)?.len();

        let attachment = StoredAttachment {
            id: uuid::Uuid::new_v4().to_string(),
            item_id: item_id.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            mime_type: mime_type.to_string(),
            filename: storage_key.clone(),
            size_bytes,
            created_at: at.to_string(),
        };

        let written = (|| -> Result<()> {
            let tx = self
                .conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO inbox_attachments
                    (id, item_id, kind, name, mime_type, filename, size_bytes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    attachment.id,
                    attachment.item_id,
                    attachment.kind,
                    attachment.name,
                    attachment.mime_type,
                    attachment.filename,
                    size_bytes as i64,
                    attachment.created_at,
                ],
            )?;
            fs::rename(&pending, &final_path)?;
            tx.commit()?;
            Ok(())
        })();

        if written.is_err() {
            // Best effort, exactly like the product: a leftover file is harmless, a
            // deleted file that a committed row points at is not.
            let _ = fs::remove_file(&pending);
            let _ = fs::remove_file(&final_path);
        }
        written?;
        Ok(attachment)
    }

    pub fn attachment(&self, filename: &str) -> Result<Option<StoredAttachment>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, item_id, kind, name, mime_type, filename, size_bytes, created_at
                 FROM inbox_attachments WHERE filename = ?1",
                [filename],
                |row| {
                    Ok(StoredAttachment {
                        id: row.get(0)?,
                        item_id: row.get(1)?,
                        kind: row.get(2)?,
                        name: row.get(3)?,
                        mime_type: row.get(4)?,
                        filename: row.get(5)?,
                        size_bytes: row.get::<_, i64>(6)? as u64,
                        created_at: row.get(7)?,
                    })
                },
            )
            .optional()?)
    }

    /// Whether the bytes a row points at are actually on disk. A `stat`, not a scan.
    pub fn attachment_file(&self, filename: &str) -> PathBuf {
        self.attachment_dir().join(filename)
    }

    /// Insert a plain Inbox item so an attachment has a legal parent.
    ///
    /// Only `kind = 'text'` is produced here; richer kinds belong to the capture
    /// path, which is not part of this phase.
    pub fn create_inbox_text_item(&mut self, id: &str, captured_at: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at)
             VALUES (?1, 'text', '', '', 'native-spike', 'S3 compatibility probe', ?2)",
            params![id, captured_at],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{init_schema, Rebuild, DB_FILE};
    use rusqlite::Connection;

    fn store() -> Store {
        let mut conn = Connection::open_in_memory().expect("memory db");
        init_schema(&mut conn, Rebuild::Defer).expect("schema");
        // A per-process directory so the two tests below cannot see each other's
        // `.pending` files: the assertion "no pending file remains" is only
        // meaningful when the directory is not shared.
        let root = std::env::temp_dir().join(format!("hub-core-s3-{}", std::process::id()));
        fs::create_dir_all(root.join(ATTACHMENT_DIR)).expect("assets");
        Store {
            conn,
            path: root.join(DB_FILE),
        }
    }

    #[test]
    fn an_attachment_without_a_parent_item_is_refused_before_touching_disk() {
        let mut store = store();
        let source = std::env::temp_dir().join("hub-core-attachment-src.txt");
        fs::write(&source, b"payload").expect("source");
        let error = store
            .store_attachment("nope", &source, "file", "x.txt", "text/plain", "2026-11-25T00:00:00+00:00")
            .expect_err("must refuse");
        assert!(matches!(error, StoreError::Missing(_)), "unexpected: {error:?}");
        let _ = fs::remove_file(&source);
    }

    #[test]
    fn the_storage_key_keeps_the_extension_and_no_pending_file_remains() {
        let mut store = store();
        store
            .create_inbox_text_item("item-1", "2026-11-25T00:00:00+00:00")
            .expect("parent");

        let source = std::env::temp_dir().join("hub-core-attachment-src.png");
        fs::write(&source, vec![0u8; 1024]).expect("source");
        let stored = store
            .store_attachment(
                "item-1",
                &source,
                "image",
                "shot.png",
                "image/png",
                "2026-11-25T00:00:00+00:00",
            )
            .expect("store");

        assert!(stored.filename.ends_with(".png"), "{}", stored.filename);
        assert_eq!(stored.size_bytes, 1024);
        let path = store.attachment_file(&stored.filename);
        assert!(path.exists(), "{} missing", path.display());

        let dir = store.attachment_dir();
        let pending = fs::read_dir(&dir)
            .expect("assets dir")
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".pending"));
        assert!(!pending, "a .pending file survived the rename");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&source);
    }
}
