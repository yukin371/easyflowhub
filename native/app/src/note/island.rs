//! The text island: the RichEdit child window that holds the characters.
//!
//! ## Why this is a separate file
//!
//! `note.rs` was over the size cap with the island inside it, and this is the seam
//! the module already had: everything here talks to a control handle or to the
//! theme that control was built with, and nothing here decides layout, window
//! state or shortcuts. Splitting on that seam keeps `impl NoteShell` readable from
//! both sides -- one half is "where the window is and what it looks like", this
//! half is "what is inside it and when that changes".
//!
//! It is a child module rather than a free function set because the island and the
//! shell share the note's identity, log, viewport and save pump, and threading six
//! arguments through every call would only move the coupling into the signatures.

use hub_core::note_text;
use hub_core::PixelRect;
use hub_editor::Editor;

use super::NoteShell;
use crate::esc;
use crate::log::Log;
use crate::save;

impl NoteShell {
    /// Translate the user's logical appearance into the editor's physical theme.
    ///
    /// `pixels_per_point` is the only place the scale is applied: `font_pt` is in
    /// points and must *not* be multiplied (that bug rendered 16.5 pt at 150%),
    /// while the inset is in logical pixels and must be.
    pub(super) fn theme_for(&self, ctx: &egui::Context) -> hub_editor::Theme {
        let ppp = ctx.pixels_per_point().max(0.1);
        hub_editor::Theme {
            background: colorref(self.palette.canvas),
            foreground: colorref(self.palette.ink),
            font_face: self.appearance.font_family.clone(),
            font_pt: self.appearance.font_pt,
            line_spacing: self.appearance.line_spacing,
            padding: (
                (self.appearance.padding_x as f32 * ppp).round() as i32,
                (self.appearance.padding_y as f32 * ppp).round() as i32,
            ),
            caret_width: (self.appearance.caret_width as f32 * ppp).round() as i32,
        }
    }

    pub(super) fn sync_editor(&mut self, ctx: &egui::Context, rect: PixelRect) {
        let theme = self.theme_for(ctx);
        let hwnd = self.hwnd.unwrap_or(0);

        match self.editor.as_mut() {
            None => match Editor::create(hwnd, rect, &theme) {
                Ok(mut editor) => {
                    editor.on_escape(esc::handler(
                        self.escape.clone(),
                        self.esc_presses.clone(),
                        ctx.clone(),
                        self.viewport,
                    ));
                    editor.on_key(self.keys.handler(ctx.clone(), self.shortcuts.clone()));
                    // RichEdit treats `Ctrl+wheel` as text zoom. The shell wants it
                    // for window opacity, so the notch is claimed at the control.
                    editor.on_wheel(self.wheel.handler(ctx.clone()));
                    // Every message that can change the document. The callback only
                    // raises a flag and asks for a frame; the comparison against the
                    // last written text is what decides whether anything is saved.
                    editor.on_edit(save::handler(
                        self.save.flag(),
                        ctx.clone(),
                        self.viewport,
                    ));
                    // A click in the body, which egui never sees: the control is a
                    // child window covering that rectangle. Dismisses the tools
                    // panel (Q7).
                    editor.on_click(self.clicks.handler(ctx.clone(), self.viewport));
                    // Q3: the note's edge is the resize grip, and below the strip
                    // that edge belongs to this control. The hook claims the press
                    // so no caret moves and no selection starts while the window is
                    // being resized under the pointer.
                    self.edge.arm(ctx);
                    let signal = self.edge.clone();
                    editor.on_press(Box::new(move |press| signal.on_press(press)));
                    // ...and *look* like a grip: the pointer over the band becomes
                    // the resize cursor, so the edge is visible before it is used.
                    let hover = self.edge.clone();
                    editor.on_hover(Box::new(move |press| hover.on_hover(press)));
                    // Focus first: `EM_SETSEL`/`EM_SCROLLCARET` are no-ops without it,
                    // so the caret used to be parked at 0 and the first screenful
                    // was the only one that ever scrolled.
                    editor.focus();
                    // An existing note is filled before the caret is placed, or the
                    // caret would sit at the end of an empty document and the first
                    // keystroke would land at the top instead of after the text.
                    let mut loaded = None;
                    if let Some(text) = self.save.take_initial() {
                        let chars = text.chars().count();
                        editor.set_text(&text);
                        // The load must not be undoable: `Ctrl+Z` right after opening
                        // should undo the user's typing, not empty the note.
                        editor.clear_undo();
                        // What the control reports is the baseline, not what was
                        // pushed into it: the control may add a paragraph mark, and
                        // treating that as an edit would rewrite the row right after
                        // every open.
                        let shown = editor.text();
                        self.save.rebase(&shown);
                        loaded = Some(chars);
                    }
                    park(&editor, self.id, &self.log);
                    editor.apply_caret_width(theme.caret_width);
                    // Logged *after* the caret is parked, not before: the gate (and
                    // any later script) types as soon as it sees this line, and a
                    // line written while the caret was still at 0 is a race the
                    // reader cannot win -- measured once as a character landing
                    // second-from-last.
                    if let Some(chars) = loaded {
                        self.log.mark(
                            0,
                            "note_load",
                            &format!("id={} note_id={} chars={chars}", self.id, self.save.id()),
                        );
                    }
                    // Q19's criterion: focused on open, caret at the end. Logged
                    // because "the first keystroke went somewhere else" is otherwise
                    // indistinguishable from a key that was never delivered.
                    self.log.mark(
                        0,
                        "editor_focus",
                        &format!(
                            "id={} focused={} caret={:?}",
                            self.id,
                            editor.is_focused(),
                            editor.caret()
                        ),
                    );
                    self.last_theme = Some(theme.clone());
                    self.log.mark(
                        0,
                        "island_created",
                        &format!(
                            "id={} rect={}x{}+{}+{} ppp={:.2} pt={} pad={}x{} caret={}",
                            self.id,
                            rect.width,
                            rect.height,
                            rect.x,
                            rect.y,
                            ctx.pixels_per_point(),
                            self.appearance.font_pt,
                            theme.padding.0,
                            theme.padding.1,
                            theme.caret_width
                        ),
                    );
                    self.editor = Some(editor);
                }
                Err(error) => self.log.mark(0, "island_failed", &error),
            },
            Some(editor) => {
                if self.last_theme.as_ref() != Some(&theme) {
                    editor.apply_theme(&theme);
                    self.last_theme = Some(theme);
                }
                if editor.set_rect(rect) {
                    self.log.mark(
                        0,
                        "island_rect",
                        &format!(
                            "id={} {}x{}+{}+{}",
                            self.id, rect.width, rect.height, rect.x, rect.y
                        ),
                    );
                }
            }
        }
        self.last_island = Some(rect);
    }

