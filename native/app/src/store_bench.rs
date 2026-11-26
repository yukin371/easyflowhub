//! S3 evidence: read and write the product's `notes.db` and say exactly what came
//! back.
//!
//! Read-only unless `--store-write` is passed, and `--store-write` refuses to touch
//! the default app data directory: the point is to prove compatibility on a *copy*,
//! and a probe row landing in the user's real database is the one mistake this mode
//! must not be able to make. The gate script copies `notes.db` first, so refusing the
//! real path costs nothing.
//!
//! Timestamps in the probe rows are fixed constants (`2026-11-25T00:00:00+00:00`)
//! rather than `now()`. They are valid RFC3339, the product parses them, and a fixed
//! value means two runs of the gate produce byte-identical evidence instead of a
//! diff that has to be explained away.

use std::path::Path;

use hub_core::store::{default_root, NoteRow, Store};

/// Row the bench creates and then updates. Stable id so the Tauri-side test can look
/// for it by name instead of guessing.
pub const PROBE_CREATED: &str = "s3-probe-created";
/// Row the bench creates and soft-deletes.
pub const PROBE_DELETED: &str = "s3-probe-deleted";
/// Inbox item that owns the probe attachment.
pub const PROBE_ITEM: &str = "s3-probe-item";

const T0: &str = "2026-11-25T00:00:00+00:00";
const T1: &str = "2026-11-25T00:00:01+00:00";

/// Body used for the write probe. CJK plus an emoji plus a tab and a newline: the
/// four things that break naive encoding handling, in 40 bytes.
pub const PROBE_BODY: &str = "探针正文\t第二行\nemoji 🐟 与 ASCII";

pub struct Evidence {
    pub root: String,
    pub db: String,
    pub db_bytes: u64,
    pub sqlite_version: String,
    pub journal_mode: String,
    pub user_version: i64,
    pub page_size: i64,
    pub encoding: String,
    pub counts_before: CountsView,
    pub counts_after: CountsView,
    /// Which rungs of the schema ladder this file has. Measured, not stamped; see
    /// `hub_core::store::schema`.
    pub schema: SchemaView,
    /// Full `sqlite_master` dump before and after the write. Kept as text, not a
    /// hash, so a failure can be read as a diff.
    pub schema_before: String,
    pub schema_after: String,
    pub schema_same: bool,
    pub integrity_before: bool,
    pub integrity_after: bool,
    pub notes: NotesView,
    pub todo: TodoView,
    pub settings: SettingsView,
    pub writes: WriteView,
    pub dump: Vec<NoteRow>,
    pub pass: bool,
}

#[derive(Default, Clone)]
pub struct CountsView {
    pub notes: i64,
    pub notes_deleted: i64,
    pub settings: i64,
    pub inbox_items: i64,
    pub inbox_attachments: i64,
}

/// The schema report, flattened for JSON: `Schema` keeps `&'static str` rung names,
/// which is right for the library and awkward to serialise.
#[derive(Default, Clone)]
pub struct SchemaView {
    pub version: u32,
    pub expected: u32,
    pub missing: Vec<String>,
    pub ahead: Vec<String>,
    pub legacy_capture_kinds: bool,
}

impl SchemaView {
    fn of(schema: &hub_core::store::Schema) -> Self {
        Self {
            version: schema.version,
            expected: hub_core::store::SCHEMA_VERSION,
            missing: schema.missing.iter().map(|name| name.to_string()).collect(),
            ahead: schema.ahead.clone(),
            legacy_capture_kinds: schema.legacy_capture_kinds,
        }
    }

    fn current(&self) -> bool {
        self.version == self.expected && self.missing.is_empty()
    }
}

pub struct NotesView {
    pub live: usize,
    pub with_deleted: usize,
    pub pinned_first: bool,
    pub updated_desc_within_pinned: bool,
    pub sort_ok: bool,
}

pub struct TodoView {
    pub found: bool,
    pub id: String,
    pub open: usize,
    pub done: usize,
    pub content_bytes: usize,
}

