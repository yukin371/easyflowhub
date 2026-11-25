//! Quick-note shell: the 32 px title strip, and the hole left for the text island.
//!
//! ## Why the strip is drawn by hand
//!
//! Parity Q11 ("the three slots never overlap") is a **structural** property, not
//! something to eyeball. The shipped toolbar was reworked once because its middle
//! button was absolutely centred and covered the left icon in narrow windows. So
//! the slot boundaries come from `slots()` — a pure function with a unit test at
//! the minimum width — and the painting only consumes them. A layout that cannot
//! be tested without a window will eventually be "fixed" by moving a magic number.
//!
//! ## What this module does not do
//!
//! It never applies window alpha. The label shows the opacity that is actually in
//! effect, which is 100% until `hub-win` grows a compositing path (batch 1c). A
//! strip that says `GHOST` over an opaque window would be worse than one that
//! says `100%`: it would make the feature look implemented in a screenshot.

use egui::{
    Align2, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use crate::theme::Palette;
use hub_core::STRIP_HEIGHT;

/// Width reserved for each side slot, logical pixels.
///
/// Wide enough for the widest left label (`GHOST` at 11 pt is ~34 px) plus the
/// 8 px inset, and for a 24 px button on the right. The middle slot gets what is
/// left, which is what keeps the three from overlapping at 150 px.
pub const SIDE_SLOT_WIDTH: f32 = 44.0;

/// Height of the placeholder panel behind the `•••` button.
pub const TOOLS_PANEL_HEIGHT: f32 = 72.0;

const BUTTON_SIZE: f32 = 24.0;
const GHOST_EPSILON: f32 = 0.005;
const OPACITY_STEP: f32 = 0.2;

/// Horizontal slots, as `[start, end)` ranges in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slots {
    pub left: (f32, f32),
    pub center: (f32, f32),
    pub right: (f32, f32),
}

/// Split `width` into the fixed side slots and the flexible middle.
///
/// Each side is capped at a third of the width, so even a window narrower than
/// the 150 px minimum (a bug elsewhere) cannot make the ranges meet or invert.
pub fn slots(width: f32) -> Slots {
    let width = width.max(0.0);
    let side = SIDE_SLOT_WIDTH.min(width / 3.0);
    Slots {
        left: (0.0, side),
        center: (side, width - side),
        right: (width - side, width),
    }
}

/// Text for the left slot: a percentage, or `GHOST` at zero.
///
/// Rounding is to whole percent, so 0.404 reads `40%` rather than `40.4%`; the
/// shipped toolbar does the same and a longer string would widen the slot.
pub fn opacity_label(opacity: f32) -> String {
    if !opacity.is_finite() {
        return "100%".to_string();
    }
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= GHOST_EPSILON {
        "GHOST".to_string()
    } else {
        format!("{}%", (opacity * 100.0).round() as i32)
    }
}

/// One 20% step up or down, on a fixed grid.
///
/// Quantising before stepping is not cosmetic: `0.2 + 0.2 + 0.2` is 0.6000000000001
/// in binary, and a wheel handler that adds repeatedly drifts until the label
/// shows `59%`. Rounding to the grid makes ten steps land exactly on 1.0.
pub fn step_opacity(current: f32, up: bool) -> f32 {
    let current = if current.is_finite() {
        current.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let steps = (current / OPACITY_STEP).round();
    let next = steps + if up { 1.0 } else { -1.0 };
    (next * OPACITY_STEP).clamp(0.0, 1.0)
}

/// Mutable strip state: what the user has toggled, not what is being displayed.
#[derive(Debug, Clone, Copy)]
pub struct StripState {
    /// Applied window opacity, 0..=1. 1.0 until batch 1c wires compositing.
    pub opacity: f32,
    /// Always-on-top, mirrored from the window so a failed command is visible.
    pub pinned: bool,
    pub tools_open: bool,
    /// Opacity to restore when ghost mode is left (parity Q29).
    pub ghost_restore: f32,
}

impl Default for StripState {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            pinned: true,
            tools_open: false,
            ghost_restore: 0.2,
        }
    }
}

