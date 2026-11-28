//! The scripts' trash: the directory `scriptmgr` leaves behind, read from here.
//!
//! ## Why this module reads another process's directory
//!
//! The shipped manager reaches the scripts' trash through the script service
//! (`scriptmgr-go`'s `ListScriptTrash` / `RestoreScriptTrash` / `PurgeScriptTrash`).
//! The native shell has no sidecar handshake yet -- that is the same missing piece
//! that made `relay_url` a setting -- so "the scripts' trash" is either unreachable
//! or reachable as what it actually is on disk: one directory per deleted script,
//! each with an `entry.json`.
//!
//! That format is a **contract with another process**, and it is written down in
//! `scriptmgr-go/internal/api/scriptfiles.go`. Reading it here is a deliberate,
//! narrow duplication:
//!
//! * the shape copied is what `readTrashEntry` requires -- a non-empty id equal to
//!   the directory name, and an absolute `original_path`. Anything else is skipped
//!   and reported, never "repaired": the native shell must not guess at a manifest
//!   it did not write.
//! * `restore` refuses when the original path is occupied, exactly as
//!   `RestoreScriptTrash` does. Overwriting a file that appeared after the delete
//!   would destroy the only other copy.
//! * `purge` and `empty` remove directories, nothing more. There is no index to
//!   keep in step: the service re-reads the directory on every call, which is what
//!   makes it safe for two processes to touch it.
//!
//! What is **not** duplicated: deleting a live script into the trash. That decides
//! which roots are legal (`pathInsideRoot` against the service's configured roots)
//! and has a rollback path this shell has no reason to re-implement. The native
//! workbench only ever sees the trash, never creates one.

use std::fs;
use std::path::{Path, PathBuf};

use crate::store::StoreError;

pub type Result<T> = std::result::Result<T, StoreError>;

/// `%APPDATA%\scriptmgr\trash\scripts`, or `None` when there is no `%APPDATA%`.
///
/// `os.UserConfigDir()` on Windows is `%APPDATA%`, which is why the service's
/// `config.Load` lands here. A machine without `%APPDATA%` is not a machine this
/// product starts on, so `None` -- not a panic -- and the caller says so.
pub fn default_root() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("scriptmgr")
            .join("trash")
            .join("scripts"),
    )
}

/// One deleted script, as its `entry.json` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub original_path: String,
    pub script_type: String,
    pub deleted_at: String,
    pub size_bytes: u64,
}

/// The manifest file name. Also the one name `restore` never moves back.
const MANIFEST: &str = "entry.json";

/// What `list` found: the entries, and the directories it could not read.
///
/// Two lists rather than one plus an error, because one corrupt directory must not
/// hide the twenty healthy ones -- and because "the trash did not list" and "the
/// trash listed less than it holds" are different things to show a user.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    pub entries: Vec<Entry>,
    /// Directory names that were not a readable entry, with the reason.
    pub skipped: Vec<String>,
}

/// Every entry in the trash, newest delete first. A missing root is an empty trash.
pub fn list(root: &Path) -> Result<Listing> {
    let dirs = match fs::read_dir(root) {
        Ok(dirs) => dirs,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Listing::default())
        }
        Err(error) => return Err(StoreError::Io(error)),
    };
    let mut listing = Listing::default();
    for dir in dirs {
        let dir = dir?;
        if !dir.file_type()?.is_dir() {
            continue;
        }
        let name = dir.file_name().to_string_lossy().to_string();
        match read_entry(&dir.path()) {
            Ok(entry) if entry.id == name => listing.entries.push(entry),
            Ok(entry) => listing
                .skipped
                .push(format!("{name}: manifest id is {:?}", entry.id)),
            Err(error) => listing.skipped.push(format!("{name}: {error}")),
        }
    }
    // Newest first. A string compare is right because the service writes RFC 3339
    // with a fixed offset; a mixed-offset entry would sort wrong and is a bug in the
    // manifest, not something to correct here.
    listing
        .entries
        .sort_by(|left, right| right.deleted_at.cmp(&left.deleted_at));
    Ok(listing)
}