pub struct SettingsView {
    pub keys: Vec<String>,
    pub parsed_objects: usize,
    pub parsed_scalars: usize,
    pub autostart: Option<bool>,
}

#[derive(Default)]
pub struct WriteView {
    pub created: bool,
    pub updated_at_changed: bool,
    pub soft_deleted: bool,
    pub attachment_filename: String,
    pub attachment_bytes: u64,
    pub attachment_row: bool,
}

pub fn run(root: Option<&Path>, artifacts: Option<&Path>, write: bool) -> i32 {
    let resolved = match root {
        Some(path) => path.to_path_buf(),
        None => match default_root() {
            Ok(path) => path,
            Err(error) => {
                eprintln!("store bench: cannot resolve the app data dir: {error}");
                return 1;
            }
        },
    };

    if write {
        // A probe row in the live database would be data corruption dressed up as
        // evidence. Refuse, loudly, instead of trusting the operator.
        if let Ok(default) = default_root() {
            if same_path(&resolved, &default) {
                eprintln!(
                    "store bench: refusing to write into the live app data dir ({}).\n\
                     Copy the database and pass --store-root <copy>.",
                    resolved.display()
                );
                return 1;
            }
        }
    }

    let mut store = match Store::open_in(&resolved) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("store bench: cannot open {}: {error}", resolved.display());
            return 1;
        }
    };

    if write {
        // Remove our own rows from a previous run first, so the counts delta is the
        // same on every run. Without this, running the gate twice on one copy turns
        // "created a row" into "updated a row" and the verdict flips for no reason.
        if let Err(error) = clear_probes(&mut store) {
            eprintln!("store bench: cannot clear earlier probe rows: {error}");
            return 1;
        }
    }

    let mut evidence = match capture(&mut store) {
        Ok(evidence) => evidence,
        Err(error) => {
            eprintln!("store bench: read failed: {error}");
            return 1;
        }
    };

    if write {
        match write_probes(&mut store) {
            Ok(writes) => evidence.writes = writes,
            Err(error) => {
                eprintln!("store bench: write failed: {error}");
                return 1;
            }
        }
        if let Err(error) = refresh_after_write(&mut store, &mut evidence) {
            eprintln!("store bench: post-write read failed: {error}");
            return 1;
        }
    }

    evidence.pass = judge(&evidence, write);

    println!("{}", report(&evidence));
    println!("---STORE-JSON---");
    // Without the row dump: 40 notes of CJK prose on stdout make a CI log
    // unreadable. The dump goes to the artifacts file, where the gate script diffs it.
    println!("{}", report_json(&evidence, false));

    if let Some(dir) = artifacts {
        if let Err(error) = write_artifacts(dir, &evidence) {
            eprintln!("store bench: cannot write artifacts to {}: {error}", dir.display());
            return 1;
        }
    }

    if evidence.pass {
        0
    } else {
        2
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |path: &Path| {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .replace('/', "\\")
            .to_lowercase()
    };
    norm(a) == norm(b)
}

fn counts_view(counts: &hub_core::Counts) -> CountsView {
    CountsView {
        notes: counts.notes.unwrap_or(-1),
        notes_deleted: counts.notes_deleted.unwrap_or(-1),
        settings: counts.settings.unwrap_or(-1),
        inbox_items: counts.inbox_items.unwrap_or(-1),
        inbox_attachments: counts.inbox_attachments.unwrap_or(-1),
    }
}

