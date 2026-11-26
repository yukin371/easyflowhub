//! Autosave: the one place that decides when a note's text reaches `notes.db`.
//!
//! ## Why this is a module and not four lines in `note.rs`
//!
//! Because the interesting part is not the write, it is the *when*, and the when
//! has three separate failure modes that all look alike from the outside:
//!
//! * **Nothing arrives.** The text control is a child window: while it has focus,
//!   keystrokes never reach the frame. A debounce that only runs on frames would
//!   never start. So the control raises a flag on the messages that can change the
//!   text, and the flag asks for a frame -- the same handoff `esc` already uses.
//! * **A signal is missed.** Drag-dropped text, an IME commit, an edit made by a
//!   message nobody listed. Correctness therefore does not depend on the signal: a
//!   slow scan compares the control's text against the last written text every
//!   [`SCAN`]. The flag is a latency optimisation, the scan is the guarantee.
//! * **The write fails.** A locked database, a full disk, a corrupt file. The last
//!   written text is then *not* updated, so the next attempt still knows there is
//!   unsaved work. Marking the save as done because the attempt was made is how
//!   text disappears without anyone noticing.
//!
//! ## The debounce is 900 ms, from the shipped build
//!
//! `useQuickNoteSession` sets `AUTOSAVE_DELAY = 900`. Faster would write a row per
//! word; slower would make the close-flush the normal path rather than the exception.
//!
//! ## What a save writes
//!
//! `title`/`content`/`tags` come from [`hub_core::note_text`], which mirrors the
//! shipped parser: title from the first line, trailing `#`-line peeled off into
//! tags. Two rows of a note -- one written here, one written by the other build --
//! must be byte-identical, or the list title changes depending on which window the
//! note was last touched in.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hub_core::clock;
use hub_core::note_text;
use hub_core::{new_note_id, NoteRow, NoteWrite, Store};

use crate::backup;
use crate::log::Log;

/// Idle time after the last change before the text is written (shipped: 900 ms).
pub const AUTOSAVE: Duration = Duration::from_millis(900);

/// Minimum gap between attempts after a failed write.
///
/// Without it a locked database turns into an attempt per frame, and every attempt
/// can block for the store's 5 s busy timeout -- a freeze, on a thread that is
/// drawing the window.
pub const RETRY: Duration = Duration::from_secs(1);

/// How often the control's text is compared with the last written text even when
/// no signal arrived. See the module header: this is the correctness path.
pub const SCAN: Duration = Duration::from_secs(2);

/// Repaint interval when nothing is pending. The shell draws on input, and the
/// island paints itself, so this is a keep-alive rather than an animation.
pub const IDLE: Duration = Duration::from_millis(500);

/// Shortest sleep we ever ask egui for. `request_repaint_after(ZERO)` is a busy loop.
pub const FLOOR: Duration = Duration::from_millis(16);

/// What one attempt did, for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Saved {
        created: bool,
        title: String,
        chars: usize,
        ms: u128,
    },
    /// Nothing was written, and that is correct: an empty new note, or no change.
    Skipped(&'static str),
    /// No store was opened, so the note is memory-only -- Phase 1 behaviour.
    Disabled,
    Failed(String),
}

/// The note's identity plus everything needed to decide whether to write it.
pub struct Saver {
    store: Option<Arc<Mutex<Store>>>,
    id: String,
    created_at: String,
    /// Text as last written, after newline normalisation. The comparison basis.
    saved: String,
    /// When the debounce expires. `None` means nothing is pending.
    due_at: Option<Instant>,
    /// Not before this instant. Set after a failure, so retries back off.
    retry_at: Option<Instant>,
    /// A row exists. From here on an empty body is a legitimate state to write --
    /// otherwise clearing the editor only clears the control and the old text comes
    /// back the next time the note is opened.
    persisted: bool,
    saves: u32,
    failures: u32,
    /// `store=off` is logged once; after that silence is not the same as success.
    said_disabled: bool,
    /// `{store_root}/autosave`, for the draft that survives a crash (Q24). `None`
    /// when there is no store, because then there is no root to put it beside.
    backups: Option<PathBuf>,
    /// One failed draft write is explained once. Repeating it per keystroke would
    /// bury the save lines that matter -- and a draft that cannot be written is
    /// exactly when those lines need to be readable.
    said_stash_error: bool,
}

impl Saver {
    /// A note that has never been written: fresh id, nothing persisted.
    pub fn fresh(store: Option<Arc<Mutex<Store>>>, backups: Option<PathBuf>) -> Self {
        Self {
            store,
            id: new_note_id(),
            created_at: clock::now_iso(),
            saved: String::new(),
            due_at: None,
            retry_at: None,
            persisted: false,
            saves: 0,
            failures: 0,
            said_disabled: false,
            backups,
            said_stash_error: false,
        }
    }

