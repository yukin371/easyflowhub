//! The trash pane: its sub-tabs, and the bar that acts on a multi-row pick.
//!
//! ## Why the sub-tabs exist when only one of them has a list
//!
//! The shipped panel's trash holds three kinds -- notes, captured items, scripts --
//! because the Tauri manager owns all three. The native workbench only owns notes,
//! so the other two chips lead to a sentence that says exactly that. A dimmed chip
//! would say the same thing without the click; a chip that does nothing at all is
//! the one shape this window refuses, because "I pressed it and nothing happened" is
//! indistinguishable from a bug. The click here *does* answer the question it raises
//! ("where did my deleted capture go?"), and the answer is not "try again".
//!
//! ## Why a selection bar and not a header button
//!
//! 恢复选中 needs a count, and a count that lives in the header changes the header's
//! width every time a row is ticked. The bar appears between the search field and the
//! list, where the rows it acts on are, and it disappears when nothing is picked --
//! including after the restore it just performed.

use egui::{CornerRadius, RichText, Stroke, Vec2};

use crate::theme::Theme;

use super::head;
use super::{report, Action, State};

/// The three kinds of thing the shipped trash can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrashView {
    #[default]
    Notes,
    Inbox,
    Scripts,
}

impl TrashView {
    pub const ALL: [TrashView; 3] = [TrashView::Notes, TrashView::Inbox, TrashView::Scripts];

    /// Stable id: it ends up in the log and in reported hit names (`trash-view:inbox`).
    pub fn id(self) -> &'static str {
        match self {
            TrashView::Notes => "notes",
            TrashView::Inbox => "inbox",
            TrashView::Scripts => "scripts",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TrashView::Notes => "笔记",
            TrashView::Inbox => "收集箱",
            TrashView::Scripts => "脚本",
        }
    }

    /// Why this view has no list in the native shell, if it has none.
    ///
    /// Separate from [`TrashView::has_list`] so the sentence lives next to the label
    /// it belongs to instead of in the drawing code, where it would read as "the
    /// list is empty".
    pub fn notice(self) -> Option<&'static str> {
        match self {
            TrashView::Notes => None,
            TrashView::Inbox => Some("收集箱条目的回收站在 Tauri 版管理器里；原生工作台只处理笔记。"),
            TrashView::Scripts => Some("脚本的回收站在 Tauri 版管理器里；原生工作台只处理笔记。"),
        }
    }

    pub fn has_list(self) -> bool {
        self.notice().is_none()
    }
}

/// The chip row under the panel header.
pub(super) fn subnav(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    notes: usize,
) -> Option<Action> {
    let mut asked = None;
    ui.horizontal(|ui| {
        for view in TrashView::ALL {
            let chosen = state.trash_view == view;
            let label = if view == TrashView::Notes {
                format!("{} {notes}", view.label())
            } else {
                view.label().to_string()
            };
            let chip = chip(ui, palette, &label, chosen);
            report(state, &format!("trash-view:{}", view.id()), chip.rect);
            if chip.clicked() && !chosen {
                state.trash_view = view;
                asked = Some(Action::SetTrashView(view));
                // The fill was decided before the click was known and this frame has
                // already painted the old selection; same reason the todo filter asks.
                ui.ctx().request_repaint();
            }
        }
    });
    asked
}

/// 已选 N 条 · 恢复选中 · 取消选择, when something is picked.
pub(super) fn selection_bar(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
) -> Option<Action> {
    if state.checked.is_empty() {
        return None;
    }
    let mut asked = None;
    let count = state.checked.len();
    egui::Frame::NONE
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(CornerRadius::same(palette.radius.card))
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("已选 {count} 条"))
                        .size(10.5)
                        .color(palette.ink_soft),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let restore = head::primary(ui, palette, "恢复选中");
                    report(state, "restore-selected", restore.rect);
                    if restore.clicked() {
                        // The ids travel with the action: the controller never reads
                        // view state, and the log then names exactly what was asked
                        // for instead of "whatever was ticked at the time".
                        asked = Some(Action::RestoreSelected(
                            state.checked.iter().cloned().collect(),
                        ));
                    }
                    let clear = head::ghost(ui, palette, "取消选择");
                    report(state, "clear-selection", clear.rect);
                    if clear.clicked() {
                        state.checked.clear();
                        // Nothing else repaints: the bar and every box just changed.
                        ui.ctx().request_repaint();
                    }
                });
            });
        });
    asked
}

/// The sentence a sub-tab without a list shows instead of one.
pub(super) fn notice(ui: &mut egui::Ui, palette: Theme, state: &mut State, view: TrashView) {
    ui.add_space(12.0);
    if let Some(text) = view.notice() {
        let label = ui.label(RichText::new(text).size(11.0).color(palette.ink_subtle));
        // Reported so the gate can tell "the notice replaced the list" from "the list
        // happened to be empty" -- both look the same in a screenshot.
        report(state, "trash-notice", label.rect);
    }
}

fn chip(ui: &mut egui::Ui, palette: Theme, label: &str, chosen: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(10.0)
                .color(if chosen { palette.ink } else { palette.ink_soft }),
        )
        .fill(if chosen {
            palette.accent_soft
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(Stroke::new(
            1.0,
            if chosen {
                palette.accent
            } else {
                palette.border
            },
        ))
        .corner_radius(CornerRadius::same(palette.radius.chip))
        .min_size(Vec2::new(0.0, 24.0)),
    )
}
