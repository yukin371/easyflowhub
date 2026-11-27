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

use egui::{Align2, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

use crate::hint;
use crate::theme::Theme;
use hub_core::{Command, STRIP_HEIGHT, Table};

/// Width reserved for each side slot, logical pixels.
///
/// The shipped toolbar declares `minWidth: 46` on the left slot, but that is a
/// *minimum*: `GHOST` at 10 px with `letterSpacing: 0.22em` is ~44 px of text,
/// so the slot actually grows to 46+16 or more. Its counterpart here is
/// `LABEL_INSET + 44`, which is the same thing without a flex pass.
///
/// It is not a hard wall. Below `3 × 60` the ranges are capped at a third of the
/// width so they cannot invert, and at the 150 px minimum the label does overrun
/// its slot; the `the_ghost_label_clears_the_dots` test pins the remaining
/// clearance to the dots rather than pretending it fits.
pub const SIDE_SLOT_WIDTH: f32 = 60.0;

/// Height of the placeholder panel behind the `•••` button.
pub const TOOLS_PANEL_HEIGHT: f32 = 72.0;

/// Inset of the opacity label from the window's left edge.
///
/// 16 is not a round number picked by eye: the shipped strip has
/// `paddingInline: 8` and its left slot has another `paddingInline: 8`, so the
/// text starts 16 px in. The right side is *not* the same number — see
/// [`BUTTON_INSET`] — because a 16 px glyph sits inside a 28 px button.
pub const LABEL_INSET: f32 = 16.0;

/// Inset of the rightmost button's *box* from the window's right edge.
///
/// The shipped strip ends with `paddingInline: 8`, and the pin is the last child,
/// so its 28 px box ends 8 px in. The glyph inside it is centred with 6 px of
/// slack, which puts its visible edge at 14 px — 2 px tighter than the label.
/// That asymmetry is in the shipped toolbar and reads as correct; making both
/// sides 12 px (the first attempt) put both off by 2 px in opposite directions.
pub const BUTTON_INSET: f32 = 8.0;

/// Where the pin's drawn edge lands: `BUTTON_INSET` plus the button's own slack.
pub const ICON_INSET: f32 = BUTTON_INSET + (BUTTON_SIZE - ICON_SIZE) / 2.0;

/// Inset used inside the tools panel, which has no button box to align with.
pub const EDGE_PADDING: f32 = 12.0;

const BUTTON_SIZE: f32 = 28.0;
/// Icon box inside a button, matching the shipped 16×16 SVGs.
const ICON_SIZE: f32 = 16.0;
/// The unit box the shipped icons are authored in (`viewBox="0 0 24 24"`).
const ICON_VIEWBOX: f32 = 24.0;
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
///
/// `table` is the shortcut table, not a `&str` per button: the hint under the
/// pointer comes from the same object the keyboard handler reads, so a rebound
/// key changes the tooltip and the behaviour in one edit. Passing pre-formatted
/// strings would have been one more place to forget.
pub fn draw(
    ctx: &egui::Context,
    state: &mut StripState,
    palette: Theme,
    table: &Table,
) -> StripOutput {
    let full = ctx.screen_rect();
    let mut actions = StripActions::default();
    let mut content = full;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(palette.canvas))
        .show(ctx, |ui| {
            ui.set_clip_rect(full);
            let strip = Rect::from_min_size(full.min, Vec2::new(full.width(), STRIP_HEIGHT));

            let painter = ui.painter();
            painter.rect_filled(strip, CornerRadius::ZERO, palette.surface);
            painter.hline(
                strip.x_range(),
                strip.bottom() - 1.0,
                Stroke::new(1.0, palette.border),
            );

            let slots = slots(full.width());
            let label = opacity_label(state.opacity);
            painter.text(
                egui::pos2(strip.left() + LABEL_INSET, strip.center().y),
                Align2::LEFT_CENTER,
                &label,
                FontId::proportional(11.0),
                if state.opacity <= GHOST_EPSILON {
                    palette.accent
                } else {
                    palette.ink_soft
                },
            );

            // Geometry first: the drag surface needs to know where the buttons are
            // before it decides whether a press belongs to it.
            let tools_rect = center_button_rect(&slots.center, strip);
            let pin_rect = right_button_rect(strip);

            // The drag surface is registered *before* the buttons on purpose: egui
            // gives a click to the last widget under the pointer, so registering it
            // after them left both buttons dead to the mouse while the keyboard
            // still worked (found by `scripts/check-native-tools.py`, 2026-11-26).
            let drag = ui.interact(strip, ui.id().with("strip"), Sense::drag());
            // ...but ordering alone is not enough: `Sense::drag()` reports
            // `drag_started` on the same press that the button above it claims, so
            // pressing `•••` would also start a window drag. Measured with the OS
            // move loop: the window moved a few pixels and the *next* click landed
            // on nothing. `actions.drag` is now a request the shell honours by
            // moving the window itself (see `app/src/note/drag.rs`).
            let pointer = ctx.input(|i| i.pointer.interact_pos());
            let on_button = pointer
                .map(|p| tools_rect.contains(p) || pin_rect.contains(p))
                .unwrap_or(false);
            if drag.drag_started() && !on_button {
                actions.drag = true;
            }

            // Buttons on top of it, so a press on one is a click and not a drag.
            let tools = hint::hint(
                ui.interact(tools_rect, ui.id().with("tools"), Sense::click()),
                palette,
                Command::ToggleTools.label(),
                &table.hint(Command::ToggleTools),
            );
            let pin = hint::hint(
                ui.interact(pin_rect, ui.id().with("pin"), Sense::click()),
                palette,
                Command::TogglePin.label(),
                &table.hint(Command::TogglePin),
            );

            let painter = ui.painter();
            // Neither button paints a background, in any state: the shipped
            // toolbar gives these a `background: none` and no border, and a
            // rounded plate behind a 16 px glyph reads as a different control.
            paint_tools_glyph(painter, tools_rect, glyph_color(palette, state.opacity));
            paint_bookmark(
                painter,
                pin_rect,
                if state.pinned {
                    palette.accent
                } else {
                    palette.ink_subtle
                },
                state.pinned,
            );

            if tools.hovered() || pin.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            if tools.clicked() {
                actions.toggle_tools = true;
            }
            if pin.clicked() {
                actions.toggle_pin = true;
            }

            let mut top = strip.bottom();
            if state.tools_open {
                let panel = Rect::from_min_size(
                    egui::pos2(full.left(), top),
                    Vec2::new(full.width(), TOOLS_PANEL_HEIGHT),
                );
                let painter = ui.painter();
                painter.rect_filled(panel, CornerRadius::ZERO, palette.canvas);
                painter.hline(
                    panel.x_range(),
                    panel.bottom() - 1.0,
                    Stroke::new(1.0, palette.border),
                );
                painter.text(
                    egui::pos2(panel.left() + EDGE_PADDING, panel.top() + 12.0),
                    Align2::LEFT_TOP,
                    "工具面板",
                    FontId::proportional(11.0),
                    palette.ink_soft,
                );
                painter.text(
                    egui::pos2(panel.left() + EDGE_PADDING, panel.top() + 32.0),
                    Align2::LEFT_TOP,
                    "Ctrl+滚轮 调整不透明度 · 20% 一档",
                    FontId::proportional(10.0),
                    palette.ink_subtle,
                );
                painter.text(
                    egui::pos2(panel.left() + EDGE_PADDING, panel.top() + 48.0),
                    Align2::LEFT_TOP,
                    &format!("{} · 幽灵模式（再按恢复原值）", table.hint(Command::ToggleGhost)),
                    FontId::proportional(10.0),
                    palette.ink_subtle,
                );
                top = panel.bottom();
            }

            content = Rect::from_min_max(egui::pos2(full.left(), top), full.max);
        });

    StripOutput { content, actions }
}

