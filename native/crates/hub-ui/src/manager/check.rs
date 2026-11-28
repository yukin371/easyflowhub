//! The one checkbox shape.
//!
//! Two places tick things now: the todo pane (a tick is written back to a note the
//! task lives in) and the trash (a tick picks a row for a batch restore). They are
//! different operations on different data, but the *control* must be the same square
//! -- two boxes that disagree by a pixel make the window look like two products.
//!
//! ## Why it is drawn and not a glyph
//!
//! `✓` was the first version and it landed as a smudge: at 9–10 px the character is
//! whatever face covers U+2713, its strokes are one pixel, and its baseline does not
//! line up with a box this size. Two `line_segment`s in `on_accent` are the same
//! shape on every DPI and in every font.

use egui::{CornerRadius, Stroke, StrokeKind};

use crate::theme::Theme;

/// Side of the drawn square, in points.
pub(super) const BOX: f32 = 14.0;

/// Paint a checkbox. The caller owns the target: this draws inside `boxed` only.
///
/// The unchecked outline brightens under the pointer, because the square is the only
/// part of the row that looks like a control and a control that ignores the pointer
/// reads as decoration.
pub(super) fn check_box(ui: &egui::Ui, palette: Theme, boxed: egui::Rect, checked: bool) {
    let radius = CornerRadius::same(4);
    let painter = ui.painter();
    if checked {
        painter.rect_filled(boxed, radius, palette.accent);
        let (cx, cy) = (boxed.center().x, boxed.center().y);
        let pen = Stroke::new(1.6, palette.on_accent);
        painter.line_segment(
            [egui::pos2(cx - 3.5, cy), egui::pos2(cx - 1.0, cy + 2.5)],
            pen,
        );
        painter.line_segment(
            [egui::pos2(cx - 1.0, cy + 2.5), egui::pos2(cx + 4.0, cy - 3.0)],
            pen,
        );
    } else {
        let line = if ui.rect_contains_pointer(boxed) {
            palette.accent
        } else {
            palette.border_strong
        };
        painter.rect_stroke(boxed, radius, Stroke::new(1.0, line), StrokeKind::Inside);
    }
}
