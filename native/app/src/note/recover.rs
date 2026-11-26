//! The window-side half of Q24: notice the draft, hide the editor, answer.
//!
//! `backup.rs` decides *what* to ask and `hub_ui::recover` draws it; this module
//! owns the two things only a window can do -- taking the text control off the
//! screen so the card is not behind it, and writing the answer back through the
//! save pump.
//!
//! ## Why the editor is hidden, not just covered
//!
//! The text control is a Win32 child window created over this frame's client area.
//! Child windows paint above their parent, so every egui pixel under it is lost --
//! the switcher learned this the same way. Hiding is also what makes the card
//! *modal*: with the control visible, a click on the card would land on the note's
//! body and the choice could never be made.
//!
//! ## What each answer writes
//!
//! Restore and merge both go through `save::Pump::flush`, so the row is written
//! immediately instead of on the 900 ms debounce. That is not impatience: a user
//! who answers "restore my backup" and then closes the window in the next second
//! must not lose the text they just recovered. Keep writes nothing -- the whole
//! point of that button is that the row is already right.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hub_core::clock;
use hub_core::{NoteRow, Store};
use hub_ui::recover;

use crate::backup;
use crate::log::Log;
use crate::save;

use super::NoteShell;

/// Whether this window has a question to ask.
pub(super) enum State {
    /// Nothing to ask -- no draft, a stale one, or the user already answered.
    Done,
    Asking {
        draft: backup::Draft,
        /// The control was taken off the screen. Done once per question: calling
        /// `ShowWindow(SW_HIDE)` every frame is a message per frame for no change.
        hidden: bool,
        /// The button rectangles were written to the log, once.
        logged: bool,
    },
}

/// The state a window opens with: a question if `row` has a draft worth asking
/// about, otherwise nothing. `row` is `None` for a scratch pad, which has no id
/// to look a draft up under.
pub(super) fn start(
    store: &Option<Arc<Mutex<Store>>>,
    row: Option<&NoteRow>,
    log: &Log,
) -> State {
    match row {
        Some(row) => inspect(store, row, log),
        None => State::Done,
    }
}

/// Look for a draft that belongs to `row`, and log what was found.
///
/// Called when a note is opened and after the switcher changes what this window
/// shows -- both are moments where the body and the row may have parted ways while
/// the process was not running.
pub(super) fn inspect(store: &Option<Arc<Mutex<Store>>>, row: &NoteRow, log: &Log) -> State {
    let Some(dir) = store.as_ref().and_then(save::backups_of) else {
        return State::Done;
    };
    match backup::inspect(&dir, &row.id, row) {
        backup::Verdict::Quiet => {
            log.mark(
                0,
                "note_recover",
                &format!("note_id={} found=false", row.id),
            );
            State::Done
        }
        backup::Verdict::Removed(why) => {
            log.mark(
                0,
                "note_recover",
                &format!("note_id={} removed={why}", row.id),
            );
            State::Done
        }
        backup::Verdict::Ask(draft) => {
            log.mark(
                0,
                "note_recover",
                &format!(
                    "note_id={} ask=true timestamp={} chars={}",
                    row.id,
                    draft.timestamp,
                    draft.content.chars().count()
                ),
            );
            State::Asking {
                draft,
                hidden: false,
                logged: false,
            }
        }
    }
}