impl StripState {
    /// Ghost mode: 0, then back to the value that was in effect.
    ///
    /// Restoring `max(0.2, last)` rather than a fixed 20% is the shipped
    /// behaviour: leaving ghost mode should not forget that the user had set 40%.
    pub fn toggle_ghost(&mut self) {
        if self.opacity <= GHOST_EPSILON {
            self.opacity = self.ghost_restore.max(OPACITY_STEP);
            self.ghost_restore = self.opacity;
        } else {
            self.ghost_restore = self.opacity;
            self.opacity = 0.0;
        }
    }
}

/// What the user did to the strip this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct StripActions {
    pub toggle_tools: bool,
    pub toggle_pin: bool,
    pub drag: bool,
}

/// Result of drawing the chrome.
pub struct StripOutput {
    /// Where the text island goes, in logical points.
    pub content: Rect,
    pub actions: StripActions,
}

/// Draw the strip (and the tools panel when open); return where the island goes.
pub fn draw(ctx: &egui::Context, state: &mut StripState, palette: Palette) -> StripOutput {
    let full = ctx.screen_rect();
    let mut actions = StripActions::default();
    let mut content = full;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(palette.panel))
        .show(ctx, |ui| {
            ui.set_clip_rect(full);
            let strip = Rect::from_min_size(full.min, Vec2::new(full.width(), STRIP_HEIGHT));

            let painter = ui.painter();
            painter.rect_filled(strip, CornerRadius::ZERO, palette.panel_strong);
            painter.hline(
                strip.x_range(),
                strip.bottom() - 1.0,
                Stroke::new(1.0, palette.border),
            );

            let slots = slots(full.width());
            let label = opacity_label(state.opacity);
            painter.text(
                egui::pos2(strip.left() + 8.0, strip.center().y),
                Align2::LEFT_CENTER,
                &label,
                FontId::proportional(11.0),
                if state.opacity <= GHOST_EPSILON {
                    palette.accent
                } else {
                    palette.ink_soft
                },
            );

            // Buttons first: egui gives the later-registered widget priority for
            // clicks, so pressing `•••` must not also start a drag.
            let tools_rect = button_rect(&slots.center, strip);
            let pin_rect = button_rect(&slots.right, strip);
            let tools = ui.interact(tools_rect, ui.id().with("tools"), Sense::click());
            let pin = ui.interact(pin_rect, ui.id().with("pin"), Sense::click());

            let tools_hot = tools.hovered() || state.tools_open;
            let pin_hot = pin.hovered() || state.pinned;
            let painter = ui.painter();
            paint_button(painter, tools_rect, tools_hot, palette, "•••");
            paint_button(painter, pin_rect, pin_hot, palette, "📌");

            if tools.clicked() {
                actions.toggle_tools = true;
            }
            if pin.clicked() {
                actions.toggle_pin = true;
            }

            let drag_rect = strip;
            let drag = ui.interact(drag_rect, ui.id().with("strip"), Sense::drag());
            let pointer = ctx.input(|i| i.pointer.interact_pos());
            let on_button = pointer
                .map(|p| tools_rect.contains(p) || pin_rect.contains(p))
                .unwrap_or(false);
            if drag.drag_started() && !on_button {
                actions.drag = true;
            }

            let mut top = strip.bottom();
            if state.tools_open {
                let panel = Rect::from_min_size(
                    egui::pos2(full.left(), top),
                    Vec2::new(full.width(), TOOLS_PANEL_HEIGHT),
                );
                let painter = ui.painter();
                painter.rect_filled(panel, CornerRadius::ZERO, palette.panel);
                painter.hline(
                    panel.x_range(),
                    panel.bottom() - 1.0,
                    Stroke::new(1.0, palette.border),
                );
                painter.text(
                    egui::pos2(panel.left() + 8.0, panel.top() + 12.0),
                    Align2::LEFT_TOP,
                    "工具面板",
                    FontId::proportional(11.0),
                    palette.ink_soft,
                );
                painter.text(
                    egui::pos2(panel.left() + 8.0, panel.top() + 32.0),
                    Align2::LEFT_TOP,
                    "内容随 Phase 2 补齐 · 置顶见标题条右槽",
                    FontId::proportional(10.0),
                    palette.ink_subtle,
                );
                top = panel.bottom();
            }

            content = Rect::from_min_max(egui::pos2(full.left(), top), full.max);
        });

    StripOutput { content, actions }
}

