//! Fixed chrome: the header, the query field, the empty state and the footer.
//!
//! Split from `launcher.rs` along a real seam, not a line count: everything here
//! is drawn once per frame and owns no selection state, while the list half owns
//! the cursor and returns an [`Action`](super::Action). The two also fail
//! differently -- chrome bugs are "it looks wrong", list bugs are "Enter ran the
//! wrong row" -- and only the second kind needs the tests next door.

use egui::{Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

use super::State;
use crate::theme::Theme;

pub(super) const HEADER_HEIGHT: f32 = 44.0;
pub(super) const SEARCH_HEIGHT: f32 = 42.0;
pub(super) const FOOTER_HEIGHT: f32 = 34.0;

/// Width of the accent bar on a selected row.
const BAR_WIDTH: f32 = 2.0;

/// Inset of the bar from the row's top and bottom, so it fits inside the corner.
const BAR_INSET: f32 = 9.0;

pub(super) fn header(ui: &mut egui::Ui, state: &State) {
    let palette = state.palette;
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), HEADER_HEIGHT), Sense::hover());
    let baseline = rect.center().y;

    let brand = ui.painter().layout_no_wrap(
        "easyflowhub".to_string(),
        FontId::proportional(17.0),
        palette.ink,
    );
    ui.painter().text(
        egui::pos2(rect.left(), baseline),
        Align2::LEFT_CENTER,
        "easyflowhub",
        FontId::proportional(17.0),
        palette.ink,
    );
    // Measured rather than guessed: the brand is proportional text whose width
    // depends on which face the fallback picked.
    ui.painter().text(
        egui::pos2(rect.left() + brand.size().x + 8.0, baseline + 1.0),
        Align2::LEFT_CENTER,
        "动作面板",
        FontId::proportional(12.0),
        palette.ink_subtle,
    );

    if !state.open_hint.is_empty() {
        let galley = chip_galley(ui.painter(), palette, &state.open_hint);
        let size = chip_size(&galley);
        let chip =
            Rect::from_min_size(egui::pos2(rect.right() - size.x, baseline - size.y / 2.0), size);
        paint_chip(ui.painter(), palette, chip, galley);
    }

    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, palette.border));
}

pub(super) fn search(ui: &mut egui::Ui, state: &mut State) {
    let palette = state.palette;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, SEARCH_HEIGHT), Sense::hover());
    state.layout.search = Some(rect);

    // Read before the box can take focus this frame: the outline describes the
    // frame the user is looking at, not the one being built.
    let focused = state.keyboard;
    let radius = CornerRadius::same(palette.radius.card);
    ui.painter().rect_filled(rect, radius, palette.surface);
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(
            if focused { 1.5 } else { 1.0 },
            if focused { palette.focus_ring } else { palette.border },
        ),
        StrokeKind::Inside,
    );

    // A magnifier, drawn rather than typed: the CJK fallback has no glyph for the
    // usual suspects and they come out as tofu boxes.
    let lens = egui::pos2(rect.left() + 18.0, rect.center().y - 1.0);
    let glyph = if focused {
        palette.accent
    } else {
        palette.ink_subtle
    };
    ui.painter()
        .circle_stroke(lens, 4.6, Stroke::new(1.4, glyph));
    ui.painter().line_segment(
        [lens + Vec2::new(3.2, 3.2), lens + Vec2::new(6.8, 6.8)],
        Stroke::new(1.4, glyph),
    );

    let edit_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 34.0, rect.top() + 4.0),
        egui::pos2(rect.right() - 10.0, rect.bottom() - 4.0),
    );
    let edit = egui::TextEdit::singleline(&mut state.query)
        .frame(false)
        .hint_text("搜索动作、笔记")
        .font(FontId::proportional(14.0))
        .text_color(palette.ink)
        .margin(egui::Margin::symmetric(2, 8))
        .id_source("launcher_query");
    let response = ui.put(edit_rect, edit);
    if state.take_focus {
        response.request_focus();
        state.take_focus = false;
    }
    state.keyboard = response.has_focus();
}

