//! The second yes.
//!
//! One button in this window cannot be undone: 彻底删除 removes the row from the
//! database, not from the trash. The first version purged on a single click -- the
//! gate clicked it and the note was gone, which is exactly how a user would find
//! out. Undo does not exist here (there is no journal to undo into), so the only
//! honest fix is to ask once more before acting.
//!
//! ## Why the pending action is state and not an [`Action`](super::Action)
//!
//! An `Action` says "the user decided this". A dialog that has been opened and not
//! answered is not a decision, it is a question -- and the controller must not see
//! questions, or it would have to know how to draw them. So the view holds it
//! ([`State::pending`](super::State)) and only emits `Action::Purge` after the
//! second yes. Esc is the one thing the controller *does* need to know about: it
//! closes the window, so "Esc while a dialog is open" has to mean "cancel the
//! dialog" instead. That is what [`State::dismiss`](super::State::dismiss) reports.
//!
//! ## Three kinds, three sentences
//!
//! The trash holds notes, capture items and scripts, and "permanent" is not the
//! same fact for the three: a note leaves a table, a capture takes its attachment
//! files with it, and a script *is* a file. One shared sentence would have to be
//! vague enough to cover all three -- which is the sentence a user reads right
//! before pressing the only irreversible button in the window.

use egui::{Align, Layout, RichText, Stroke};

use crate::theme::Theme;

use super::{report, Action, RowKind, State};

/// A destructive action that has been asked for and not yet answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// Permanent: the row leaves the database (and, for a capture, so do the files
    /// it pointed at). Restoring it is not possible, which is why this asks.
    Purge {
        kind: RowKind,
        id: String,
        title: String,
    },
    /// One whole sub-tab at once. Same irreversibility as [`Pending::Purge`], but
    /// the question has to name a count instead of a title -- "清空回收站？" with no
    /// number is a question the user cannot answer.
    EmptyTrash { kind: RowKind, count: usize },
}

impl Pending {
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Purge { .. } => "purge",
            Self::EmptyTrash { .. } => "empty",
        }
    }

    /// Which source this question is about. Logged next to the verb: `verb=empty
    /// kind=scripts` is a post-mortem's only way to tell the three apart.
    pub fn kind(&self) -> RowKind {
        match self {
            Self::Purge { kind, .. } | Self::EmptyTrash { kind, .. } => *kind,
        }
    }

    /// The row this question is about. `*` for the whole-sub-tab question, which has
    /// no id: the log still needs a field there, and an empty one reads as a bug.
    pub fn id(&self) -> &str {
        match self {
            Self::Purge { id, .. } => id,
            Self::EmptyTrash { .. } => "*",
        }
    }

    fn question(&self) -> &'static str {
        match (self, self.kind()) {
            (Self::Purge { .. }, RowKind::Note) => "彻底删除这条笔记？",
            (Self::Purge { .. }, RowKind::Inbox) => "彻底删除这个收集项？",
            (Self::Purge { .. }, RowKind::Script) => "彻底删除这个脚本？",
            (Self::EmptyTrash { .. }, RowKind::Note) => "清空回收站？",
            (Self::EmptyTrash { .. }, RowKind::Inbox) => "清空收集箱？",
            (Self::EmptyTrash { .. }, RowKind::Script) => "清空脚本？",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Purge { title, .. } => match self.kind() {
                RowKind::Note => format!("「{title}」会从数据库删除，回收站也找不回来。"),
                RowKind::Inbox => {
                    format!("「{title}」和它的附件会被删除，回收站也找不回来。")
                }
                RowKind::Script => {
                    format!("「{title}」的文件会从回收站删除，原位置不会再有它。")
                }
            },
            Self::EmptyTrash { count, .. } => match self.kind() {
                RowKind::Note => {
                    format!("{count} 条笔记会从数据库删除，回收站也找不回来。")
                }
                RowKind::Inbox => {
                    format!("{count} 个收集项和它们的附件会被删除，回收站也找不回来。")
                }
                RowKind::Script => {
                    format!("{count} 个脚本的文件会被删除，回收站也找不回来。")
                }
            },
        }
    }

    fn yes(&self) -> &'static str {
        match (self, self.kind()) {
            (Self::Purge { .. }, _) => "彻底删除",
            (Self::EmptyTrash { .. }, RowKind::Note) => "清空回收站",
            (Self::EmptyTrash { .. }, RowKind::Inbox) => "清空收集箱",
            (Self::EmptyTrash { .. }, RowKind::Script) => "清空脚本",
        }
    }

    /// What answering yes means. One place, so a new destructive verb cannot forget
    /// to say which [`Action`](super::Action) it produces -- and cannot drop the
    /// kind, which is what tells the controller which store to write.
    fn agreed(self) -> Action {
        match self {
            Self::Purge { kind, id, .. } => Action::Purge { kind, id },
            Self::EmptyTrash { kind, .. } => Action::EmptyTrash { kind },
        }
    }
}