/// Put one script back where it came from.
///
/// Everything in the entry directory except the manifest travels back: the script
/// itself, and whatever metadata file the service kept next to it. Naming them by
/// their own names -- rather than reconstructing the two names the service happens
/// to use -- means a service that starts keeping a second sidecar does not need this
/// function changed.
///
/// All destinations are checked before anything moves. A half-finished restore
/// leaves the script back in place and its metadata in the trash, which is the
/// failure the service itself documents and the one worth avoiding.
pub fn restore(root: &Path, id: &str) -> Result<Entry> {
    let dir = entry_dir(root, id)?;
    let entry = read_entry(&dir)?;
    let destination = PathBuf::from(&entry.original_path);
    if !destination.is_absolute() {
        return Err(StoreError::Refused(format!(
            "manifest path is not absolute: {}",
            entry.original_path
        )));
    }
    if destination.exists() {
        return Err(StoreError::Refused(format!(
            "a file already exists at {}",
            destination.display()
        )));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| StoreError::Refused(format!("{} has no parent", destination.display())))?;

    let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
    for file in fs::read_dir(&dir)? {
        let file = file?;
        if !file.file_type()?.is_file() {
            continue;
        }
        let name = file.file_name();
        if name == MANIFEST {
            continue;
        }
        let target = parent.join(&name);
        if target.exists() {
            return Err(StoreError::Refused(format!(
                "a file already exists at {}",
                target.display()
            )));
        }
        moves.push((file.path(), target));
    }
    fs::create_dir_all(parent)?;
    for (from, to) in moves {
        move_file(&from, &to)?;
    }
    fs::remove_dir_all(&dir)?;
    Ok(entry)
}

/// Delete one entry for good. `false` when it was not there any more.
pub fn purge(root: &Path, id: &str) -> Result<bool> {
    let dir = entry_dir(root, id)?;
    if !dir.join(MANIFEST).is_file() {
        return Ok(false);
    }
    fs::remove_dir_all(&dir)?;
    Ok(true)
}

/// Empty the trash. Reports how many entries went.
pub fn empty(root: &Path) -> Result<usize> {
    let listing = list(root)?;
    if !root.exists() {
        return Ok(0);
    }
    fs::remove_dir_all(root)?;
    Ok(listing.entries.len())
}

/// Delete what has been in the trash past the window.
///
/// The cutoff is compared at second resolution because the two sides do not write
/// the same string: this crate writes nine fractional digits and a `+00:00` offset,
/// the service writes RFC 3339 with whatever precision `time.RFC3339` produces. The
/// first nineteen characters -- `YYYY-MM-DDTHH:MM:SS` -- are the part both agree on.
pub fn purge_expired(root: &Path, now: &str, days: i64) -> Result<usize> {
    if days <= 0 {
        return Ok(0);
    }
    let Some(ms) = crate::clock::unix_ms(now) else {
        return Ok(0);
    };
    let cutoff = crate::clock::iso_from_unix(ms / 1_000 - days * 86_400, 0);
    let Some(cutoff) = second_prefix(&cutoff) else {
        return Ok(0);
    };
    let cutoff = cutoff.to_string();
    let mut purged = 0;
    for entry in list(root)?.entries {
        let Some(deleted) = second_prefix(&entry.deleted_at) else {
            // An entry whose date cannot be read is never expired by guesswork: the
            // delete is irreversible and the manifest is the only witness.
            continue;
        };
        if deleted < cutoff.as_str() {
            let dir = root.join(&entry.id);
            if fs::remove_dir_all(&dir).is_ok() {
                purged += 1;
            }
        }
    }
    Ok(purged)
}

/// The directory an id names, refusing anything that could escape the root.
fn entry_dir(root: &Path, id: &str) -> Result<PathBuf> {
    valid_id(id)
        .then(|| root.join(id))
        .ok_or_else(|| StoreError::Refused(format!("invalid trash entry id {id:?}")))
}

/// Ids are minted by the service as hex; a path separator in one would be an escape.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

/// `YYYY-MM-DDTHH:MM:SS` of an RFC 3339 stamp, or `None` when it is not one.
fn second_prefix(stamp: &str) -> Option<&str> {
    let bytes = stamp.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    for (at, byte) in bytes[..19].iter().enumerate() {
        let separator = match at {
            4 | 7 => Some(b'-'),
            10 => Some(b'T'),
            13 | 16 => Some(b':'),
            _ => None,
        };
        match separator {
            Some(want) if *byte != want => return None,
            Some(_) => {}
            None if !byte.is_ascii_digit() => return None,
            None => {}
        }
    }
    // Every byte above is ASCII, so the 19th is a character boundary.
    Some(&stamp[..19])
}