/// A section title with its count.
///
/// The count earns its place: "最近笔记" alone leaves the user guessing whether
/// the fifth line is the end of the list or the end of what fits.
pub(super) fn section_label(ui: &mut egui::Ui, state: &State, text: &str, count: usize) {
    let palette = state.palette;
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).size(11.0).color(palette.ink_subtle));
        ui.add_space(5.0);
        ui.label(RichText::new(count.to_string()).size(11.0).color(palette.ink_subtle));
    });
}

pub(super) fn empty(ui: &mut egui::Ui, state: &State) {
    let palette = state.palette;
    let height = 132.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter().text(
        rect.center() - Vec2::new(0.0, 11.0),
        Align2::CENTER_CENTER,
        "没有匹配的结果",
        FontId::proportional(13.5),
        palette.ink_soft,
    );
    // What to type instead. "Nothing found" on its own is a dead end in a panel
    // whose only affordance is a text box.
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 13.0),
        Align2::CENTER_CENTER,
        "试试 笔记 / 工作台 / 新建",
        FontId::proportional(11.5),
        palette.ink_subtle,
    );
}

pub(super) fn footer(ui: &mut egui::Ui, state: &State) {
    let palette = state.palette;
    // Pushed to the bottom regardless of how much the list used, so the hint does
    // not jump up and down as the query grows and shrinks the result set.
    let y = ui.max_rect().bottom() - FOOTER_HEIGHT;
    let rect = Rect::from_min_max(
        egui::pos2(ui.max_rect().left(), y),
        egui::pos2(ui.max_rect().right(), ui.max_rect().bottom()),
    );
    ui.painter()
        .hline(rect.x_range(), y, Stroke::new(1.0, palette.border));

    // Keys drawn as keys. Bare text in a row reads as a caption; a chip is what
    // makes "Enter" look pressable and tells the eye where the hint stops.
    let baseline = rect.center().y;
    let mut x = rect.left();
    for (key, label) in [("Enter", "打开"), ("Esc", "关闭")] {
        let galley = chip_galley(ui.painter(), palette, key);
        let size = chip_size(&galley);
        let chip = Rect::from_min_size(egui::pos2(x, baseline - size.y / 2.0), size);
        paint_chip(ui.painter(), palette, chip, galley);
        x = chip.right() + 6.0;

        // Measured, not a fixed step: `打开` and `关闭` are two characters wide and
        // a hard-coded advance either crowds the next chip or leaves a gap that
        // depends on the language of the label.
        let text = ui.painter().layout_no_wrap(
            label.to_string(),
            FontId::proportional(11.0),
            palette.ink_subtle,
        );
        ui.painter().text(
            egui::pos2(x, baseline),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.0),
            palette.ink_subtle,
        );
        x += text.size().x + 18.0;
    }

    ui.painter().text(
        rect.right_center(),
        Align2::RIGHT_CENTER,
        "方向键 选择",
        FontId::proportional(11.0),
        palette.ink_subtle,
    );
}

/// The 2 px accent bar marking the row Enter will run.
pub(super) fn accent_bar(painter: &egui::Painter, palette: Theme, rect: Rect) {
    let bar = Rect::from_min_max(
        egui::pos2(rect.left() + 1.0, rect.top() + BAR_INSET),
        egui::pos2(
            rect.left() + 1.0 + BAR_WIDTH,
            rect.bottom() - BAR_INSET,
        ),
    );
    painter.rect_filled(bar, CornerRadius::same(1), palette.accent);
}

fn chip_galley(painter: &egui::Painter, palette: Theme, key: &str) -> std::sync::Arc<egui::Galley> {
    painter.layout_no_wrap(key.to_string(), FontId::monospace(10.0), palette.ink_soft)
}

fn chip_size(galley: &egui::Galley) -> Vec2 {
    galley.size() + Vec2::new(12.0, 5.0)
}

fn paint_chip(
    painter: &egui::Painter,
    palette: Theme,
    rect: Rect,
    galley: std::sync::Arc<egui::Galley>,
) {
    let radius = CornerRadius::same(palette.radius.chip);
    painter.rect_filled(rect, radius, palette.surface);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, palette.border),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + Vec2::new(6.0, 2.5), galley, palette.ink_soft);
}