fn capture(store: &mut Store) -> Result<Evidence, String> {
    let contract = store.contract().map_err(|e| e.to_string())?;
    let measured = store.schema().map_err(|e| e.to_string())?;
    let counts = store.counts().map_err(|e| e.to_string())?;
    let before = store.schema_fingerprint().map_err(|e| e.to_string())?;

    let live = store.notes(false).map_err(|e| e.to_string())?;
    let all = store.notes(true).map_err(|e| e.to_string())?;

    let sort_ok = is_sorted(&live);
    let (pinned_first, updated_desc_within_pinned) = sort_breakdown(&live);

    let todo = store.todo_inbox().map_err(|e| e.to_string())?;
    let (open, done) = match &todo {
        Some(note) => Store::count_todo_lines(&note.content),
        None => (0, 0),
    };

    let settings_rows = store.settings().map_err(|e| e.to_string())?;
    let mut parsed_objects = 0;
    let mut parsed_scalars = 0;
    let mut autostart = None;
    for row in &settings_rows {
        match serde_json::from_str::<serde_json::Value>(&row.value) {
            Ok(serde_json::Value::Object(map)) => {
                parsed_objects += 1;
                // Measured shape: `{"enabled":true}`, not a bare `true`. The Tauri
                // side writes this key on its own so a settings save cannot clobber
                // it; reading it as a bool would silently report "not set".
                if row.key == "autostart" {
                    autostart = map.get("enabled").and_then(|value| value.as_bool());
                }
            }
            Ok(serde_json::Value::Bool(value)) => {
                parsed_scalars += 1;
                if row.key == "autostart" {
                    autostart = Some(value);
                }
            }
            Ok(_) => parsed_scalars += 1,
            Err(_) => {}
        }
    }

    let db_bytes = std::fs::metadata(store.path()).map(|m| m.len()).unwrap_or(0);

    Ok(Evidence {
        root: store.root().display().to_string(),
        db: store.path().display().to_string(),
        db_bytes,
        sqlite_version: store.sqlite_version(),
        journal_mode: contract.journal_mode,
        user_version: contract.user_version,
        page_size: contract.page_size,
        encoding: contract.encoding,
        counts_before: counts_view(&counts),
        counts_after: counts_view(&counts),
        schema: SchemaView::of(&measured),
        schema_before: before.clone(),
        schema_after: before,
        schema_same: true,
        integrity_before: store.integrity_ok().map_err(|e| e.to_string())?,
        integrity_after: true,
        notes: NotesView {
            live: live.len(),
            with_deleted: all.len(),
            pinned_first,
            updated_desc_within_pinned,
            sort_ok,
        },
        todo: TodoView {
            found: todo.is_some(),
            id: todo.as_ref().map(|n| n.id.clone()).unwrap_or_default(),
            open,
            done,
            content_bytes: todo.as_ref().map(|n| n.content.len()).unwrap_or(0),
        },
        settings: SettingsView {
            keys: settings_rows.iter().map(|row| row.key.clone()).collect(),
            parsed_objects,
            parsed_scalars,
            autostart,
        },
        writes: WriteView::default(),
        dump: all,
        pass: true,
    })
}

fn refresh_after_write(store: &mut Store, evidence: &mut Evidence) -> Result<(), String> {
    let counts = store.counts().map_err(|e| e.to_string())?;
    evidence.counts_after = counts_view(&counts);
    evidence.integrity_after = store.integrity_ok().map_err(|e| e.to_string())?;
    evidence.schema_after = store.schema_fingerprint().map_err(|e| e.to_string())?;
    // Re-dump after writing. The gate compares this dump against the sqlite3 CLI's
    // own view of the same file, so it has to include what we just wrote -- a
    // pre-write dump would make that comparison silently skip the interesting rows.
    evidence.dump = store.notes(true).map_err(|e| e.to_string())?;
    // The whole point of the spike in one comparison: if a write changed
    // `sqlite_master`, the Tauri build is now looking at a schema it did not create.
    evidence.schema_same = evidence.schema_before == evidence.schema_after;
    Ok(())
}

