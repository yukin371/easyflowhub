//! The switcher's window-side half: hide the island, own the keyboard, switch.
//!
//! Split out of `note.rs` for the same reason `island.rs` was: the guard caps a
//! file at 800 lines, and this is a seam the module already had. Everything here
//! happens *because the panel is open* -- the body belongs to a list instead of
//! to a text control, and leaving the panel is the moment the window has to
//! decide what it is showing.
//!
//! ## Why switching is in place
//!
//! The shipped switcher changes what the current window shows. Opening a second
//! window instead would leave the old note behind, which is not a switch but a
//! duplicate: the user would end up with one window per note they ever looked at.
//! In place means the pump has to forget the old document -- after its pending
//! write is flushed -- and take on the new one's id and baseline.

use std::sync::MutexGuard;

use hub_core::Store;
use hub_ui::switcher;

use super::NoteShell;

impl NoteShell {
    /// `Ctrl+K`: take the body over with the list (Q25).
    pub(super) fn open_switcher(&mut self, ctx: &egui::Context) {
        self.switcher.open();
        self.items = self.load_items();
        self.switcher_open = true;
        // The island is a child window and children paint over their parent, so
        // the panel would otherwise be behind the text it is searching for.
        if let Some(editor) = self.editor.as_ref() {
            editor.hide();
        }
        // Focus has to come back to the frame or the keys keep going to a hidden
        // control, and the query box would never see a character.
        if let Some(hwnd) = self.hwnd {
            hub_win::window::focus(hwnd);
        }
        self.log.mark(
            0,
            "note_switcher",
            &format!("id={} open=true items={}", self.id, self.items.len()),
        );
        ctx.request_repaint();
    }

    /// Leave the switcher and give the text back.
    pub(super) fn close_switcher(&mut self, ctx: &egui::Context) {
        if !self.switcher_open {
            return;
        }
        self.switcher_open = false;
        self.items.clear();
        if let Some(editor) = self.editor.as_ref() {
            editor.show();
            editor.focus();
        }
        self.log
            .mark(0, "note_switcher", &format!("id={} open=false", self.id));
        ctx.request_repaint();
    }

    /// The notes the panel can pick from.
    ///
    /// Read on open, never per frame: a query keystroke costs a `contains` over
    /// this list, and a database read per keystroke would undo the reason the
    /// list is held at all.
    fn load_items(&self) -> Vec<switcher::Item> {
        let Some(store) = self.store.as_ref() else {
            self.log
                .mark(0, "note_switcher", "store=off items=0 (nothing to switch to)");
            return Vec::new();
        };
        let guard: MutexGuard<'_, Store> =
            store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match guard.notes(false) {
            Ok(rows) => rows
                .iter()
                .map(|row| {
                    switcher::Item::new(&row.id, &row.title, &row.content, &row.tags, &row.updated_at)
                })
                .collect(),
            Err(error) => {
                self.log
                    .mark(0, "note_switcher", &format!("error=\"{error}\" items=0"));
                Vec::new()
            }
        }
    }

    /// Point this window at another note (Q27).
    pub(super) fn switch_to(&mut self, id: &str) {
        let from = self.save.id().to_string();
        if from == id {
            self.log
                .mark(0, "note_switch", &format!("id={} to={id} result=same", self.id));
            return;
        }
        // Flush before dropping the pump's memory of the old document: `load`
        // replaces the id and the baseline, and a pending debounce that went with
        // them is the same silent loss as a close without a flush.
        if let Some(text) = self.editor.as_ref().map(|editor| editor.text()) {
            self.save.flush(&text, "switch", &self.log);
        }
        let row = self.store.as_ref().and_then(|store| {
            let guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.note(id).ok().flatten()
        });
        let Some(row) = row else {
            self.log.mark(
                0,
                "note_switch",
                &format!("id={} to={id} result=missing", self.id),
            );
            return;
        };
        let text = self.save.load(&row);
        if let Some(editor) = self.editor.as_ref() {
            editor.set_text(&text);
            // The switch must not be undoable back into the previous note.
            editor.clear_undo();
            editor.caret_to_end();
            editor.scroll_caret();
        }
        let shown = self
            .editor
            .as_ref()
            .map(|editor| editor.text())
            .unwrap_or_default();
        self.save.rebase(&shown);
        self.log.mark(
            0,
            "note_switch",
            &format!(
                "id={} from={from} to={id} chars={} title=\"{}\"",
                self.id,
                shown.chars().count(),
                row.title
            ),
        );
    }

    /// Start a new note in this window, optionally seeded with the query.
    ///
    /// Q27's "index 0 creates a new note": the previous document is flushed first
    /// for the same reason a switch is.
    ///
    /// The seed is written *now*, not on the 900 ms debounce, and that is not an
    /// optimisation: `Pump::tick` drops a pending write when the document matches
    /// the baseline, so a seeded note used to sit in memory until the window closed
    /// -- a note the switcher had already listed as "opened" with nothing in the
    /// list behind it. Writing it here also gives the row a `created_at` before the
    /// user can press Esc on it.
    ///
    /// The baseline is then taken from what the *control* reports rather than from
    /// the seed: RichEdit adds a paragraph mark to a document it was handed, and
    /// treating that mark as an edit would rewrite the same row one second later.
    pub(super) fn start_new(&mut self, title: &str) {
        if let Some(text) = self.editor.as_ref().map(|editor| editor.text()) {
            self.save.flush(&text, "switch", &self.log);
        }
        self.save.restart();
        if let Some(editor) = self.editor.as_ref() {
            editor.set_text(title);
            editor.clear_undo();
            editor.caret_to_end();
        }
        let shown = self
            .editor
            .as_ref()
            .map(|editor| editor.text())
            .unwrap_or_default();
        // A single-line search box cannot contain a line break, so anything after
        // the last visible character is the control's, not the user's.
        let seed = shown.trim_end_matches(['\r', '\n']);
        if seed.is_empty() {
            self.save.rebase(&shown);
        } else {
            self.save.flush(seed, "seed", &self.log);
            self.save.rebase(&shown);
        }
        self.log.mark(
            0,
            "note_switch",
            &format!(
                "id={} to=new note_id={} seeded={} chars={}",
                self.id,
                self.save.id(),
                !seed.is_empty(),
                seed.chars().count()
            ),
        );
    }
}