fn center_button_rect(slot: &(f32, f32), strip: Rect) -> Rect {
    let center_x = (slot.0 + slot.1) / 2.0;
    Rect::from_center_size(
        egui::pos2(center_x, strip.center().y),
        Vec2::splat(BUTTON_SIZE),
    )
}

/// The right button is pinned to `BUTTON_INSET`, not centred in its slot.
///
/// The slot is wide enough to hold a 28 px button plus the shipped inset;
/// centring the button in it instead would leave 16 px on the right and the
/// glyph would sit 6 px further in than the shipped toolbar puts it.
fn right_button_rect(strip: Rect) -> Rect {
    let right = strip.right() - BUTTON_INSET;
    Rect::from_min_max(
        egui::pos2(right - BUTTON_SIZE, strip.center().y - BUTTON_SIZE / 2.0),
        egui::pos2(right, strip.center().y + BUTTON_SIZE / 2.0),
    )
}

/// The `color-mix(in srgb, … 72%, transparent)` the shipped toolbar paints its
/// icons with, including its `0.32` floor.
///
/// The floor is not arbitrary: at ghost opacity the icons must stay visible
/// enough to be clicked, which is why the shipped toolbar never lets them fall
/// below roughly a third no matter how transparent the window gets.
fn glyph_color(palette: Theme, opacity: f32) -> egui::Color32 {
    let alpha = (opacity.clamp(0.0, 1.0) * 0.72).max(0.32);
    let ink = palette.ink;
    // Alpha, not `gamma_multiply`: that one scales RGB as well, which darkens
    // the glyph instead of letting the strip show through — on this near-black
    // panel the difference is a visibly muddier icon.
    egui::Color32::from_rgba_unmultiplied(
        ink.r(),
        ink.g(),
        ink.b(),
        (alpha * 255.0).round() as u8,
    )
}