    /// A note loaded from the database. Keeps the row's id and `created_at`, so a
    /// save is an update and not a second identity for the same note.
    pub fn resume(
        store: Option<Arc<Mutex<Store>>>,
        backups: Option<PathBuf>,
        row: &NoteRow,
    ) -> Self {
        Self {
            store,
            id: row.id.clone(),
            created_at: row.created_at.clone(),
            saved: note_text::format_for_display(&row.content, &row.tags),
            due_at: None,
            retry_at: None,
            persisted: true,
            saves: 0,
            failures: 0,
            said_disabled: false,
            backups,
            said_stash_error: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn disabled(&self) -> bool {
        self.store.is_none()
    }

    /// The text the control should be filled with, if this note came from a row.
    pub fn loaded_text(row: &NoteRow) -> String {
        note_text::format_for_display(&row.content, &row.tags)
    }

    /// A change may have happened at `now`. Starts (or restarts) the debounce.
    ///
    /// Restarting is the point: the timer measures *silence*, so a keystroke at
    /// t=800 ms must push the write to t=1700 ms, not let it fire at 900 ms.
    pub fn touched(&mut self, now: Instant) {
        if self.store.is_none() {
            return;
        }
        self.due_at = Some(now + AUTOSAVE);
    }

    /// Has the debounce expired, and is a retry allowed?
    pub fn due(&self, now: Instant) -> bool {
        let ready = self.due_at.is_some_and(|at| now >= at);
        let retry_ok = self.retry_at.map_or(true, |at| now >= at);
        ready && retry_ok
    }

    /// How long until this note needs looking at again.
    pub fn wait(&self, now: Instant) -> Duration {
        let mut next = IDLE;
        if let Some(at) = self.due_at {
            next = next.min(at.saturating_duration_since(now));
        }
        if let Some(at) = self.retry_at {
            next = next.min(at.saturating_duration_since(now));
        }
        next.max(FLOOR)
    }

    /// Does the control hold something other than what was last written?
    pub fn dirty(&self, raw: &str) -> bool {
        note_text::normalize_newlines(raw) != self.saved
    }

    /// Take the control's text as the baseline without writing it.
    ///
    /// Used once, right after a note is loaded. The control does not necessarily
    /// report back the exact string that was pushed into it -- a paragraph mark is
    /// added to the last line -- and treating that difference as an edit would make
    /// every reopened note rewrite its own row a second after opening.
    pub fn rebase(&mut self, raw: &str) {
        self.saved = note_text::normalize_newlines(raw);
    }

    /// Put the current text on disk as a draft, when it differs from the row.
    ///
    /// Q24's other half: the prompt is only useful if something writes the file it
    /// reads. Called on the way *into* a save rather than after it -- a write that
    /// fails is exactly the case where this file is the only copy left.
    ///
    /// Skipped when the text matches the row: a draft of what is already stored
    /// would be deleted by the very next `inspect`, which is churn with no upside.
    pub fn stash(&mut self, raw: &str, log: &Log) {
        let Some(dir) = self.backups.as_ref() else {
            return;
        };
        if !self.dirty(raw) {
            return;
        }
        let draft = backup::Draft::of(raw, clock::now_ms());
        match backup::write(dir, &self.id, &draft) {
            Ok(()) => self.said_stash_error = false,
            Err(error) => {
                // Once, not per keystroke: the note still saves, and the log line
                // that says so must not be pushed out by the one that says this.
                if !self.said_stash_error {
                    self.said_stash_error = true;
                    log.mark(
                        0,
                        "note_backup",
                        &format!("id={} write=\"{error}\"", self.id),
                    );
                }
            }
        }
    }

    /// The row is now the truth, so the draft has nothing left to protect.
    fn clear_stash(&self) {
        if let Some(dir) = self.backups.as_ref() {
            backup::clear(dir, &self.id);
        }
    }

    /// Drop the pending write when the text turns out to be what was already
    /// written. Returns `true` when nothing is pending any more.
    ///
    /// This is what keeps a keystroke that RichEdit ignored (a modifier, a key it
    /// consumed) from leaving the note permanently "dirty" and every frame reading
    /// the whole document.
    pub fn settle(&mut self, raw: &str) -> bool {
        if note_text::normalize_newlines(raw) != self.saved {
            return false;
        }
        self.due_at = None;
        true
    }

    /// Write now, whatever the timer says. Used by the close path and by the
    /// debounce having elapsed.
    pub fn save(&mut self, raw: &str) -> Outcome {
        let text = note_text::normalize_newlines(raw);
        let Some(store) = self.store.clone() else {
            return Outcome::Disabled;
        };

        if self.persisted && text == self.saved {
            self.due_at = None;
            // A draft that matches the row protects nothing. This branch is reached
            // when the control reported a difference that was not one -- it happens
            // once per open, because `rebase` takes the baseline from the control's
            // own text while a later read can differ by a paragraph mark.
            self.clear_stash();
            return Outcome::Skipped("unchanged");
        }

        let parsed = note_text::parse(&text);

        // Parity Q23: a window that never received a keystroke must not leave a row
        // behind. Once a row exists the opposite holds -- see `persisted`.
        if !self.persisted && note_text::is_blank(&text) && parsed.title.trim().is_empty() {
            self.saved = text;
            self.due_at = None;
            self.clear_stash();
            return Outcome::Skipped("empty");
        }

        let write = NoteWrite {
            id: self.id.clone(),
            title: parsed.title.clone(),
            content: parsed.content.clone(),
            tags: note_text::normalize_tags(&parsed.tags),
            created_at: self.created_at.clone(),
            updated_at: clock::now_iso(),
        };

        let started = Instant::now();
        let result = match store.lock() {
            Ok(mut store) => store.save_note_body(&write).map_err(|error| error.to_string()),
            // A poisoned lock means another note panicked while writing. Reporting
            // it as a failed save keeps the text eligible for the next attempt
            // instead of pretending it is on disk.
            Err(_) => Err("store lock poisoned".to_string()),
        };
        let ms = started.elapsed().as_millis();

        match result {
            Ok(created) => {
                let chars = parsed.content.chars().count();
                self.persisted = true;
                self.saved = text;
                self.due_at = None;
                self.retry_at = None;
                self.saves += 1;
                // Only after the row is written: clearing first would open a window
                // where neither copy exists.
                self.clear_stash();
                Outcome::Saved {
                    created,
                    title: parsed.title,
                    chars,
                    ms,
                }
            }
            Err(error) => {
                self.failures += 1;
                self.retry_at = Some(Instant::now() + RETRY);
                // `due_at` stays: there is unsaved text, and the only thing that
                // clears it is a write that actually succeeded.
                Outcome::Failed(format!("{error} after {ms} ms"))
            }
        }
    }
}

/// The frame-side half: the editor's change flag, the scan clock, and the log.
pub struct Pump {
    saver: Saver,
    edit: Arc<AtomicBool>,
    scan: Instant,
    initial: Option<String>,
}

impl Pump {
    pub fn new(store: Option<Arc<Mutex<Store>>>, row: Option<&NoteRow>) -> Self {
        // Next to the database, so a note opened with `--store-root` keeps its
        // drafts under the same root the gate points at. Resolved once: the root
        // cannot change while this window is open.
        let backups = store.as_ref().and_then(backups_of);
        let saver = match row {
            Some(row) => Saver::resume(store, backups, row),
            None => Saver::fresh(store, backups),
        };
        let initial = row.map(Saver::loaded_text);
        Self {
            saver,
            edit: Arc::new(AtomicBool::new(false)),
            scan: Instant::now(),
            initial,
        }
    }

    pub fn id(&self) -> &str {
        self.saver.id()
    }

    pub fn disabled(&self) -> bool {
        self.saver.disabled()
    }

    /// The flag the control raises on a text-changing message.
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.edit.clone()
    }

    /// Mark a change the shell made itself.
    ///
    /// `EM_UNDO`/`EM_REDO` edit the document without producing a `WM_CHAR`, so the
    /// control's hook never fires and the undo would wait for the 2 s scan.
    pub fn changed(&self) {
        self.edit.store(true, Ordering::SeqCst);
    }

    /// Point this window at another note and hand back its text.
    ///
    /// The caller flushes first. What is dropped here is the pump's memory of the
    /// previous document -- id, `created_at`, baseline text, pending debounce --
    /// because all four belong to a note this window no longer shows.
    pub fn load(&mut self, row: &NoteRow) -> String {
        self.saver = Saver::resume(
            self.saver.store.clone(),
            self.saver.backups.clone(),
            row,
        );
        self.edit.store(false, Ordering::SeqCst);
        self.scan = Instant::now();
        self.initial = None;
        Saver::loaded_text(row)
    }

    /// Start a new note in this window: a fresh id, nothing persisted yet.
    pub fn restart(&mut self) {
        self.saver = Saver::fresh(self.saver.store.clone(), self.saver.backups.clone());
        self.edit.store(false, Ordering::SeqCst);
        self.scan = Instant::now();
        self.initial = None;
    }

    /// Text to put in the control once it exists, then `None` forever after.
    pub fn take_initial(&mut self) -> Option<String> {
        self.initial.take()
    }

    /// Accept what the control actually holds as the saved baseline.
    pub fn rebase(&mut self, text: &str) {
        self.saver.rebase(text);
    }

    /// One frame's worth of work. Returns when the next look is needed.
    ///
    /// `text` is a closure because reading the control copies the whole document
    /// (`WM_GETTEXT`): on most frames there is nothing to compare, and paying for
    /// the copy sixty times a second to discover that would be the expensive way to
    /// do nothing.
    pub fn tick<F>(&mut self, now: Instant, text: F, log: &Log) -> Duration
    where
        F: FnOnce() -> String,
    {
        if self.saver.disabled() {
            if !self.saver.said_disabled {
                self.saver.said_disabled = true;
                log.mark(
                    0,
                    "note_save",
                    &format!("id={} store=off (this note lives in memory only)", self.saver.id()),
                );
            }
            return IDLE;
        }

        let touched = self.edit.swap(false, Ordering::SeqCst);
        if touched {
            self.saver.touched(now);
        }
        let scan_due = now.saturating_duration_since(self.scan) >= SCAN;
        let due = self.saver.due(now);
        if !touched && !due && !scan_due {
            return self.saver.wait(now);
        }

        let text = text();
        self.scan = now;

        if touched && !due && self.saver.settle(&text) {
            // The message did not change anything. Keep the clock, drop the write.
            return self.saver.wait(now);
        }
        // Before the save, never after: see `Saver::stash`.
        self.saver.stash(&text, log);
        if due {
            self.commit(&text, "autosave", log);
        } else if scan_due && self.saver.dirty(&text) {
            // The scan found a change no signal reported: arm the debounce from
            // here, so the write happens one idle period later. Guarded by `dirty`
            // because arming it on an unchanged document would schedule a write
            // every scan forever -- a log line every 2 s, and a pointless
            // transaction for a note nobody is typing in.
            self.saver.touched(now);
        }
        self.saver.wait(now)
    }

    /// Write now, whatever the timer says.
    ///
    /// `reason` names the caller in the log. It is a parameter rather than always
    /// `"close"` because the three paths that flush are different questions --
    /// "the window went away", "this window now shows another note" and "a note was
    /// created from a search" -- and a log that calls all three a close cannot
    /// answer why a row appeared.
    pub fn flush(&mut self, text: &str, reason: &str, log: &Log) {
        if self.saver.disabled() {
            log.mark(
                0,
                "note_save",
                &format!("id={} reason={reason} store=off skipped=true", self.saver.id()),
            );
            return;
        }
        self.saver.stash(text, log);
        self.commit(text, reason, log);
    }

    fn commit(&mut self, text: &str, reason: &str, log: &Log) {
        let outcome = self.saver.save(text);
        let id = self.saver.id().to_string();
        match outcome {
            Outcome::Saved {
                created,
                title,
                chars,
                ms,
            } => log.mark(
                0,
                "note_save",
                &format!(
                    "id={id} reason={reason} created={created} title=\"{title}\" chars={chars} ms={ms} saves={}",
                    self.saver.saves
                ),
            ),
            Outcome::Skipped(why) => log.mark(
                0,
                "note_save",
                &format!("id={id} reason={reason} skipped={why}"),
            ),
            Outcome::Disabled => log.mark(
                0,
                "note_save",
                &format!("id={id} reason={reason} store=off skipped=true"),
            ),
            Outcome::Failed(error) => log.mark(
                0,
                "note_save",
                &format!(
                    "id={id} reason={reason} failed=\"{error}\" failures={} retry={}ms",
                    self.saver.failures,
                    RETRY.as_millis()
                ),
            ),
        }
    }
}

/// `{root}/autosave` for a store. The lock is taken once, at open time.
///
/// Also used by `note::recover`, so the prompt and the writer cannot disagree
/// about where the drafts are -- a prompt reading a different directory would
/// simply never find one, which looks exactly like "this note has no backup".
pub(crate) fn backups_of(store: &Arc<Mutex<Store>>) -> Option<PathBuf> {
    let guard = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Some(backup::dir(guard.root()))
}

/// Build the callback handed to `Editor::on_edit`.
///
/// Same shape as [`crate::esc::handler`] and for the same reason: the subclass proc
/// runs inside a message handler, so it may not touch the shell. It raises a flag
/// and asks *this* viewport for a frame -- asking the context without a target
/// would wake whichever viewport ran last, and with two notes open the save would
/// wait for the other note's timer.
pub fn handler(
    flag: Arc<AtomicBool>,
    ctx: egui::Context,
    viewport: egui::ViewportId,
) -> Box<dyn Fn() + Send + Sync> {
    Box::new(move || {
        flag.store(true, Ordering::SeqCst);
        ctx.request_repaint_of(viewport);
    })
}

#[cfg(test)]
mod tests;
