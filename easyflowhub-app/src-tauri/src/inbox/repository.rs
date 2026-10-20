//! Inbox repository: every read/write of the `inbox_items` and
//! `inbox_attachments` rows goes through this module, so resource identity and
//! query behaviour have exactly one owner. Multi-statement transactions are
//! orchestrated by `service.rs` on the connections opened here.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use std::path::PathBuf;

use super::attachments::InboxAttachment;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub source: String,
    pub context: String,
    pub captured_at: String,
    pub archived_at: Option<String>,
    pub converted_note_id: Option<String>,
    pub attachments: Vec<InboxAttachment>,
}

/// Open the shared notes database connection with the Inbox schema applied.
pub(crate) fn get_connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    let mut conn = crate::notes::get_connection(app)?;
    super::schema::init_db(&mut conn)
        .map_err(|error| format!("Failed to initialize inbox storage: {error}"))?;
    Ok(conn)
}

/// Internal accessor for sibling native owners (transcription) that read or
/// extend Inbox data through this module's controlled helpers.
pub(crate) fn inbox_connection(app: &tauri::AppHandle) -> Result<Connection, String> {
    get_connection(app)
}

/// The shared resource directory holding Inbox / note attachments.
pub(crate) fn assets_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    crate::notes::assets_path(app)
}

fn map_attachment(row: &rusqlite::Row<'_>) -> SqliteResult<InboxAttachment> {
    Ok(InboxAttachment {
        id: row.get(0)?,
        item_id: row.get(1)?,
        kind: row.get(2)?,
        name: row.get(3)?,
        mime_type: row.get(4)?,
        filename: row.get(5)?,
        size_bytes: row.get::<_, i64>(6)?.max(0) as u64,
        created_at: row.get(7)?,
        source_path: row.get(8)?,
    })
}

fn list_attachments(conn: &Connection, item_id: &str) -> Result<Vec<InboxAttachment>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, item_id, kind, name, mime_type, filename, size_bytes, created_at, source_path
            FROM inbox_attachments
            WHERE item_id = ?
            ORDER BY created_at, id
            "#,
        )
        .map_err(|error| format!("Failed to prepare inbox attachments: {error}"))?;
    let rows = stmt
        .query_map([item_id], map_attachment)
        .map_err(|error| format!("Failed to query inbox attachments: {error}"))?
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|error| format!("Failed to collect inbox attachments: {error}"))?;
    Ok(rows)
}

pub(crate) fn get_item(conn: &Connection, id: &str) -> Result<Option<InboxItem>, String> {
    let mut item = conn
        .query_row(
            r#"
            SELECT id, kind, title, content, source, context, captured_at, archived_at, converted_note_id
            FROM inbox_items WHERE id = ?
            "#,
            [id],
            |row| {
                Ok(InboxItem {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    source: row.get(4)?,
                    context: row.get(5)?,
                    captured_at: row.get(6)?,
                    archived_at: row.get(7)?,
                    converted_note_id: row.get(8)?,
                    attachments: Vec::new(),
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to get inbox item: {error}"))?;

    if let Some(item) = &mut item {
        item.attachments = list_attachments(conn, &item.id)?;
    }
    Ok(item)
}

/// List inbox items, optionally including archived ones, each with its
/// attachments attached.
pub(crate) fn list_items(
    conn: &Connection,
    include_archived: bool,
) -> Result<Vec<InboxItem>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, kind, title, content, source, context, captured_at, archived_at, converted_note_id
            FROM inbox_items
            WHERE (? = 1 OR archived_at IS NULL)
            ORDER BY captured_at DESC, id DESC
            "#,
        )
        .map_err(|error| format!("Failed to prepare inbox list: {error}"))?;
    let rows = stmt
        .query_map([include_archived as i32], |row| {
            Ok(InboxItem {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                source: row.get(4)?,
                context: row.get(5)?,
                captured_at: row.get(6)?,
                archived_at: row.get(7)?,
                converted_note_id: row.get(8)?,
                attachments: Vec::new(),
            })
        })
        .map_err(|error| format!("Failed to query inbox list: {error}"))?;
    let mut items = rows
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|error| format!("Failed to collect inbox list: {error}"))?;
    drop(stmt);
    for item in &mut items {
        item.attachments = list_attachments(conn, &item.id)?;
    }
    Ok(items)
}

pub(crate) fn insert_item_row(
    conn: &Connection,
    id: &str,
    kind: &str,
    title: &str,
    content: &str,
    source: &str,
    context: &str,
    captured_at: &str,
) -> SqliteResult<()> {
    conn.execute(
        r#"
        INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
        params![id, kind, title, content, source, context, captured_at],
    )?;
    Ok(())
}

/// Set or clear the archive marker of an item.
pub(crate) fn set_archived(conn: &Connection, id: &str, archived: bool) -> Result<(), String> {
    let archived_at = archived.then(|| Utc::now().to_rfc3339());
    conn.execute(
        "UPDATE inbox_items SET archived_at = ? WHERE id = ?",
        params![archived_at, id],
    )
    .map_err(|error| format!("Failed to update inbox item: {error}"))?;
    Ok(())
}

/// Storage filename of a single attachment, for opening it from disk.
pub(crate) fn attachment_filename(conn: &Connection, id: &str) -> Result<String, String> {
    conn.query_row("SELECT filename FROM inbox_attachments WHERE id = ?", [id], |row| row.get(0))
        .map_err(|error| format!("无法找到附件: {error}"))
}

/// Link target of an item, only when it is stored as a link capture.
pub(crate) fn item_link_content(conn: &Connection, id: &str) -> Result<String, String> {
    conn.query_row(
        "SELECT content FROM inbox_items WHERE id = ? AND kind = 'link'",
        [id],
        |row| row.get(0),
    )
    .map_err(|error| format!("无法找到链接: {error}"))
}

/// Note id the item was already converted to, if any.
pub(crate) fn item_converted_note_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT converted_note_id FROM inbox_items WHERE id = ?",
        [id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("无法找到收集项: {error}"))
    .map(|value| value.flatten())
}