/// Three dots, drawn instead of typed.
///
/// The glyph `•••` was the obvious choice and the wrong one: the font stack falls
/// back per-character, so the bullets picked up the CJK face and rendered at a
/// different optical size than the label next to them. Circles are exact.
fn paint_tools_glyph(painter: &egui::Painter, rect: Rect, color: egui::Color32) {
    for offset in [-5.5_f32, 0.0, 5.5] {
        painter.circle_filled(rect.center() + Vec2::new(offset, 0.0), 1.6, color);
    }
}

/// The always-on-top icon, traced from the shipped toolbar rather than redrawn.
///
/// `QuickNoteToolbar.tsx` renders `M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z`
/// at 16×16 with a 2-unit stroke, `fill:none` while floating and
/// `fill:currentColor` while pinned. It is a **bookmark, not a thumbtack** — the
/// earlier `📌` was a different metaphor that egui's font stack could not even
/// draw (it came out as a white box). The 2-unit corner radius and the 3.5-unit
/// notch are what keep this legible at 16 px, so they are copied as numbers
/// instead of being re-derived.
fn paint_bookmark(painter: &egui::Painter, rect: Rect, color: egui::Color32, filled: bool) {
    let box_rect = Rect::from_center_size(rect.center(), Vec2::splat(ICON_SIZE));
    let outline = bookmark_outline(box_rect);
    let stroke = Stroke::new(2.0 * ICON_SIZE / ICON_VIEWBOX, color);

    if filled {
        // egui's fill tessellator is only defined for convex paths, and the notch
        // at (12, 17.5) is a reflex vertex. Three convex pieces sharing the two
        // edges that meet there cover exactly the same area.
        let mut body = outline[..9].to_vec();
        body.push(bookmark_point(box_rect, 19.0, 17.5));
        body.push(bookmark_point(box_rect, 5.0, 17.5));
        painter.add(Shape::convex_polygon(body, color, Stroke::NONE));
        painter.add(Shape::convex_polygon(
            vec![
                bookmark_point(box_rect, 5.0, 17.5),
                bookmark_point(box_rect, 12.0, 17.5),
                bookmark_point(box_rect, 5.0, 21.0),
            ],
            color,
            Stroke::NONE,
        ));
        painter.add(Shape::convex_polygon(
            vec![
                bookmark_point(box_rect, 19.0, 17.5),
                bookmark_point(box_rect, 19.0, 21.0),
                bookmark_point(box_rect, 12.0, 17.5),
            ],
            color,
            Stroke::NONE,
        ));
    }

    // `strokeLinejoin="round"`: egui butts its segments together, which would
    // leave a notch at the two sharp corners at this stroke width. A dot per
    // vertex is that join, and it costs one triangle each.
    for point in &outline {
        painter.circle_filled(*point, stroke.width / 2.0, color);
    }
    painter.add(Shape::closed_line(outline, stroke));
}

