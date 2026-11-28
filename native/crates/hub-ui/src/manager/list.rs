//! One row of the list: title, preview, timestamp, and the buttons that act on it.
//!
//! Split out of `manager.rs` for a plain reason: the parent is 700 lines and the
//! 800-line ceiling is a rule, not a suggestion. The row is the densest drawing
//! code in the workbench and it shares nothing with the header or the sidebar, so
//! it is the piece that moves.
//!
//! ## Two lines, not one
//!
//! The row used to be title and preview side by side on a 34 px line, which meant
//! the title column had a fixed 160 px and a long title simply ran into the
//! preview. The shipped list stacks them: title on top, one line of body under
//! it, the timestamp at the far right. That is also why the row is 48 px now -- a
//! single line cannot hold two lines of text.
//!
//! ## One hit area, two behaviours
//!
//! A trashed row also has to be *picked* (for the batch restore) as well as
//! *selected* (which reveals 恢复 / 彻底删除 under it). Two overlapping click
//! regions would make that ambiguous -- egui would hand the click to whichever was
//! registered last. So the row keeps one `Response` and splits it by x: the left
//! column is the checkbox, everything else selects. Picking does not select on
//! purpose: a selection reveals a button strip, which moves every row below it, and
//! a second tick aimed from the old layout would then land on the wrong row.

use egui::{Align2, CornerRadius, FontId, RichText, Sense, Vec2};

use crate::hint;
use crate::theme::Theme;

use super::check::{check_box, BOX};
use super::{report, Action, Row, State};

/// Row height: two lines of text plus 12 px of air on each side.
const HEIGHT: f32 = 48.0;

/// Where the title column starts. The marker column holds "置顶" / the checkbox; the
/// widest of them is three characters at 9 px, which fits in 44.
const TEXT_X: f32 = 56.0;

/// Width of the checkbox target inside the marker column. The whole column is the
/// target -- 14 px of drawn square is not a 14 px control.
const CHECK_COLUMN: f32 = 30.0;

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
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::click());
    // The row is addressable too: the buttons only exist once a row is selected, so
    // the gate has to be able to *select* one, and "click the third row" is exactly
    // the coordinate this file refused to hardcode.
    report(state, &format!("row:{}", row.id), rect);
    let chosen = state.selected.as_deref() == Some(row.id.as_str());
    let radius = CornerRadius::same(palette.radius.row);
    if chosen || response.hovered() {
        ui.painter().rect_filled(
            rect,
            radius,
            if chosen {
                palette.surface
            } else {
                palette.surface_soft
            },
        );
    }
    if chosen {
        // A 3 px accent bar on the selected row: the fill alone is one step off the
        // page, and on a light theme that step is easy to miss.
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + 10.0),
                egui::vec2(3.0, rect.height() - 20.0),
            ),
            CornerRadius::same(2),
            palette.accent,
        );
    }

    if let Some((marker, ink)) = if row.deleted_at.is_some() {
        // No "回收站" label in the marker column any more: every row on this tab is
        // in the trash, and the column is needed for the checkbox that picks rows for
        // the batch restore.
        None
    } else if row.pinned {
        Some(("置顶", palette.accent))
    } else {
        None
    } {
        ui.painter().text(
            egui::pos2(rect.left() + 14.0, rect.center().y),
            Align2::LEFT_CENTER,
            marker,
            FontId::proportional(9.0),
            ink,
        );
    }
    if row.deleted_at.is_some() {
        let target = egui::Rect::from_min_size(rect.min, Vec2::new(CHECK_COLUMN, HEIGHT));
        report(state, &format!("check:{}", row.id), target);
        let boxed = egui::Rect::from_center_size(
            egui::pos2(target.left() + 15.0, target.center().y),
            Vec2::splat(BOX),
        );
        check_box(ui, palette, boxed, state.checked.contains(&row.id));
    }

    let text_x = rect.left() + TEXT_X;
    let title_y = if row.preview.is_empty() {
        rect.center().y
    } else {
        rect.center().y - 8.0
    };
    ui.painter().text(
        egui::pos2(text_x, title_y),
        Align2::LEFT_CENTER,
        &row.title,
        FontId::proportional(12.5),
        if row.deleted_at.is_some() {
            palette.ink_soft
        } else {
            palette.ink
        },
    );
    if !row.preview.is_empty() {
        // Clipped at the timestamp column rather than measured: the painter does
        // not wrap, and a preview that runs under the time reads as two strings
        // colliding instead of one truncated line.
        let clip = egui::Rect::from_min_max(
            egui::pos2(text_x, rect.top() + 2.0),
            egui::pos2(rect.right() - 66.0, rect.bottom() - 2.0),
        );
        ui.painter().with_clip_rect(clip).text(
            egui::pos2(text_x, rect.center().y + 8.0),
            Align2::LEFT_CENTER,
            &row.preview,
            FontId::proportional(10.0),
            palette.ink_subtle,
        );
    }
    ui.painter().text(
        egui::pos2(rect.right() - 10.0, rect.center().y),
        Align2::RIGHT_CENTER,
        &row.when,
        FontId::monospace(9.5),
        palette.ink_subtle,
    );

    if response.clicked() {
        // The checkbox column is a different verb from the row, and the split is by
        // x inside this one hit area (see the module comment). Picking a row for the
        // batch must not also select it: the button strip that selection reveals
        // moves every row below, and the second tick of a two-row batch would land on
        // whatever moved into that place.
        let on_box = response
            .interact_pointer_pos()
            .map(|at| at.x < rect.left() + CHECK_COLUMN)
            .unwrap_or(false);
        if row.deleted_at.is_some() && on_box {
            if !state.checked.remove(&row.id) {
                state.checked.insert(row.id.clone());
            }
            ui.ctx().request_repaint();
        } else {
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
    }
    // Actions live on the selected row's header line rather than inside every row:
    // three 16 px buttons per row is a wall of glyphs, and the list is the part
    // that has to stay readable.
    if chosen {
        // Four pixels above and below: the button row is a third band of the row
        // and without the gap it reads as the next row's first line.
        ui.add_space(2.0);
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
        ui.add_space(4.0);
    }
    action
}