    /// Park the caret where the next keystroke belongs: the end of the body.
    ///
    /// Q19 was written as "caret at the end", and the end of the *text* is inside
    /// the tag line on any note that has tags -- so the first thing typed after
    /// reopening was appended to the tags (`# 工作 重要` became one tag,
    /// `工作 重要续`). `note_text::caret_offset` finds the end of the body instead;
    /// its byte offset then goes through `hub_editor::position_of`, because the
    /// control counts character positions, not bytes and not UTF-16 units.
    pub(super) fn park_caret(&self) {
        if let Some(editor) = self.editor.as_ref() {
            park(editor, self.id, &self.log);
        }
    }

    /// Undo/redo through the control, not through a shadow history.
    ///
    /// `Ctrl+Z` used to be left to RichEdit; routing it through the table too is
    /// what makes the egui path (focus on the title strip) behave like the island
    /// path. `can=` is logged because a redo with nothing to redo would otherwise
    /// look identical to a working one.
    ///
    /// It lives here rather than in `note.rs` because every line of it is about the
    /// control: the history, the document length, and the edit signal an undo does
    /// not produce.
    pub(super) fn history(&self, undo: bool) {
        let Some(editor) = self.editor.as_ref() else {
            self.log.mark(0, "note_history", "editor=none");
            return;
        };
        let available = if undo {
            editor.can_undo()
        } else {
            editor.can_redo()
        };
        if available {
            if undo {
                editor.undo(1);
            } else {
                editor.redo(1);
            }
        }
        // `EM_UNDO` changes the document without a `WM_CHAR`, so the edit signal
        // that normally starts the debounce never fires. Raising it here is what
        // keeps an undo from waiting for the 2 s scan.
        self.save.changed();
        self.log.mark(
            0,
            "note_history",
            &format!(
                "id={} {} can={available} len={}",
                self.id,
                if undo { "undo" } else { "redo" },
                editor.text().chars().count()
            ),
        );
    }
}

/// The same placement for a control the shell does not own yet: the island is
/// built as a local and only moved into `self.editor` after the text is in it.
fn park(editor: &Editor, id: u32, log: &Log) {
    let text = editor.text();
    let at = note_text::caret_offset(&text);
    // Bytes from the parser, character positions for the control: `CRLF` counts
    // once there and twice in UTF-16. The two numbers differ by exactly the number
    // of line breaks above the caret, which is the whole tag-line bug.
    let positions = hub_editor::position_of(&text, at);
    let end = hub_editor::position_of(&text, text.len());
    editor.set_caret(positions);
    editor.scroll_caret();
    // The script's readiness marker is `editor_focus`, which now carries the real
    // caret; this line says *why* it is not at the end, so a note that reopened
    // into its tags again would be visible in the log alone.
    log.mark(
        0,
        "note_caret",
        &format!(
            "id={id} at={positions} of={end} tag_line={}",
            at < text.len()
        ),
    );
}

/// egui stores RGB; Windows colour values are `0x00BBGGRR`.
fn colorref(color: egui::Color32) -> u32 {
    color.r() as u32 | (color.g() as u32) << 8 | (color.b() as u32) << 16
}