impl NoteShell {
    /// Draw the card and apply the answer. `true` means it owns this frame.
    ///
    /// The autosave tick is skipped on purpose: the document cannot change while
    /// the control is hidden, and the pump has no pending write -- an opened note
    /// is rebased before this runs.
    pub(super) fn recover_frame(&mut self, ctx: &egui::Context, area: egui::Rect) -> bool {
        if !matches!(self.recover, State::Asking { .. }) {
            return false;
        }
        // egui's first frame reports a placeholder screen rectangle -- measured
        // 216 x 356 against a real 400 x 268 -- and a card drawn into it puts the
        // buttons off the left edge. The frame after this one has the window's real
        // size, so nothing is lost by waiting, and the gate reads the rectangles
        // from a frame where they mean something. The floor is the card's own
        // minimum: below it the card would be drawn with its buttons outside it.
        if area.width() < 384.0 || area.height() < 244.0 {
            ctx.request_repaint();
            return false;
        }

        if let State::Asking { hidden, .. } = &mut self.recover {
            if !*hidden {
                if let Some(editor) = self.editor.as_ref() {
                    editor.hide();
                }
                *hidden = true;
            }
        }

        // The draft is borrowed for the whole draw, so the answer is collected here
        // and applied afterwards -- `resolve` needs `&mut self`.
        let action = {
            let State::Asking { draft, .. } = &self.recover else {
                return false;
            };
            let current = self
                .editor
                .as_ref()
                .map(|editor| editor.text())
                .unwrap_or_default();
            let when = format!(
                "备份时间：{}",
                backup::ago(clock::now_ms(), draft.timestamp)
            );
            let shown = recover::draw(
                ctx,
                self.palette,
                ctx.screen_rect(),
                area,
                &when,
                &current,
                &draft.display(),
            );
            self.note_button_rects(ctx, &shown);
            shown.action
        };

        if action != recover::Action::None {
            self.resolve(action);
        }
        ctx.request_repaint_after(save::IDLE);
        true
    }

    /// Publish where the three buttons are, once, so a gate can click them.
    ///
    /// Physical pixels, because that is what `click_client` takes. Written from the
    /// same `Shown` the painter used, which is the difference between "the gate
    /// clicked the button" and "the gate clicked where the button used to be".
    fn note_button_rects(&mut self, ctx: &egui::Context, shown: &recover::Shown) {
        let State::Asking { logged, .. } = &mut self.recover else {
            return;
        };
        if *logged {
            return;
        }
        *logged = true;
        let ppp = ctx.pixels_per_point();
        let describe = |rect: egui::Rect| {
            format!(
                "{:.0},{:.0},{:.0},{:.0}",
                rect.min.x * ppp,
                rect.min.y * ppp,
                rect.width() * ppp,
                rect.height() * ppp
            )
        };
        self.log.mark(
            0,
            "recover_rect",
            &format!(
                "card={} screen={} keep={} merge={} restore={}",
                describe(shown.card),
                describe(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    ctx.screen_rect().size()
                )),
                describe(shown.buttons[0]),
                describe(shown.buttons[1]),
                describe(shown.buttons[2])
            ),
        );
    }

    /// Apply one of the three answers.
    fn resolve(&mut self, action: recover::Action) {
        let replaced = std::mem::replace(&mut self.recover, State::Done);
        let State::Asking { draft, .. } = replaced else {
            return;
        };
        let id = self.save.id().to_string();

        if action == recover::Action::Keep {
            if let Some(dir) = self.backup_dir() {
                backup::clear(&dir, &id);
            }
            self.log.mark(
                0,
                "note_recover_action",
                &format!("note_id={id} action=keep"),
            );
            self.focus_editor();
            return;
        }

        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let (text, verb) = match action {
            recover::Action::Restore => (draft.display(), "restore"),
            recover::Action::Merge => (backup::merge(&editor.text(), &draft), "merge"),
            recover::Action::Keep | recover::Action::None => return,
        };
        let chars = text.chars().count();
        editor.set_text(&text);
        // Replacing the document empties the control's undo stack anyway; saying so
        // keeps a future `set_text` that preserves history from making this answer
        // undoable into a half-restored state.
        editor.clear_undo();
        // A draft carries the tag line too, so restoring one has the same "caret at
        // the end means inside the tags" problem as opening a note does.
        self.park_caret();
        let shown = editor.text();
        // `flush` stashes first and clears the draft on success: if the write fails,
        // the same question comes back instead of the text vanishing.
        self.save.flush(&shown, "recover", &self.log);
        self.save.rebase(&shown);
        self.log.mark(
            0,
            "note_recover_action",
            &format!("note_id={id} action={verb} chars={chars}"),
        );
        self.focus_editor();
    }

    /// Give the text back after the card is gone.
    fn focus_editor(&mut self) {
        if let Some(editor) = self.editor.as_ref() {
            editor.show();
            editor.focus();
        }
        if let Some(hwnd) = self.hwnd {
            hub_win::window::focus(hwnd);
        }
    }

    fn backup_dir(&self) -> Option<PathBuf> {
        self.store.as_ref().and_then(save::backups_of)
    }
}
