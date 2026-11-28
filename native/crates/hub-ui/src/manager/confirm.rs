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

use egui::{Align, Layout, RichText, Stroke};

use crate::theme::Theme;

use super::{report, Action, State};

/// A destructive action that has been asked for and not yet answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// Permanent: the row leaves the database. Restoring it is not possible, which
    /// is why this is the one action that asks.
    Purge { id: String, title: String },
}

impl Pending {
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Purge { .. } => "purge",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Purge { id, .. } => id,
        }
    }

    fn question(&self) -> &'static str {
        match self {
            Self::Purge { .. } => "彻底删除这条笔记？",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Purge { title, .. } => {
                format!("「{title}」会从数据库删除，回收站也找不回来。")
            }
        }
    }

    fn yes(&self) -> &'static str {
        match self {
            Self::Purge { .. } => "彻底删除",
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
            Action::Purge(pending.id().to_string())
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
            action = draw(ctx, Theme::standard(), state, rows, &[], &table);
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
        assert_eq!(
            state.pending,
            Some(Pending::Purge {
                id: "gone".to_string(),
                title: "删掉的".to_string()
            }),
            "the dialog must be open"
        );
    }

    #[test]
    fn the_second_yes_is_the_one_that_purges() {
        let ctx = egui::Context::default();
        let rows = trashed();
        let mut state = State {
            tab: Tab::Trash,
            selected: Some("gone".to_string()),
            pending: Some(Pending::Purge {
                id: "gone".to_string(),
                title: "删掉的".to_string(),
            }),
            ..Default::default()
        };
        // The dialog is drawn this frame, so its buttons are known after it settles.
        let at = settle(&ctx, &mut state, &rows, "confirm-ok").center();

        let action = pass(&ctx, &mut state, &rows, click(at));
        assert_eq!(action, Action::Purge("gone".to_string()));
        assert_eq!(state.pending, None, "answering closes the dialog");
    }

    #[test]
    fn cancelling_keeps_the_row_and_closes_the_dialog() {
        let ctx = egui::Context::default();
        let rows = trashed();
        let mut state = State {
            tab: Tab::Trash,
            selected: Some("gone".to_string()),
            pending: Some(Pending::Purge {
                id: "gone".to_string(),
                title: "删掉的".to_string(),
            }),
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
        state.pending = Some(Pending::Purge {
            id: "gone".to_string(),
            title: "删掉的".to_string(),
        });
        assert!(state.dismiss());
        assert_eq!(state.pending, None, "dismiss must clear it, not just report");
        assert!(!state.dismiss(), "the second Esc is the window's");
    }
}