fn read_entry(dir: &Path) -> Result<Entry> {
    let text = fs::read_to_string(dir.join(MANIFEST))?;
    let raw: RawEntry = serde_json::from_str(&text)
        .map_err(|error| StoreError::Refused(format!("unreadable {MANIFEST}: {error}")))?;
    if raw.id.trim().is_empty() {
        return Err(StoreError::Refused("manifest is missing an id".to_string()));
    }
    if raw.original_path.trim().is_empty() {
        return Err(StoreError::Refused(
            "manifest is missing an original path".to_string(),
        ));
    }
    Ok(Entry {
        id: raw.id,
        name: raw.name,
        original_path: raw.original_path,
        script_type: raw.script_type,
        deleted_at: raw.deleted_at,
        size_bytes: raw.size_bytes.max(0) as u64,
    })
}

/// The manifest as written by `model.ScriptTrashEntry`. Field names are its JSON
/// tags, so a rename on that side shows up here as a missing field, not as silently
/// empty strings.
#[derive(serde::Deserialize)]
struct RawEntry {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    original_path: String,
    #[serde(default)]
    script_type: String,
    #[serde(default)]
    deleted_at: String,
    #[serde(default)]
    size_bytes: i64,
}

/// `rename`, with a copy-and-remove fallback for the case where the trash and the
/// original path are on different volumes (`%APPDATA%` on C: and the script on D:
/// is ordinary on Windows).
fn move_file(from: &Path, to: &Path) -> Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(rename) => {
            fs::copy(from, to).map_err(|error| {
                StoreError::Refused(format!(
                    "could not move {} to {}: {error} (rename said {rename})",
                    from.display(),
                    to.display()
                ))
            })?;
            fs::remove_file(from)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-11-28T12:00:00.000000000+00:00";

    fn root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("hub-script-trash-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        root
    }

    /// Write one entry the way the service does: a directory named after the id,
    /// the script inside it, and `entry.json`.
    fn seed(root: &Path, id: &str, original: &Path, deleted_at: &str) -> PathBuf {
        let dir = root.join(id);
        fs::create_dir_all(&dir).expect("entry dir");
        let name = original
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        fs::write(dir.join(&name), b"echo hi").expect("script");
        let manifest = serde_json::json!({
            "id": id,
            "name": name,
            "original_path": original.to_string_lossy(),
            "script_type": "python",
            "deleted_at": deleted_at,
            "size_bytes": 7,
        });
        fs::write(dir.join(MANIFEST), manifest.to_string()).expect("manifest");
        dir
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hub-script-home-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("home");
        dir
    }

    #[test]
    fn a_missing_root_is_an_empty_trash_not_an_error() {
        // A machine with no script library must not make the trash tab fail; it is
        // a different sentence from "everything was emptied".
        let missing = std::env::temp_dir().join("hub-script-trash-does-not-exist");
        let _ = fs::remove_dir_all(&missing);
        let listing = list(&missing).expect("a missing root lists nothing");
        assert!(listing.entries.is_empty());
        assert!(listing.skipped.is_empty());
    }

    #[test]
    fn entries_are_read_newest_first_and_a_corrupt_one_does_not_hide_the_rest() {
        let root = root("list");
        let home = temp_dir("list");
        seed(&root, "aaa", &home.join("a.py"), "2026-11-20T10:00:00Z");
        seed(&root, "bbb", &home.join("b.py"), "2026-11-25T10:00:00Z");
        // A directory that is not an entry: the service would report it, and so do we.
        fs::create_dir_all(root.join("ccc")).expect("junk dir");

        let listing = list(&root).expect("list");
        let ids: Vec<&str> = listing.entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["bbb", "aaa"], "newest delete first");
        assert_eq!(listing.skipped.len(), 1, "{:?}", listing.skipped);
        assert!(listing.skipped[0].starts_with("ccc"), "{:?}", listing.skipped);
    }

    #[test]
    fn an_id_that_is_not_the_directory_name_is_skipped() {
        // The service checks this too (`entry.ID != dir.Name()`), and for the same
        // reason: an id nobody can address is a row that cannot be restored.
        let root = root("mismatch");
        let home = temp_dir("mismatch");
        let dir = seed(&root, "wrong", &home.join("x.py"), "2026-11-20T10:00:00Z");
        fs::rename(&dir, root.join("right")).expect("rename");
        let listing = list(&root).expect("list");
        assert!(listing.entries.is_empty());
        assert!(listing.skipped[0].contains("manifest id is"), "{:?}", listing.skipped);
    }

    #[test]
    fn restoring_puts_the_script_and_its_sidecar_back() {
        let root = root("restore");
        let home = temp_dir("restore");
        let original = home.join("sub").join("a.py");
        seed(&root, "one", &original, "2026-11-20T10:00:00Z");
        // The metadata file the service moves along with the script.
        fs::write(root.join("one").join("a.py.meta.json"), b"{}").expect("sidecar");

        let entry = restore(&root, "one").expect("restore");
        assert_eq!(entry.original_path, original.to_string_lossy());
        assert_eq!(fs::read(&original).expect("script is home"), b"echo hi");
        assert!(home.join("sub").join("a.py.meta.json").exists(), "the sidecar came too");
        assert!(!root.join("one").exists(), "the entry directory is gone");
    }

    #[test]
    fn restoring_refuses_rather_than_overwrite_a_file_that_came_back() {
        // The user re-created the script after deleting it. Overwriting it would
        // destroy the only other copy, so the restore stops and says why.
        let root = root("occupied");
        let home = temp_dir("occupied");
        let original = home.join("a.py");
        seed(&root, "one", &original, "2026-11-20T10:00:00Z");
        fs::write(&original, b"newer").expect("the user's new file");

        let error = restore(&root, "one").expect_err("must refuse");
        assert!(format!("{error}").contains("already exists"), "{error}");
        assert_eq!(fs::read(&original).expect("kept"), b"newer");
        assert!(root.join("one").join(MANIFEST).exists(), "the entry stayed intact");
    }

    #[test]
    fn an_id_with_a_separator_is_refused_before_it_reaches_the_filesystem() {
        let root = root("escape");
        let error = restore(&root, "../etc").expect_err("must refuse");
        assert!(format!("{error}").contains("invalid"), "{error}");
    }

    #[test]
    fn purging_one_removes_only_that_directory() {
        let root = root("purge");
        let home = temp_dir("purge");
        seed(&root, "one", &home.join("a.py"), "2026-11-20T10:00:00Z");
        seed(&root, "two", &home.join("b.py"), "2026-11-21T10:00:00Z");

        assert!(purge(&root, "one").expect("purge"));
        assert!(!root.join("one").exists());
        assert!(root.join("two").exists());
        // A second call is not an error: the row is gone, which is what it asked for.
        assert!(!purge(&root, "one").expect("purge again"));
    }

    #[test]
    fn emptying_counts_what_it_took() {
        let root = root("empty");
        let home = temp_dir("empty");
        seed(&root, "one", &home.join("a.py"), "2026-11-20T10:00:00Z");
        seed(&root, "two", &home.join("b.py"), "2026-11-21T10:00:00Z");
        assert_eq!(empty(&root).expect("empty"), 2);
        assert!(!root.exists());
        // Empty again is zero, not a failure: the button may be pressed twice.
        assert_eq!(empty(&root).expect("empty again"), 0);
    }

    #[test]
    fn the_retention_sweep_compares_seconds_across_the_two_stamp_formats() {
        // The local cutoff has nine fractional digits and `+00:00`; the service
        // writes `Z`. Comparing the whole strings would put one format before the
        // other forever, which is why the sweep compares nineteen characters.
        let root = root("expire");
        let home = temp_dir("expire");
        seed(&root, "old", &home.join("a.py"), "2026-10-01T10:00:00Z");
        seed(&root, "fresh", &home.join("b.py"), "2026-11-27T10:00:00Z");

        assert_eq!(purge_expired(&root, NOW, 30).expect("sweep"), 1);
        assert!(!root.join("old").exists());
        assert!(root.join("fresh").exists(), "20 days is inside the window");
    }

    #[test]
    fn an_unreadable_date_or_a_zero_window_keeps_everything() {
        let root = root("refuse");
        let home = temp_dir("refuse");
        seed(&root, "old", &home.join("a.py"), "2026-10-01T10:00:00Z");
        assert_eq!(purge_expired(&root, "not a date", 30).expect("sweep"), 0);
        assert_eq!(purge_expired(&root, NOW, 0).expect("sweep"), 0);
        assert!(root.join("old").exists());
    }

    #[test]
    fn a_stamp_that_is_not_rfc3339_has_no_second_prefix() {
        assert!(second_prefix("2026-11-28T12:00:00Z").is_some());
        assert!(second_prefix("2026-11-28T12:00:00.000000000+00:00").is_some());
        assert!(second_prefix("2026-11-28 12:00:00").is_none());
        assert!(second_prefix("2026-11-28T12:00").is_none());
        assert!(second_prefix("").is_none());
    }
}