/// Draw the dialog, and answer with what the user chose.
///
/// Returns `Action::Purge` only on the second yes. Everything else -- 取消, Esc, a
/// click on the backdrop -- clears [`State::pending`] and returns `Action::None`.
pub fn dialog(ctx: &egui::Context, palette: Theme, state: &mut State) -> Action {
    let Some(pending) = state.pending.clone() else {
        return Action::None;
    };

    let mut decision: Option<bool> = None;
    let response = egui::Modal::new(egui::Id::new("manager-confirm"))
        .frame(
            egui::Frame::NONE
                .fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border_strong))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.set_max_width(300.0);
            ui.label(
                RichText::new(pending.question())
                    .size(13.0)
                    .color(palette.ink),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(pending.detail())
                    .size(11.0)
                    .color(palette.ink_soft),
            );
            ui.add_space(14.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let yes = ui.add(
                    egui::Button::new(
                        RichText::new(pending.yes())
                            .size(11.0)
                            .color(palette.on_accent),
                    )
                    .fill(palette.danger)
                    .stroke(Stroke::NONE),
                );
                report(state, "confirm-ok", yes.rect);
                if yes.clicked() {
                    decision = Some(true);
                }
                let no = ui.add(
                    egui::Button::new(RichText::new("取消").size(11.0).color(palette.ink))
                        .fill(palette.canvas)
                        .stroke(Stroke::new(1.0, palette.border)),
                );
                report(state, "confirm-cancel", no.rect);
                if no.clicked() {
                    decision = Some(false);
                }
            });
        });

    // A click on the backdrop is "get me out of here", never "yes".
    if decision.is_none() && response.should_close() {
        decision = Some(false);
    }
    // Enter is the keyboard's yes -- but only when nothing is being typed into.
    // Without the focus guard, a stray Enter in the search box would delete a row.
    if decision.is_none()
        && ctx.memory(|memory| memory.focused().is_none())
        && ctx.input(|input| input.key_pressed(egui::Key::Enter))
    {
        decision = Some(true);
    }

    match decision {
        Some(true) => {
            state.pending = None;
            pending.agreed()
        }
        Some(false) => {
            state.pending = None;
            Action::None
        }
        None => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::{draw, Row, Tab};
    use hub_core::Table;

    fn trashed() -> Vec<Row> {
        vec![Row::new(
            "gone",
            "删掉的",
            "删掉的",
            "",
            "2026-11-26T02:31:00+00:00",
            false,
            Some("2026-11-26T03:00:00+00:00".to_string()),
        )]
    }

    fn purge() -> Pending {
        Pending::Purge {
            kind: RowKind::Note,
            id: "gone".to_string(),
            title: "删掉的".to_string(),
        }
    }

    fn pass(
        ctx: &egui::Context,
        state: &mut State,
        rows: &[Row],
        events: Vec<egui::Event>,
    ) -> Action {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(900.0, 620.0),
            )),
            events,
            ..Default::default()
        };
        let table = Table::defaults();
        let mut action = Action::None;
        let _ = ctx.run(input, |ctx| {
            action = draw(ctx, Theme::standard(), state, rows, &[], &table, None);
        });
        action
    }

    fn rect_of(state: &State, name: &str) -> egui::Rect {
        state
            .hits
            .iter()
            .find(|(other, _)| other == name)
            .unwrap_or_else(|| panic!("{name} was not reported"))
            .1
    }

    /// Run frames until the named control stops moving, then hand back where it is.
    ///
    /// A centred `Area` learns its own size from the frame it just drew, so on the
    /// first frame the dialog is laid out as if it were zero-sized and lands in the
    /// corner. The real window runs sixty frames a second and the gate reads the
    /// *latest* rectangle, so "settle, then click" is the honest model -- clicking
    /// the first frame's rectangle would be testing a position no user can see.
    fn settle(ctx: &egui::Context, state: &mut State, rows: &[Row], name: &str) -> egui::Rect {
        let mut last: Option<egui::Rect> = None;
        for _ in 0..4 {
            let _ = pass(ctx, state, rows, Vec::new());
            let Some(rect) = state
                .hits
                .iter()
                .find(|(other, _)| other == name)
                .map(|(_, rect)| *rect)
            else {
                continue;
            };
            if last == Some(rect) {
                return rect;
            }
            last = Some(rect);
        }
        last.unwrap_or_else(|| panic!("{name} was never reported"))
    }

    fn click(at: egui::Pos2) -> Vec<egui::Event> {
        let press = |pressed: bool| {
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            }]
        };
        let mut events = press(true);
        events.extend(press(false));
        events
    }

    #[test]
    fn a_purge_click_asks_instead_of_deleting() {
        // The regression this whole module exists for: one click used to be enough.
        let ctx = egui::Context::default();
        let rows = trashed();
        let mut state = State {
            tab: Tab::Trash,
            selected: Some("gone".to_string()),
            ..Default::default()
        };
        // The buttons are drawn for the row selected when the frame started.
        let action = pass(&ctx, &mut state, &rows, Vec::new());
        assert_eq!(action, Action::None);
        let at = rect_of(&state, "purge:gone").center();

        let action = pass(&ctx, &mut state, &rows, click(at));
        assert_eq!(action, Action::None, "one click must not purge");
        assert_eq!(state.pending, Some(purge()), "the dialog must be open");
    }

    #[test]
    fn the_second_yes_is_the_one_that_purges() {
        let ctx = egui::Context::default();
        let rows = trashed();
        let mut state = State {
            tab: Tab::Trash,
            selected: Some("gone".to_string()),
            pending: Some(purge()),
            ..Default::default()
        };
        // The dialog is drawn this frame, so its buttons are known after it settles.
        let at = settle(&ctx, &mut state, &rows, "confirm-ok").center();

        let action = pass(&ctx, &mut state, &rows, click(at));
        assert_eq!(
            action,
            Action::Purge {
                kind: RowKind::Note,
                id: "gone".to_string()
            }
        );
        assert_eq!(state.pending, None, "answering closes the dialog");
    }

    #[test]
    fn the_second_yes_empties_the_whole_trash() {
        // Same dialog, different verb: the yes has to produce `EmptyTrash`, not the
        // `Purge` of some row -- the two questions are asked by the same widget and
        // that is exactly where they could be confused.
        let ctx = egui::Context::default();
        let rows = trashed();
        let mut state = State {
            tab: Tab::Trash,
            pending: Some(Pending::EmptyTrash {
                kind: RowKind::Note,
                count: 1,
            }),
            ..Default::default()
        };
        let at = settle(&ctx, &mut state, &rows, "confirm-ok").center();

        let action = pass(&ctx, &mut state, &rows, click(at));
        assert_eq!(
            action,
            Action::EmptyTrash {
                kind: RowKind::Note
            }
        );
        assert_eq!(state.pending, None);
    }

    #[test]
    fn each_kind_asks_its_own_question_and_answers_with_its_own_kind() {
        // "Permanent" is three different facts. A shared sentence would have to be
        // vague enough to cover a table row, an attachment and a file -- and this is
        // the text a user reads before the only irreversible click in the window.
        let note = purge();
        let capture = Pending::Purge {
            kind: RowKind::Inbox,
            id: "cap".to_string(),
            title: "一张图".to_string(),
        };
        let script = Pending::Purge {
            kind: RowKind::Script,
            id: "s1".to_string(),
            title: "run.py".to_string(),
        };
        assert_eq!(note.question(), "彻底删除这条笔记？");
        assert_eq!(capture.question(), "彻底删除这个收集项？");
        assert_eq!(script.question(), "彻底删除这个脚本？");
        assert!(capture.detail().contains("附件"), "{}", capture.detail());
        assert!(script.detail().contains("原位置"), "{}", script.detail());
        assert_eq!(capture.agreed(), Action::Purge { kind: RowKind::Inbox, id: "cap".to_string() });
        assert_eq!(script.agreed(), Action::Purge { kind: RowKind::Script, id: "s1".to_string() });

        let empty = |kind| Pending::EmptyTrash { kind, count: 2 };
        assert_eq!(empty(RowKind::Inbox).yes(), "清空收集箱");
        assert_eq!(empty(RowKind::Script).yes(), "清空脚本");
        assert_eq!(
            empty(RowKind::Script).agreed(),
            Action::EmptyTrash { kind: RowKind::Script }
        );
        // The log fields, so a post-mortem can tell the three apart.
        assert_eq!(empty(RowKind::Script).kind().id(), "scripts");
        assert_eq!(empty(RowKind::Script).id(), "*");
    }

    #[test]
    fn cancelling_keeps_the_row_and_closes_the_dialog() {
        let ctx = egui::Context::default();
        let rows = trashed();
        let mut state = State {
            tab: Tab::Trash,
            selected: Some("gone".to_string()),
            pending: Some(purge()),
            ..Default::default()
        };
        let at = settle(&ctx, &mut state, &rows, "confirm-cancel").center();

        let action = pass(&ctx, &mut state, &rows, click(at));
        assert_eq!(action, Action::None, "cancel must not purge");
        assert_eq!(state.pending, None);
    }

    #[test]
    fn dismiss_reports_whether_anything_was_open() {
        // The controller's Esc is "close the window"; with a dialog open it has to
        // become "cancel the dialog". That decision needs an answer, not a guess.
        let mut state = State::default();
        assert!(!state.dismiss(), "nothing was open");
        state.pending = Some(purge());
        assert!(state.dismiss());
        assert_eq!(state.pending, None, "dismiss must clear it, not just report");
        assert!(!state.dismiss(), "the second Esc is the window's");
    }
}