fn button_rect(slot: &(f32, f32), strip: Rect) -> Rect {
    let center_x = (slot.0 + slot.1) / 2.0;
    Rect::from_center_size(
        egui::pos2(center_x, strip.center().y),
        Vec2::splat(BUTTON_SIZE),
    )
}

fn paint_button(painter: &egui::Painter, rect: Rect, active: bool, palette: Palette, glyph: &str) {
    if active {
        painter.rect_filled(rect, CornerRadius::same(6), palette.panel);
        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, palette.border),
            StrokeKind::Inside,
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(11.0),
        if active { palette.ink } else { palette.ink_subtle },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_slots_never_overlap_at_the_minimum_width() {
        let slots = slots(hub_core::MIN_NOTE_WIDTH);
        assert_eq!(slots.left, (0.0, SIDE_SLOT_WIDTH));
        assert_eq!(slots.right.1, hub_core::MIN_NOTE_WIDTH);
        assert!(slots.center.0 <= slots.center.1, "center inverted: {slots:?}");
        assert!(slots.center.0 - slots.left.1 <= f32::EPSILON);
        assert!(slots.right.0 - slots.center.1 <= f32::EPSILON);
        // The middle slot must still be wide enough to show a 62 px gap, which is
        // what the shipped toolbar leaves after its fixed sides.
        assert!(slots.center.1 - slots.center.0 >= 60.0);
    }

    #[test]
    fn slots_stay_ordered_even_below_the_minimum() {
        for width in [0.0, 40.0, 90.0, 132.0, 400.0] {
            let slots = slots(width);
            assert!(slots.left.1 <= slots.center.0, "w={width} {slots:?}");
            assert!(slots.center.1 <= slots.right.0, "w={width} {slots:?}");
            assert!(slots.right.1 <= width + f32::EPSILON, "w={width} {slots:?}");
        }
    }

    #[test]
    fn at_the_minimum_width_both_buttons_fit_inside_their_slots() {
        let slots = slots(hub_core::MIN_NOTE_WIDTH);
        let strip = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(150.0, 32.0));
        let tools = button_rect(&slots.center, strip);
        let pin = button_rect(&slots.right, strip);
        assert!(tools.left() >= slots.center.0 && tools.right() <= slots.center.1);
        assert!(pin.left() >= slots.right.0 && pin.right() <= slots.right.1);
        assert!(tools.right() <= pin.left(), "buttons overlap at 150 px");
    }

    #[test]
    fn label_reads_ghost_only_at_zero() {
        assert_eq!(opacity_label(1.0), "100%");
        assert_eq!(opacity_label(0.2), "20%");
        assert_eq!(opacity_label(0.404), "40%");
        assert_eq!(opacity_label(0.0), "GHOST");
        assert_eq!(opacity_label(-1.0), "GHOST");
        assert_eq!(opacity_label(f32::NAN), "100%");
    }

    #[test]
    fn opacity_steps_do_not_drift() {
        let mut opacity = 1.0;
        for _ in 0..5 {
            opacity = step_opacity(opacity, false);
        }
        // Five steps down must land exactly on 0, not 0.0000001 short of it.
        assert_eq!(opacity, 0.0);
        for _ in 0..5 {
            opacity = step_opacity(opacity, true);
        }
        assert_eq!(opacity, 1.0);
        assert_eq!(step_opacity(0.0, false), 0.0);
        assert_eq!(step_opacity(1.0, true), 1.0);
    }

    #[test]
    fn ghost_mode_restores_the_value_it_interrupted() {
        let mut state = StripState::default();
        state.opacity = 0.4;
        state.toggle_ghost();
        assert_eq!(state.opacity, 0.0);
        assert_eq!(opacity_label(state.opacity), "GHOST");
        state.toggle_ghost();
        // Back to 40%, not to 20%: Q29 is specifically about not forgetting.
        assert_eq!(opacity_label(state.opacity), "40%");
    }

    #[test]
    fn ghost_mode_from_zero_returns_to_a_visible_value() {
        let mut state = StripState::default();
        state.opacity = 0.0;
        state.toggle_ghost();
        assert_eq!(opacity_label(state.opacity), "20%");
    }
}
