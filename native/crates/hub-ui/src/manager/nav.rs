//! The sidebar: the brand block, the three workspace sections, and settings
//! pinned to the bottom.
//!
//! ## Why the rows are hand-painted
//!
//! The previous rail was four `add_sized` text buttons. Nothing told the open
//! section apart except a fill, and every row read as "a line of text you can
//! click". The shipped manager gives each row an icon tile, a name and a caption
//! in small caps, so the rail says *where you are* before you read the words.
//! `egui::Button` lays out one label inside one hit area and cannot hold a tile
//! plus two lines, so the row is `allocate_exact_size` plus a painter. The click
//! still comes from the `Response`, which is what keeps the gate contract intact:
//! the row reports `tab:<id>` and the gate clicks the centre of that rectangle,
//! so moving the row does not move the script.

use egui::{Align2, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Vec2};

use crate::theme::Theme;

use super::{report, Action, State, Tab};

/// The rail is 216 px wide: a quarter of the 860 px window, which leaves the
/// content column the larger half. It used to be 132, enough for four words.
pub(super) const WIDTH: f32 = 216.0;

/// Row height. Two lines of text need 44; the single-line row was 28.
const ROW: f32 = 44.0;

/// Sidebar in, at most one action out.
pub(super) fn sidebar(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    notes: usize,
    trash: usize,
) -> Action {
    let mut action = Action::None;

    ui.label(
        RichText::new("EASYFLOWHUB")
            .size(8.5)
            .color(palette.ink_subtle)
            .extra_letter_spacing(2.6),
    );
    ui.add_space(3.0);
    ui.label(RichText::new("管理中心").size(19.0).color(palette.ink));
    ui.add_space(3.0);
    ui.label(
        RichText::new("整理笔记、待办与配置。")
            .size(10.5)
            .color(palette.ink_soft),
    );
    ui.add_space(18.0);
    ui.label(
        RichText::new("工作区")
            .size(8.5)
            .color(palette.ink_subtle)
            .extra_letter_spacing(2.6),
    );
    ui.add_space(6.0);

    for (tab, count) in [
        (Tab::Notes, Some(notes)),
        (Tab::Todos, None),
        (Tab::Trash, Some(trash)),
    ] {
        if row(ui, palette, state, tab, count) {
            action = Action::SetTab(tab);
        }
        ui.add_space(2.0);
    }

    // Settings sits at the bottom, one click from anywhere -- where the shipped
    // sidebar pins it. `bottom_up` means the first thing added is the lowest, so
    // the caption goes in first and the row above it.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        ui.label(
            RichText::new("ZEN LAYOUT")
                .size(8.0)
                .color(palette.ink_subtle)
                .extra_letter_spacing(2.0),
        );
        ui.add_space(8.0);
        if row(ui, palette, state, Tab::Settings, None) {
            action = Action::SetTab(Tab::Settings);
        }
    });

    action
}

/// One nav row. Returns whether it was clicked; the caller turns that into an
/// `Action` so this file never decides what a tab switch *means*.
fn row(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    tab: Tab,
    count: Option<usize>,
) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW), Sense::click());
    report(state, &format!("tab:{}", tab.id()), rect);

    let selected = state.tab == tab;
    let radius = CornerRadius::same(palette.radius.card);
    if selected {
        ui.painter()
            .rect_filled(rect, radius, palette.accent_soft);
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, palette.accent),
            StrokeKind::Inside,
        );
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, radius, palette.surface_soft);
    }

    // The tile. A filled accent square is the fastest thing on the rail to read,
    // and it is the only element that carries "you are here" without text.
    let tile = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 8.0 + 15.0, rect.center().y),
        Vec2::splat(30.0),
    );
    ui.painter().rect_filled(
        tile,
        CornerRadius::same(10),
        if selected {
            palette.accent
        } else {
            palette.canvas
        },
    );
    ui.painter().text(
        tile.center(),
        Align2::CENTER_CENTER,
        tab.glyph(),
        FontId::proportional(13.0),
        if selected {
            palette.on_accent
        } else {
            palette.ink_soft
        },
    );

    let text_x = tile.right() + 10.0;
    ui.painter().text(
        egui::pos2(text_x, rect.center().y - 7.5),
        Align2::LEFT_CENTER,
        tab.label(),
        FontId::proportional(12.5),
        if selected {
            palette.ink
        } else {
            palette.ink_soft
        },
    );
    ui.painter().text(
        egui::pos2(text_x, rect.center().y + 8.0),
        Align2::LEFT_CENTER,
        tab.caption(),
        FontId::proportional(8.0),
        palette.ink_subtle,
    );
    if let Some(count) = count {
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            count.to_string(),
            FontId::monospace(9.5),
            palette.ink_subtle,
        );
    }

    if response.clicked() {
        // Switching happens *here*, not in the controller: the content panel is
        // drawn after this one in the same frame, so setting the tab now is what
        // makes the click feel immediate. The repaint covers the rail itself,
        // which already painted the old highlight.
        state.tab = tab;
        state.query.clear();
        state.status.clear();
        ui.ctx().request_repaint();
        true
    } else {
        false
    }
}
