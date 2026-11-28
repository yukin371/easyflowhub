//! The trash pane: its three sub-tabs, and the bar that acts on a multi-row pick.
//!
//! ## Why three sub-tabs and not one merged list
//!
//! The three kinds are restored by three different writes -- an `UPDATE` on
//! `notes`, an `UPDATE` on `inbox_items`, and a set of file moves out of
//! `%APPDATA%\scriptmgr\trash\scripts`. A merged list would have to answer "which
//! one is this row" on every line, and the answer would then be wrong exactly once,
//! in the code path that deletes things. The chip is the answer, asked once.
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
use super::{report, Action, RowKind, State};

/// The three kinds of thing the trash holds.
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

    /// What the rows on this sub-tab came from. An action carries this, so the
    /// controller never has to guess which store to write.
    pub fn kind(self) -> RowKind {
        match self {
            TrashView::Notes => RowKind::Note,
            TrashView::Inbox => RowKind::Inbox,
            TrashView::Scripts => RowKind::Script,
        }
    }

    /// The sentence under the panel title.
    ///
    /// Per view rather than one shared line, because "保留 30 天" is the only part
    /// they share: what a restore does with the thing is the difference, and it is
    /// the part a user needs before pressing it.
    pub fn summary(self) -> &'static str {
        match self {
            TrashView::Notes => "删除的笔记保留 30 天；勾选多条可以一次恢复。",
            TrashView::Inbox => "删除的收集项保留 30 天；彻底删除会连附件一起清理。",
            TrashView::Scripts => "删除的脚本保留 30 天；恢复会把文件放回原位置。",
        }
    }

    /// The destructive header button's label for this sub-tab.
    ///
    /// It names the kind: on 收集箱 a button called 清空回收站 reads as "empty
    /// everything", and the only thing it actually empties is the list under it.
    pub fn empty_label(self) -> &'static str {
        match self {
            TrashView::Notes => "清空回收站",
            TrashView::Inbox => "清空收集箱",
            TrashView::Scripts => "清空脚本",
        }
    }

    /// What the search field says it can find here.
    ///
    /// Per view because the three kinds do not carry the same fields: a script row
    /// is a name and an original path, so the notes' hint would promise a body and
    /// a tag list that are not on screen -- and `Row::script` would then be accused
    /// of not searching them.
    pub fn search_hint(self) -> &'static str {
        match self {
            TrashView::Notes => "搜索标题、正文、标签",
            TrashView::Inbox => "搜索标题、正文、类型",
            TrashView::Scripts => "搜索脚本名或原路径",
        }
    }

    /// What the list says when it has no rows.
    pub fn empty_line(self) -> &'static str {
        match self {
            TrashView::Notes => "回收站是空的 · 删除的笔记会在这里保留 30 天",
            TrashView::Inbox => "没有已删除的收集项 · 从收集箱删掉的内容会出现在这里",
            TrashView::Scripts => "没有已删除的脚本 · 在脚本工作台里删掉的脚本会出现在这里",
        }
    }
}

/// The chip row under the panel header. `counts` is in [`TrashView::ALL`] order.
pub(super) fn subnav(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    counts: [usize; 3],
) -> Option<Action> {
    let mut asked = None;
    ui.horizontal(|ui| {
        for (index, view) in TrashView::ALL.into_iter().enumerate() {
            let chosen = state.trash_view == view;
            let label = format!("{} {}", view.label(), counts[index]);
            let chip = chip(ui, palette, &label, chosen);
            report(state, &format!("trash-view:{}", view.id()), chip.rect);
            if chip.clicked() && !chosen {
                state.trash_view = view;
                // A pick belongs to the list it was made in. Carrying it across
                // kinds would make 恢复选中 act on ids the current list cannot show,
                // so the set is dropped here as well as pruned by the pane.
                state.checked.clear();
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
    let kind = state.trash_view.kind();
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
                        asked = Some(Action::RestoreSelected {
                            kind,
                            ids: state.checked.iter().cloned().collect(),
                        });
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

/// What an empty list says.
///
/// A function rather than three branches inside the drawing code, for two reasons:
/// the sentence is the one thing a screenshot cannot prove (a test can compare it),
/// and the notes case has to read the retention number from the same constant the
/// header does -- two copies of "30" is one of them being wrong.
pub(super) fn empty_text(view: TrashView, scripts_note: Option<&str>) -> String {
    match (view, scripts_note) {
        (TrashView::Scripts, Some(note)) => note.to_string(),
        (TrashView::Notes, _) => format!(
            "回收站是空的 · 删除的笔记会在这里保留 {} 天",
            hub_core::store::TRASH_RETENTION_DAYS
        ),
        _ => view.empty_line().to_string(),
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
