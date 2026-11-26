//! The "unmerged backup" card (parity Q24).
//!
//! Three answers to one question: keep what is in the editor, put the draft in
//! its place, or append the draft to it. The card shows both bodies side by side
//! so the choice is made on the text rather than on a filename.
//!
//! ## Drawn by hand, not laid out by egui
//!
//! Every rectangle here is arithmetic on the card's own size, and [`draw`] returns
//! the three button rectangles. That looks like re-implementing a layout engine,
//! and it is: a gate has to click those buttons, and the only way to click them is
//! to know where they are. Reading the positions back from the same function that
//! painted them removes the alternative -- a gate that hard-codes coordinates and
//! goes red when a font gets 2 px wider, for a reason unrelated to the behaviour it
//! is checking.
//!
//! The card size is fixed for the same reason. Text that reflows the card would
//! move the buttons between the frame the gate read and the frame it clicked.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::theme::Palette;

/// Logical size of the card, before clamping to the window.
pub const CARD: Vec2 = Vec2::new(560.0, 340.0);
const PAD: f32 = 20.0;
const HEAD: f32 = 84.0;
const COLUMN_GAP: f32 = 12.0;
const BUTTON: Vec2 = Vec2::new(126.0, 34.0);
const GAP: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    /// 使用当前版本: drop the draft, leave the document alone.
    Keep,
    /// 合并到当前内容: append the draft body, union the tags.
    Merge,
    /// 恢复备份版本: the draft becomes the document.
    Restore,
}

/// What was drawn, including where the clickable parts are.
#[derive(Debug, Clone, Copy)]
pub struct Shown {
    pub action: Action,
    /// `[keep, merge, restore]`, in screen coordinates.
    pub buttons: [Rect; 3],
    pub card: Rect,
}

/// Draw the card centred in `area`, dimming `screen` (the whole window).
///
/// `when` is the human-readable backup time. It is passed as text rather than as a
/// timestamp so this crate keeps knowing nothing about clocks or file formats.
pub fn draw(
    ctx: &egui::Context,
    palette: Palette,
    screen: Rect,
    area: Rect,
    when: &str,
    current: &str,
    backup: &str,
) -> Shown {
    let size = Vec2::new(
        CARD.x.min((area.width() - 24.0).max(360.0)),
        CARD.y.min((area.height() - 24.0).max(220.0)),
    );
    let card = Rect::from_center_size(area.center(), size);

    // The row is right-aligned with the primary action at the end, and the three
    // buttons share what is left. A fixed 126 px each would run off the card the
    // moment the window is narrower than the card -- measured: the "keep" button
    // landed at x = -45 on the first frame, i.e. off screen and unclickable.
    let button_w = ((size.x - 2.0 * PAD - 2.0 * GAP) / 3.0).min(BUTTON.x);
    let cell = Vec2::new(button_w, BUTTON.y);
    let row_y = card.bottom() - PAD - cell.y;
    let restore = Rect::from_min_size(egui::pos2(card.right() - PAD - cell.x, row_y), cell);
    let merge = restore.translate(Vec2::new(-(cell.x + GAP), 0.0));
    let keep = merge.translate(Vec2::new(-(cell.x + GAP), 0.0));

    let mut action = Action::None;

    egui::Area::new(egui::Id::new("note-recover"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let painter = ui.painter();

            // The scrim is an interaction surface, not just a shade: clicking the
            // note behind the card must not reach the strip's buttons. It covers the
            // whole window, not just the body -- the strip has a close button and a
            // pin, and a modal that lets those through is not modal.
            painter.rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(96));
            let _scrim = ui.interact(screen, ui.id().with("scrim"), Sense::click());

            painter.rect_filled(card, CornerRadius::same(10), palette.panel);
            painter.rect_stroke(
                card,
                CornerRadius::same(10),
                Stroke::new(1.0, palette.border),
                StrokeKind::Inside,
            );

            let left = card.left() + PAD;
            painter.text(
                egui::pos2(left, card.top() + PAD),
                Align2::LEFT_TOP,
                "BACKUP RESTORE",
                FontId::proportional(9.5),
                palette.ink_subtle,
            );
            painter.text(
                egui::pos2(left, card.top() + PAD + 18.0),
                Align2::LEFT_TOP,
                "检测到未恢复的本地备份",
                FontId::proportional(17.0),
                palette.ink,
            );
            painter.text(
                egui::pos2(left, card.top() + PAD + 44.0),
                Align2::LEFT_TOP,
                when,
                FontId::proportional(11.0),
                palette.ink_soft,
            );

            // Two columns of body text. The clip rect does the truncation: a note
            // of ten thousand characters must not push the buttons off the card,
            // and the point of the card is the choice, not reading the whole note.
            let top = card.top() + HEAD;
            let height = (row_y - 12.0 - top).max(40.0);
            let width = (card.width() - 2.0 * PAD - COLUMN_GAP) / 2.0;
            for (index, (label, body)) in [("当前", current), ("备份", backup)]
                .into_iter()
                .enumerate()
            {
                let min = egui::pos2(left + index as f32 * (width + COLUMN_GAP), top);
                let column = Rect::from_min_size(min, Vec2::new(width, height));
                // A recessed panel per side: two blocks of text side by side with
                // nothing between them read as one paragraph with a gap.
                painter.rect_filled(
                    column,
                    CornerRadius::same(6),
                    palette.panel_strong.gamma_multiply(0.55),
                );
                painter.text(
                    column.left_top() + Vec2::new(9.0, 7.0),
                    Align2::LEFT_TOP,
                    label,
                    FontId::proportional(10.0),
                    palette.ink_subtle,
                );
                let body_rect = Rect::from_min_max(
                    column.min + Vec2::new(9.0, 24.0),
                    column.max - Vec2::new(9.0, 8.0),
                );
                let text = if body.trim().is_empty() { "（无正文）" } else { body };
                let galley = painter.layout(
                    text.to_string(),
                    FontId::proportional(12.0),
                    palette.ink_soft,
                    body_rect.width(),
                );
                painter
                    .with_clip_rect(body_rect)
                    .galley(body_rect.min, galley, palette.ink_soft);
            }

            if button(ui, palette, keep, "使用当前版本", false) {
                action = Action::Keep;
            }
            if button(ui, palette, merge, "合并到当前内容", false) {
                action = Action::Merge;
            }
            if button(ui, palette, restore, "恢复备份版本", true) {
                action = Action::Restore;
            }
        });

    Shown {
        action,
        buttons: [keep, merge, restore],
        card,
    }
}

/// One outlined button; `primary` fills it with a wash of the accent.
///
/// A wash rather than a solid accent: the note's palette is the user's (it can be
/// a light theme), and solid accent with light text is unreadable there. The wash
/// keeps the label in ink and still says which button is the default.
fn button(ui: &mut egui::Ui, palette: Palette, rect: Rect, label: &str, primary: bool) -> bool {
    let response = ui.interact(rect, ui.id().with(("recover", label)), Sense::click());
    let painter = ui.painter();

    if primary || response.hovered() {
        let fill = if primary {
            palette.accent.gamma_multiply(0.28)
        } else {
            palette.panel_strong
        };
        painter.rect_filled(rect, CornerRadius::same(17), fill);
    }
    painter.rect_stroke(
        rect,
        CornerRadius::same(17),
        Stroke::new(
            1.0,
            if primary {
                palette.accent
            } else if response.hovered() {
                palette.ink_subtle
            } else {
                palette.border
            },
        ),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        if primary { palette.ink } else { palette.ink_soft },
    );
    response.clicked()
}
