//! The panel header: kicker, title, one line of description, and the actions
//! that belong to the section.
//!
//! ## Why this is a module and not four lines in `draw`
//!
//! The workbench used to have one global strip -- "工作台", the status line and two
//! buttons -- and every section had to fit its own verb into the body. That is
//! why the todo tab grew a "悬浮卡片" button wedged next to a count. The shipped
//! manager gives each panel a header with a title, a sentence saying what the
//! panel is, and its actions on the right; the body is then free to be a body.
//!
//! The header also draws the hairline under itself, so the separation between
//! "what this is" and "the list" is one decision in one place instead of a
//! `separator()` in every branch.

use egui::{CornerRadius, RichText, Stroke, Vec2};

use crate::theme::Theme;

use super::{Action, State};

/// Draw the header and return whatever its action buttons asked for.
///
/// `right` is given the state as well as the ui because its buttons have to
/// report their rectangles (`new-note`, `todo-card`), and reporting is a write to
/// `state.hits`. Passing it here keeps the borrow honest: the closure cannot
/// touch the state that `panel_head` is already holding.
pub(super) fn panel_head(
    ui: &mut egui::Ui,
    palette: Theme,
    state: &mut State,
    kicker: &str,
    title: &str,
    description: &str,
    right: impl FnOnce(&mut egui::Ui, &mut State) -> Action,
) -> Action {
    let mut action = Action::None;
    let width = ui.available_width();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            // Leave the action cluster its own column. Without a cap the title
            // pushes the buttons off the edge on a narrow window, and a button
            // that scrolls out of the header is a button nobody finds.
            ui.set_max_width((width - 190.0).max(160.0));
            ui.label(
                RichText::new(kicker)
                    .size(8.5)
                    .color(palette.ink_subtle)
                    .extra_letter_spacing(2.4),
            );
            ui.add_space(3.0);
            ui.label(RichText::new(title).size(20.0).color(palette.ink));
            if !description.is_empty() {
                ui.add_space(2.0);
                ui.label(
                    RichText::new(description)
                        .size(10.5)
                        .color(palette.ink_soft),
                );
            }
        });
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Min),
            |ui| action = right(ui, state),
        );
    });

    ui.add_space(12.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter()
        .hline(rect.x_range(), rect.top(), Stroke::new(1.0, palette.border));
    ui.add_space(12.0);
    action
}

/// A button with no fill: "关闭" and other things that are always available but
/// never the point of the panel.
pub(super) fn ghost(ui: &mut egui::Ui, palette: Theme, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(10.5).color(palette.ink_soft))
            .fill(egui::Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(palette.radius.row))
            .min_size(Vec2::new(0.0, 26.0)),
    )
}

/// A button that carries its own surface: the second choice of two.
pub(super) fn quiet(ui: &mut egui::Ui, palette: Theme, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(10.5).color(palette.ink_soft))
            .fill(palette.surface)
            .stroke(Stroke::new(1.0, palette.border))
            .corner_radius(CornerRadius::same(palette.radius.row))
            .min_size(Vec2::new(0.0, 26.0)),
    )
}

/// A quiet button that leads to something irreversible: 清空回收站.
///
/// Not the filled danger button the dialog's yes uses. The header should not look
/// like it is asking a question -- the dialog does that -- but "this one deletes
/// things" has to be readable before it is pressed, so it carries the danger ink.
pub(super) fn warn(ui: &mut egui::Ui, palette: Theme, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(10.5).color(palette.danger))
            .fill(palette.surface)
            .stroke(Stroke::new(1.0, palette.border))
            .corner_radius(CornerRadius::same(palette.radius.row))
            .min_size(Vec2::new(0.0, 26.0)),
    )
}

/// The one filled button in a panel.
pub(super) fn primary(ui: &mut egui::Ui, palette: Theme, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(10.5).color(palette.on_accent))
            .fill(palette.accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(palette.radius.row))
            .min_size(Vec2::new(0.0, 26.0)),
    )
}
