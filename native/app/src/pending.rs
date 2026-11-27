//! The one thing an auto-reset event cannot carry: which note to open.
//!
//! `--note-id <id>` against a shell that is already running has to hand an id to a
//! process it cannot talk to. The doorbell in `hub_win::instance` is an event: it
//! wakes the owner and says nothing else. Two options were affordable and one was
//! not -- a named shared-memory section per request needs a size, a lifetime and a
//! protocol, and `WM_COPYDATA` needs a window handle the target never publishes. This
//! is the smallest thing that works: one short line in a file beside the settings,
//! read exactly once.
//!
//! Three properties are the whole design:
//!
//! * **Written by rename, never in place.** A reader that arrives mid-write would
//!   otherwise open a truncated id and spawn a window for a note that does not exist.
//!   `rename` on the same volume is atomic, so the file is either the previous
//!   request or this one.
//! * **Read once, then deleted.** The ring is auto-reset, so N launches collapse into
//!   one answer; leaving the file behind would make the *next*, unrelated ring open
//!   the same note again.
//! * **Stale requests expire.** A crash between write and ring leaves the file
//!   orphaned, and a launch an hour later must not open yesterday's note. Sixty
//!   seconds is far past the measured 0.1 s handshake and short enough that nobody
//!   would call the result "a note I did not ask for".

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Beside the settings file, so `--settings` moves both together (the gates rely on
/// that: two shells pointed at separate directories must not share a request).
pub const FILE: &str = "pending-note";

/// How long a written request stays valid.
pub const STALE_MS: u64 = 60_000;

fn path(settings_path: &Path) -> PathBuf {
    settings_path.with_file_name(FILE)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// Record "open this note" for the owner of the shell.
///
/// Returns an error rather than writing something `take` cannot parse: a silently
/// mangled id becomes a window that pretends to be the note and is not.
pub fn put(settings_path: &Path, id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains('\n') || id.contains('\r') {
        return Err(format!("note id {id:?} cannot be carried through a line"));
    }
    let target = path(settings_path);
    let temp = target.with_extension("tmp");
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    fs::write(&temp, format!("{id}\n{}\n", now_ms()))
        .map_err(|error| format!("{}: {error}", temp.display()))?;
    fs::rename(&temp, &target).map_err(|error| format!("{}: {error}", target.display()))
}

/// Take the request, if there is a fresh one.
///
/// `None` covers all three: no file, a file this process cannot read, and one old
/// enough to be an orphan. The caller's answer to all three is the same -- open a new
/// note -- so they do not need to be told apart. The removal happens before the
/// parse for the same reason it happens at all: a request is answered once.
pub fn take(settings_path: &Path) -> Option<String> {
    let target = path(settings_path);
    let raw = fs::read_to_string(&target).ok()?;
    let _ = fs::remove_file(&target);

    let mut lines = raw.lines();
    let id = lines.next()?.trim();
    let at: u64 = lines.next()?.trim().parse().ok()?;
    if id.is_empty() || now_ms().saturating_sub(at) > STALE_MS {
        return None;
    }
    Some(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// Its own directory per test: the real function writes beside a settings file,
    /// and two tests sharing one would delete each other's request.
    fn scratch() -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("efh-pending-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("native-spike.json")
    }

    #[test]
    fn a_request_survives_the_round_trip() {
        let settings = scratch();
        put(&settings, "note-42").expect("write");
        assert_eq!(take(&settings).as_deref(), Some("note-42"));
    }

    #[test]
    fn taking_a_request_answers_it_once() {
        let settings = scratch();
        put(&settings, "note-42").expect("write");
        assert_eq!(take(&settings).as_deref(), Some("note-42"));
        // The second ring is a different launch. Answering the *same* id again would
        // open a window nobody asked for.
        assert_eq!(take(&settings), None);
    }

    #[test]
    fn a_request_from_a_crashed_launch_expires() {
        let settings = scratch();
        let target = settings.with_file_name(FILE);
        fs::create_dir_all(target.parent().expect("dir")).expect("mkdir");
        fs::write(&target, format!("note-old\n{}\n", now_ms() - STALE_MS - 1)).expect("write");

        assert_eq!(take(&settings), None);
        assert!(!target.exists(), "an expired request is still cleared");
    }

    #[test]
    fn a_malformed_request_is_not_a_note() {
        let settings = scratch();
        let target = settings.with_file_name(FILE);
        fs::create_dir_all(target.parent().expect("dir")).expect("mkdir");
        fs::write(&target, "note-half").expect("write");
        assert_eq!(take(&settings), None);
    }

    #[test]
    fn an_id_that_could_not_be_read_back_is_refused() {
        let settings = scratch();
        assert!(put(&settings, "").is_err());
        assert!(put(&settings, "note\nsecond-line").is_err());
        assert_eq!(take(&settings), None, "a refused write must leave nothing behind");
    }
}