/// The bookmark's vertices, in `ICON_VIEWBOX` units, mapped into `rect`.
///
/// The two top corners are quarter arcs of radius 2 centred at (7, 5) and
/// (17, 5); four segments each is below one device pixel of error at 16 px.
fn bookmark_outline(rect: Rect) -> Vec<Pos2> {
    const RADIUS: f32 = 2.0;
    let mut points = Vec::with_capacity(12);
    points.push(bookmark_point(rect, 5.0, 5.0));
    for (centre_x, start_degrees) in [(7.0_f32, 180.0_f32), (17.0, 270.0)] {
        for step in 1..=4 {
            let degrees = start_degrees + 90.0 * step as f32 / 4.0;
            let radians = degrees.to_radians();
            points.push(bookmark_point(
                rect,
                centre_x + RADIUS * radians.cos(),
                5.0 + RADIUS * radians.sin(),
            ));
        }
    }
    points.push(bookmark_point(rect, 19.0, 21.0));
    points.push(bookmark_point(rect, 12.0, 17.5));
    points.push(bookmark_point(rect, 5.0, 21.0));
    points
}

fn bookmark_point(rect: Rect, x: f32, y: f32) -> Pos2 {
    let unit = rect.width() / ICON_VIEWBOX;
    rect.min + Vec2::new(x, y) * unit
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A click on a title-bar button, with the drag surface registered underneath.
    ///
    /// This test exists because the bug it pins was invisible to every other kind of
    /// check: the buttons were added *before* the strip's `Sense::drag()`, and egui
    /// gives a press to the last widget under the pointer, so both buttons were dead
    /// on a real mouse while `Ctrl+.` and `Ctrl+T` worked. It was found by
    /// `scripts/check-native-tools.py`, not by reading the code.
    #[test]
    fn a_click_on_a_button_is_not_swallowed_by_the_drag_surface() {
        let at = |point: Pos2| {
            let ctx = egui::Context::default();
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
            let palette = Theme::quick();
            let table = Table::defaults();
            let mut state = StripState::default();
            let mut frame = |events: Vec<egui::Event>| -> StripActions {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                };
                let mut actions = StripActions::default();
                ctx.run(input, |ctx| {
                    actions = draw(ctx, &mut state, palette, &table).actions;
                });
                actions
            };
            let button = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            };
            frame(vec![egui::Event::PointerMoved(point)]);
            frame(vec![button(point, true)]);
            frame(vec![button(point, false)])
        };

        let strip = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, STRIP_HEIGHT));
        let slots = slots(400.0);

        let pin = at(right_button_rect(strip).center());
        assert!(pin.toggle_pin, "a click on the pin never reached it");
        assert!(!pin.drag, "clicking the pin also started a window drag");

        let tools = at(center_button_rect(&slots.center, strip).center());
        assert!(tools.toggle_tools, "a click on the dots never reached them");
        assert!(!tools.drag, "clicking the dots also started a window drag");
    }

    #[test]
    fn the_pin_is_the_shipped_bookmark_not_a_thumbtack() {
        let rect = Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(ICON_SIZE));
        let outline = bookmark_outline(rect);
        // `M5 5` + four samples per arc + `L19 21`, `L12 17.5`, `L5 21`.
        assert_eq!(outline.len(), 12);
        let unit = ICON_SIZE / ICON_VIEWBOX;
        assert!(
            (outline[0] - egui::pos2(5.0 * unit, 5.0 * unit)).length() < 1e-4,
            "first vertex must be the path's M5 5, got {:?}",
            outline[0]
        );
        // The notch is what makes it a bookmark. Dropping it would leave a
        // plain rectangle that still passes a "an icon was drawn" check.
        let notch = bookmark_point(rect, 12.0, 17.5);
        assert!(outline.iter().any(|point| (*point - notch).length() < 1e-4));
    }

    #[test]
    fn the_bookmark_fits_inside_its_icon_box() {
        let rect = Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(ICON_SIZE));
        let half_stroke = ICON_SIZE / ICON_VIEWBOX;
        for point in bookmark_outline(rect) {
            assert!(
                rect.shrink(half_stroke).contains(point),
                "{point:?} leaves the 16 px box {rect:?}"
            );
        }
    }

    #[test]
    fn strip_icons_are_translucent_but_never_fade_out() {
        let palette = Theme::quick();
        // `color-mix(in srgb, --quick-text 72%, transparent)` at full opacity.
        assert_eq!(glyph_color(palette, 1.0).a(), 184);
        // The shipped `alphaAtLeast(…, 0.32, …)`: a ghost note keeps clickable
        // icons even at 0%.
        assert_eq!(glyph_color(palette, 0.0).a(), 82);
        // The hue must survive: `Color32` stores premultiplied, so the check is
        // on the unmultiplied value, with one step of slack for the round trip.
        // Using `gamma_multiply` here instead would have scaled the RGB and made
        // the icon darker than the ink it is supposed to be.
        let [r, g, b, _] = glyph_color(palette, 1.0).to_srgba_unmultiplied();
        let expected = [palette.ink.r(), palette.ink.g(), palette.ink.b()];
        for (got, want) in [r, g, b].into_iter().zip(expected) {
            assert!(got.abs_diff(want) <= 1, "hue shifted: {got} vs {want}");
        }
    }

    #[test]
    fn three_slots_never_overlap_at_the_minimum_width() {
        let width = hub_core::MIN_NOTE_WIDTH;
        let slots = slots(width);
        let side = SIDE_SLOT_WIDTH.min(width / 3.0);
        assert_eq!(slots.left, (0.0, side));
        assert_eq!(slots.right.1, width);
        assert!(slots.center.0 <= slots.center.1, "center inverted: {slots:?}");
        assert!(slots.center.0 - slots.left.1 <= f32::EPSILON);
        assert!(slots.right.0 - slots.center.1 <= f32::EPSILON);
        // The middle slot must still hold the 28 px dots button whole.
        assert!(slots.center.1 - slots.center.0 >= BUTTON_SIZE);
    }

    #[test]
    fn the_ghost_label_clears_the_dots() {
        // Measured, not estimated: `GHOST` at 11 pt in the installed CJK face is
        // 59 device px at 150%, i.e. 39 logical px (columns 24..82 of a 600 px
        // shot, starting at the 16 px inset).
        const GHOST_WIDTH: f32 = 39.0;

        // At the default 400 px window the label fits inside its own slot.
        let slots = slots(400.0);
        assert!(
            LABEL_INSET + GHOST_WIDTH <= slots.left.1,
            "label overruns its slot at 400 px: {} > {}",
            LABEL_INSET + GHOST_WIDTH,
            slots.left.1
        );

        // At the 150 px minimum it does not fit — 16 + 39 = 55 against a 50 px
        // cap — but it still stops 6 px short of the dots, which is what Q11
        // actually asks for. Pinning this keeps a future font or inset change
        // from turning a cosmetic overrun into an overlap.
        let dots_left = (slots.center.0 + slots.center.1) / 2.0 - BUTTON_SIZE / 2.0;
        assert!(LABEL_INSET + GHOST_WIDTH < dots_left);
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
        let tools = center_button_rect(&slots.center, strip);
        let pin = right_button_rect(strip);
        assert!(tools.left() >= slots.center.0 && tools.right() <= slots.center.1);
        assert!(pin.left() >= slots.right.0 && pin.right() <= slots.right.1);
        assert!(tools.right() <= pin.left(), "buttons overlap at 150 px");
    }

    #[test]
    fn the_two_edges_match_the_shipped_asymmetry() {
        let strip = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(150.0, 32.0));
        let pin = right_button_rect(strip);

        // Shipped: strip `paddingInline: 8` + left slot `paddingInline: 8`.
        assert_eq!(LABEL_INSET, 16.0);
        // Shipped: strip `paddingInline: 8`; the glyph inside the 28 px button
        // has 6 px of slack, so its visible edge is at 14.
        assert_eq!(pin.right(), 150.0 - 8.0);
        assert_eq!(ICON_INSET, 14.0);
        // The 2 px difference is deliberate. A "tidy" 12/12 would be off by 4 px
        // on the label and 2 px on the icon, and neither would line up with the
        // text below it (18 px in the shipped body).
        assert_ne!(LABEL_INSET, ICON_INSET);
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