/// `--store-hold`: keep a write transaction open for `hold_ms`, then commit.
///
/// This is the other half of the concurrency question. Two processes on one file is
/// the real deployment (the resident shell plus the Tauri app), so the gate needs a
/// writer that holds the lock on purpose: with `IMMEDIATE` the second writer waits on
/// `busy_timeout` instead of deadlocking, and when the wait exceeds the timeout it
/// must fail cleanly rather than leave a half-written row.
pub fn hold_write_transaction(root: &Path, hold_ms: u64) -> i32 {
    let store = match Store::open_in(root) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("store hold: cannot open {}: {error}", root.display());
            return 1;
        }
    };

    let acquired = std::time::Instant::now();
    if let Err(error) = store.connection().execute_batch("BEGIN IMMEDIATE") {
        eprintln!("store hold: cannot take the write lock: {error}");
        return 1;
    }
    if let Err(error) = store.connection().execute(
        "INSERT OR REPLACE INTO notes (id, title, content, tags, created_at, updated_at, is_pinned)
         VALUES ('s3-hold-row', 'S3 持锁探针', '', '', ?1, ?1, 0)",
        [T0],
    ) {
        let _ = store.connection().execute_batch("ROLLBACK");
        eprintln!("store hold: insert failed: {error}");
        return 1;
    }
    println!("store hold: lock acquired in {:.1} ms, holding {hold_ms} ms", acquired.elapsed().as_secs_f64() * 1000.0);
    std::thread::sleep(std::time::Duration::from_millis(hold_ms));
    match store.connection().execute_batch("COMMIT") {
        Ok(()) => {
            println!("store hold: committed after {} ms", acquired.elapsed().as_millis());
            0
        }
        Err(error) => {
            eprintln!("store hold: commit failed: {error}");
            1
        }
    }
}

