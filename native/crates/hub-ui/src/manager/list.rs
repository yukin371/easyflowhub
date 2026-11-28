//! One row of the list: title, preview, timestamp, and the buttons that act on it.
//!
//! Split out of `manager.rs` for a plain reason: the parent is 700 lines and the
//! 800-line ceiling is a rule, not a suggestion. The row is the densest drawing
//! code in the workbench and it shares nothing with the header or the sidebar, so
//! it is the piece that moves.

use egui::{Align2, CornerRadius, FontId, RichText, Sense, Vec2};

use crate::hint;
use crate::theme::Theme;

use super::{report, Action, Row, State};

/// One note row.
///
/// Rows are addressable even before they are selected (`row:<id>`): the buttons
/// only appear under the selected row, so a gate that can only see buttons could
/// never select one.
pub(super) fn note_row(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    row: &Row,
) -> Option<Action> {
    let mut action = None;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    // The row is addressable too: the buttons only exist once a row is selected, so
    // the gate has to be able to *select* one, and "click the third row" is exactly
    // the coordinate this file refused to hardcode.
    report(state, &format!("row:{}", row.id), rect);
    let chosen = state.selected.as_deref() == Some(row.id.as_str());
    if chosen || response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(4),
            if chosen {
                palette.surface
            } else {
                palette.surface_soft
            },
        );
    }
    if let Some(marker) = if row.deleted_at.is_some() {
        Some("回收站")
    } else if row.pinned {
        Some("置顶")
    } else {
        None
    } {
        ui.painter().text(
            egui::pos2(rect.left() + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            marker,
            FontId::proportional(9.0),
            palette.accent,
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 58.0, rect.center().y),
        Align2::LEFT_CENTER,
        &row.title,
        FontId::proportional(12.0),
        palette.ink,
    );
    if !row.preview.is_empty() {
        let start = rect.left() + 58.0 + 160.0;
        if start < rect.right() - 120.0 {
            ui.painter().text(
                egui::pos2(start, rect.center().y),
                Align2::LEFT_CENTER,
                &row.preview,
                FontId::proportional(10.5),
                palette.ink_subtle,
            );
        }
    }
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        &row.when,
        FontId::monospace(9.5),
        palette.ink_subtle,
    );

    if response.clicked() {
        // Selecting is not conditional on having an action. A trashed row has no
        // "open", and the first version therefore never set `selected` for it -- so
        // 恢复 could only ever act on the row that had just been deleted (the one
        // whose selection survived from the notes tab). Found by the gate clicking
        // a trashed row and watching nothing happen.
        //
        // The repaint is part of the fix, not a nicety: this frame already painted
        // the row with the *old* selection, and the buttons are drawn below the
        // next frame's allocation. Without asking for that frame, selecting a
        // trashed row changes nothing on screen -- which is what "nothing happened"
        // looked like in the first place.
        state.selected = Some(row.id.clone());
        ui.ctx().request_repaint();
        action = if row.deleted_at.is_some() {
            None
        } else {
            Some(Action::Open(row.id.clone()))
        };
    }
    // Actions live on the selected row's header line rather than inside every row:
    // three 16 px buttons per row is a wall of glyphs, and the list is the part
    // that has to stay readable.
    if chosen {
        ui.horizontal(|ui| {
            if row.deleted_at.is_some() {
                let restore = ui.button(RichText::new("恢复").size(10.5));
                report(state, &format!("restore:{}", row.id), restore.rect);
                if restore.clicked() {
                    action = Some(Action::Restore(row.id.clone()));
                }
                let purge = hint::hint(
                    ui.button(RichText::new("彻底删除").size(10.5)),
                    palette,
                    "从数据库删除这一行（会再确认一次）",
                    "",
                );
                report(state, &format!("purge:{}", row.id), purge.rect);
                if purge.clicked() {
                    // Not `Action::Purge`: a question is not a decision. The dialog
                    // asks, and only its yes becomes an action.
                    state.pending = Some(super::Pending::Purge {
                        id: row.id.clone(),
                        title: row.title.clone(),
                    });
                    // Same reason as the row: the dialog is a new area, and it needs
                    // a frame in which it can measure itself before it is usable.
                    ui.ctx().request_repaint();
                }
            } else {
                let open = ui.button(RichText::new("打开").size(10.5));
                report(state, &format!("open:{}", row.id), open.rect);
                if open.clicked() {
                    action = Some(Action::Open(row.id.clone()));
                }
                let pin_label = if row.pinned { "取消置顶" } else { "置顶" };
                let pin = ui.button(RichText::new(pin_label).size(10.5));
                report(state, &format!("pin:{}", row.id), pin.rect);
                if pin.clicked() {
                    action = Some(Action::Pin {
                        id: row.id.clone(),
                        pinned: !row.pinned,
                    });
                }
                let trash = ui.button(RichText::new("移到回收站").size(10.5));
                report(state, &format!("trash:{}", row.id), trash.rect);
                if trash.clicked() {
                    action = Some(Action::Trash(row.id.clone()));
                }
            }
        });
    }
    action
}