/// Stored attachment filenames belonging to an item.
pub(crate) fn item_attachment_filenames(
    conn: &Connection,
    item_id: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT filename FROM inbox_attachments WHERE item_id = ? ORDER BY created_at, id")
        .map_err(|error| format!("Failed to prepare attachment query: {error}"))?;
    let rows = stmt
        .query_map([item_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query attachments: {error}"))?;
    rows.collect::<SqliteResult<Vec<_>>>()
        .map_err(|error| format!("Failed to collect attachments: {error}"))
}

/// Original source paths recorded for an item's attachments (F10).
///
/// A voice recording is captured from `audio/{session-id}.wav` but archived in
/// `assets/` under a fresh UUID, so the assets filename alone can never locate
/// the recorder's original. This returns the recorded absolute source paths so
/// cleanup can remove the originals too.
pub(crate) fn item_attachment_source_paths(
    conn: &Connection,
    item_id: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT source_path FROM inbox_attachments \
             WHERE item_id = ? AND source_path IS NOT NULL \
             ORDER BY created_at, id",
        )
        .map_err(|error| format!("Failed to prepare source path query: {error}"))?;
    let rows = stmt
        .query_map([item_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query source paths: {error}"))?;
    rows.collect::<SqliteResult<Vec<_>>>()
        .map_err(|error| format!("Failed to collect source paths: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::schema::test_memory_db;

    #[test]
    fn captured_items_keep_source_context_and_can_be_archived_and_restored() {
        let conn = test_memory_db();
        insert_item_row(
            &conn,
            "item-1",
            "text",
            "A note",
            "Remember this",
            "launcher",
            "clipboard",
            "2026-10-17T10:00:00Z",
        )
        .expect("insert item");
        conn.execute(
            "UPDATE inbox_items SET archived_at = '2026-10-17T11:00:00Z' WHERE id = 'item-1'",
            [],
        )
        .expect("archive item");
        let archived_at: Option<String> = conn
            .query_row("SELECT archived_at FROM inbox_items WHERE id = 'item-1'", [], |row| row.get(0))
            .expect("archived state");
        assert_eq!(archived_at.as_deref(), Some("2026-10-17T11:00:00Z"));
        conn.execute("UPDATE inbox_items SET archived_at = NULL WHERE id = 'item-1'", [])
            .expect("restore item");
        let (source, context, archived): (String, String, Option<String>) = conn
            .query_row(
                "SELECT source, context, archived_at FROM inbox_items WHERE id = 'item-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("restored item");
        assert_eq!(source, "launcher");
        assert_eq!(context, "clipboard");
        assert!(archived.is_none());
    }

    #[test]
    fn item_attachment_filenames_only_returns_the_item_files() {
        let conn = test_memory_db();
        for item in ["item-a", "item-b"] {
            conn.execute(
                "INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at) VALUES (?, 'audio', '', '', 'manager', '', '2026-10-19T09:00:00Z')",
                [item],
            )
            .expect("insert item");
        }
        for (attachment, item, filename) in [
            ("att-1", "item-a", "item-a.wav"),
            ("att-2", "item-a", "item-a-transcript.txt"),
            ("att-3", "item-b", "item-b.wav"),
        ] {
            conn.execute(
                "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at) VALUES (?, ?, 'audio', ?, 'audio/wav', ?, 10, '2026-10-19T09:00:00Z')",
                params![attachment, item, filename, filename],
            )
            .expect("insert attachment");
        }

        let mut filenames = item_attachment_filenames(&conn, "item-a").expect("list filenames");
        filenames.sort();
        assert_eq!(filenames, vec!["item-a-transcript.txt", "item-a.wav"]);
    }
}