/// Delete the rows this bench created earlier, plus their attachment files.
///
/// Deliberately names the ids instead of using a `LIKE 's3-probe%'` pattern: a
/// pattern delete on a live-shaped database is one typo away from eating real data.
fn clear_probes(store: &mut Store) -> Result<(), String> {
    let filenames: Vec<String> = {
        let mut stmt = store
            .connection()
            .prepare("SELECT filename FROM inbox_attachments WHERE item_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([PROBE_ITEM], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        out
    };
    for filename in filenames {
        let _ = std::fs::remove_file(store.attachment_file(&filename));
    }

    let conn = store.connection();
    for (sql, id) in [
        ("DELETE FROM inbox_attachments WHERE item_id = ?1", PROBE_ITEM),
        ("DELETE FROM inbox_items WHERE id = ?1", PROBE_ITEM),
        ("DELETE FROM notes WHERE id = ?1", PROBE_CREATED),
        ("DELETE FROM notes WHERE id = ?1", PROBE_DELETED),
    ] {
        conn.execute(sql, [id]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_probes(store: &mut Store) -> Result<WriteView, String> {
    let mut view = WriteView::default();

    let created = NoteRow {
        id: PROBE_CREATED.to_string(),
        title: "S3 探针".to_string(),
        content: PROBE_BODY.to_string(),
        tags: "s3,probe".to_string(),
        created_at: T0.to_string(),
        updated_at: T0.to_string(),
        is_pinned: false,
        deleted_at: None,
    };
    view.created = store.upsert_note(&created).map_err(|e| e.to_string())?;

    let mut updated = created.clone();
    updated.content = format!("{PROBE_BODY}（已更新）");
    updated.updated_at = T1.to_string();
    store.upsert_note(&updated).map_err(|e| e.to_string())?;
    view.updated_at_changed = store
        .note(PROBE_CREATED)
        .map_err(|e| e.to_string())?
        .map(|note| note.updated_at == T1)
        .unwrap_or(false);

    let doomed = NoteRow {
        id: PROBE_DELETED.to_string(),
        title: "S3 探针（软删）".to_string(),
        content: "这条应该在默认列表里消失".to_string(),
        tags: String::new(),
        created_at: T0.to_string(),
        updated_at: T0.to_string(),
        is_pinned: false,
        deleted_at: None,
    };
    store.upsert_note(&doomed).map_err(|e| e.to_string())?;
    view.soft_deleted = store
        .soft_delete_note(PROBE_DELETED, T1)
        .map_err(|e| e.to_string())?;

    // Attachment: a 1 KiB file with a known pattern, so the gate can compare bytes
    // instead of trusting the size alone.
    let source = std::env::temp_dir().join("hub-s3-attachment-probe.bin");
    let payload: Vec<u8> = (0..1024u32).map(|index| (index % 251) as u8).collect();
    std::fs::write(&source, &payload).map_err(|e| e.to_string())?;

    if store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM inbox_items WHERE id = ?1",
            [PROBE_ITEM],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        == 0
    {
        store
            .create_inbox_text_item(PROBE_ITEM, T0)
            .map_err(|e| e.to_string())?;
    }

    let stored = store
        .store_attachment(PROBE_ITEM, &source, "file", "probe.bin", "application/octet-stream", T1)
        .map_err(|e| e.to_string())?;
    view.attachment_filename = stored.filename.clone();
    view.attachment_bytes = stored.size_bytes;
    view.attachment_row = store
        .attachment(&stored.filename)
        .map_err(|e| e.to_string())?
        .is_some();
    let _ = std::fs::remove_file(&source);

    Ok(view)
}

fn is_sorted(notes: &[NoteRow]) -> bool {
    sort_breakdown(notes).0 && sort_breakdown(notes).1
}

/// Two independent properties, reported separately because "sorted" hides which
/// half failed: pinning is a boolean sort key, `updated_at` is a string sort key.
fn sort_breakdown(notes: &[NoteRow]) -> (bool, bool) {
    let mut pinned_ok = true;
    let mut updated_ok = true;
    for pair in notes.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.is_pinned && !b.is_pinned {
            // pinned before unpinned: correct.
        } else if !a.is_pinned && b.is_pinned {
            pinned_ok = false;
        } else if a.updated_at < b.updated_at {
            // Same pin state: updated_at must not increase down the list.
            updated_ok = false;
        }
    }
    (pinned_ok, updated_ok)
}

fn judge(evidence: &Evidence, wrote: bool) -> bool {
    let mut pass = true;

    // The schema ladder has to have ended where this build says it ends. This is the
    // machine-checked version of "we mirror the Tauri migrations". `legacy_capture_kinds`
    // is deliberately *not* a condition: the CHECK rebuild is Tauri's to run, so
    // reporting it is the contract and failing on it would be blaming the wrong process.
    pass &= evidence.schema.current();

    // Read side: the three counts that can be checked without knowing the data.
    pass &= evidence.integrity_before;
    pass &= evidence.notes.sort_ok;
    pass &= evidence.notes.live <= evidence.notes.with_deleted;
    // Every settings row must decode as JSON; a text blob would mean the column is
    // being written by something that does not understand the format.
    let decoded = evidence.settings.parsed_objects + evidence.settings.parsed_scalars;
    pass &= decoded == evidence.settings.keys.len();

    if wrote {
        pass &= evidence.writes.created;
        pass &= evidence.writes.updated_at_changed;
        pass &= evidence.writes.soft_deleted;
        pass &= evidence.writes.attachment_row;
        pass &= evidence.writes.attachment_bytes == 1024;
        pass &= evidence.counts_after.notes == evidence.counts_before.notes + 2;
        pass &= evidence.counts_after.inbox_attachments
            == evidence.counts_before.inbox_attachments + 1;
        pass &= evidence.integrity_after;
        pass &= evidence.schema_same;
    }

    pass
}

fn report(evidence: &Evidence) -> String {
    let mut out = String::new();
    out.push_str("native store (notes.db)\n");
    out.push_str(&format!(
        "  root={} db={} bytes={}\n",
        evidence.root, evidence.db, evidence.db_bytes
    ));
    out.push_str(&format!(
        "  sqlite={} journal={} user_version={} page_size={} encoding={}\n",
        evidence.sqlite_version,
        evidence.journal_mode,
        evidence.user_version,
        evidence.page_size,
        evidence.encoding
    ));
    out.push_str(&format!(
        "  schema version={}/{} missing={} ahead={} legacy_capture_kinds={}\n",
        evidence.schema.version,
        evidence.schema.expected,
        if evidence.schema.missing.is_empty() {
            "-".to_string()
        } else {
            evidence.schema.missing.join(",")
        },
        if evidence.schema.ahead.is_empty() {
            "-".to_string()
        } else {
            evidence.schema.ahead.join(",")
        },
        evidence.schema.legacy_capture_kinds
    ));
    out.push_str(&format!(
        "  counts before notes={} deleted={} settings={} inbox={} attachments={}\n",
        evidence.counts_before.notes,
        evidence.counts_before.notes_deleted,
        evidence.counts_before.settings,
        evidence.counts_before.inbox_items,
        evidence.counts_before.inbox_attachments
    ));
    out.push_str(&format!(
        "  counts after  notes={} deleted={} settings={} inbox={} attachments={}\n",
        evidence.counts_after.notes,
        evidence.counts_after.notes_deleted,
        evidence.counts_after.settings,
        evidence.counts_after.inbox_items,
        evidence.counts_after.inbox_attachments
    ));
    out.push_str(&format!(
        "  read  live={} with_deleted={} sorted={} (pinned={} updated_desc={})\n",
        evidence.notes.live,
        evidence.notes.with_deleted,
        evidence.notes.sort_ok,
        evidence.notes.pinned_first,
        evidence.notes.updated_desc_within_pinned
    ));
    out.push_str(&format!(
        "  todo  found={} id={} open={} done={} content_bytes={}\n",
        evidence.todo.found,
        if evidence.todo.id.is_empty() { "-" } else { &evidence.todo.id },
        evidence.todo.open,
        evidence.todo.done,
        evidence.todo.content_bytes
    ));
    out.push_str(&format!(
        "  settings keys={} objects={} scalars={} autostart={:?}\n",
        evidence.settings.keys.len(),
        evidence.settings.parsed_objects,
        evidence.settings.parsed_scalars,
        evidence.settings.autostart
    ));
    if evidence.writes.created || evidence.writes.attachment_row {
        out.push_str(&format!(
            "  write created={} updated={} soft_deleted={} attachment={} bytes={} row={}\n",
            evidence.writes.created,
            evidence.writes.updated_at_changed,
            evidence.writes.soft_deleted,
            evidence.writes.attachment_filename,
            evidence.writes.attachment_bytes,
            evidence.writes.attachment_row
        ));
    }
    out.push_str(&format!(
        "  schema unchanged={} integrity before={} after={}\n",
        evidence.schema_same, evidence.integrity_before, evidence.integrity_after
    ));
    out.push_str(&format!("  verdict={}", if evidence.pass { "PASS" } else { "FAIL" }));
    out
}

fn report_json(evidence: &Evidence, include_dump: bool) -> String {
    let keys = evidence
        .settings
        .keys
        .iter()
        .map(|key| format!("\"{}\"", json_escape(key)))
        .collect::<Vec<_>>()
        .join(",");

    let dump = evidence
        .dump
        .iter()
        .map(|note| {
            format!(
                "{{\"id\":\"{}\",\"title\":\"{}\",\"content\":\"{}\",\"tags\":\"{}\",\"created_at\":\"{}\",\"updated_at\":\"{}\",\"is_pinned\":{},\"deleted_at\":{}}}",
                json_escape(&note.id),
                json_escape(&note.title),
                json_escape(&note.content),
                json_escape(&note.tags),
                json_escape(&note.created_at),
                json_escape(&note.updated_at),
                note.is_pinned,
                match &note.deleted_at {
                    Some(at) => format!("\"{}\"", json_escape(at)),
                    None => "null".to_string(),
                }
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        r#"{{"kind":"native-s3-store","root":"{}","db":"{}","db_bytes":{},"sqlite":"{}","schema":{{"version":{},"expected":{},"missing":[{}],"ahead":[{}],"legacy_capture_kinds":{}}},"contract":{{"journal_mode":"{}","user_version":{},"page_size":{},"encoding":"{}"}},"counts_before":{{"notes":{},"deleted":{},"settings":{},"inbox_items":{},"inbox_attachments":{}}},"counts_after":{{"notes":{},"deleted":{},"settings":{},"inbox_items":{},"inbox_attachments":{}}},"notes":{{"live":{},"with_deleted":{},"sorted":{},"pinned_first":{},"updated_desc":{}}},"todo":{{"found":{},"id":"{}","open":{},"done":{},"content_bytes":{}}},"settings":{{"keys":[{}],"objects":{},"scalars":{},"autostart":{}}},"writes":{{"created":{},"updated_at_changed":{},"soft_deleted":{},"attachment":"{}","attachment_bytes":{},"attachment_row":{}}},"schema_same":{},"integrity":{{"before":{},"after":{}}},"pass":{},"notes_dump":[{}]}}"#,
        json_escape(&evidence.root),
        json_escape(&evidence.db),
        evidence.db_bytes,
        json_escape(&evidence.sqlite_version),
        evidence.schema.version,
        evidence.schema.expected,
        json_list(&evidence.schema.missing),
        json_list(&evidence.schema.ahead),
        evidence.schema.legacy_capture_kinds,
        json_escape(&evidence.journal_mode),
        evidence.user_version,
        evidence.page_size,
        json_escape(&evidence.encoding),
        evidence.counts_before.notes,
        evidence.counts_before.notes_deleted,
        evidence.counts_before.settings,
        evidence.counts_before.inbox_items,
        evidence.counts_before.inbox_attachments,
        evidence.counts_after.notes,
        evidence.counts_after.notes_deleted,
        evidence.counts_after.settings,
        evidence.counts_after.inbox_items,
        evidence.counts_after.inbox_attachments,
        evidence.notes.live,
        evidence.notes.with_deleted,
        evidence.notes.sort_ok,
        evidence.notes.pinned_first,
        evidence.notes.updated_desc_within_pinned,
        evidence.todo.found,
        json_escape(&evidence.todo.id),
        evidence.todo.open,
        evidence.todo.done,
        evidence.todo.content_bytes,
        keys,
        evidence.settings.parsed_objects,
        evidence.settings.parsed_scalars,
        match evidence.settings.autostart {
            Some(value) => value.to_string(),
            None => "null".to_string(),
        },
        evidence.writes.created,
        evidence.writes.updated_at_changed,
        evidence.writes.soft_deleted,
        json_escape(&evidence.writes.attachment_filename),
        evidence.writes.attachment_bytes,
        evidence.writes.attachment_row,
        evidence.schema_same,
        evidence.integrity_before,
        evidence.integrity_after,
        evidence.pass,
        if include_dump { dump } else { String::new() }
    )
}

/// A JSON array of strings. Empty stays empty (`[]`), which is how the schema report
/// says "nothing missing" and "nothing ahead".
fn json_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",")
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Hand the schema and the rows to the gate script as files.
///
/// A hash would be smaller, but "unchanged" has to be checkable by a human when the
/// gate fails, and a diff of two schema dumps answers the question directly.
fn write_artifacts(dir: &Path, evidence: &Evidence) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("s3-notes-dump.json"), report_json(evidence, true))?;
    std::fs::write(dir.join("s3-schema-before.txt"), &evidence.schema_before)?;
    std::fs::write(dir.join("s3-schema-after.txt"), &evidence.schema_after)?;
    let mut rows = String::new();
    for note in &evidence.dump {
        rows.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            note.id,
            note.title.replace(['\t', '\n'], " "),
            note.content.len(),
            note.tags,
            note.created_at,
            note.updated_at,
            note.is_pinned,
            note.deleted_at.clone().unwrap_or_default()
        ));
    }
    std::fs::write(dir.join("s3-notes.tsv"), rows)
}
