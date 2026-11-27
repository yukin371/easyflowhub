//! Hover hints: every clickable control says what it does and which key does it.
//!
//! ## Why this is a module and not a call to `on_hover_text`
//!
//! A shortcut that nothing on screen mentions is a shortcut only its author
//! knows. The rule for this shell is therefore: **a control that has a key bound
//! to it shows the key on hover**, and the hint is built from the same table the
//! keyboard handler reads (`core`'s shortcut table), never from a string typed
//! next to the button. A hint that drifts from the binding is worse than no hint,
//! because it teaches the wrong key.
//!
//! ## Shape
//!
//! One line: the action's name, then the chord in a monospace chip. The chip is
//! drawn rather than letter-spaced text so that `Ctrl+Shift+Z` stays one glance
//! wide instead of becoming part of the sentence, and so that a future "press a
//! new key" affordance can reuse the same rectangle.

use egui::{CornerRadius, FontId, RichText, Sense, Vec2};

use crate::theme::Theme;

/// Height of the chip's padding, in points.
const CHIP_PADDING: Vec2 = Vec2::new(10.0, 3.0);

/// Attach a name-plus-chord hint to an interactive response.
///
/// Returns the response unchanged so it can be written inline:
/// `let pin = hint(pin, palette, "置顶窗口", "Ctrl+T");`
///
/// `chord` is what the settings table produced, so an empty string means "this
/// control has no key" and the hint shows the name alone -- a chip with nothing
/// in it would read as a key we forgot to name.
pub fn hint(
    response: egui::Response,
    palette: Theme,
    label: &str,
    chord: &str,
) -> egui::Response {
    if chord.is_empty() {
        return response.on_hover_ui(|ui| {
            ui.label(RichText::new(label).size(11.5).color(palette.ink_soft));
        });
    }
    response.on_hover_ui(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(11.5).color(palette.ink_soft));
            chip(ui, palette, chord);
        });
    })
}

/// The key itself, in a box: `Ctrl+K`.
pub fn chip(ui: &mut egui::Ui, palette: Theme, chord: &str) {
    let galley = ui.painter().layout_no_wrap(
        chord.to_string(),
        FontId::monospace(10.5),
        palette.ink,
    );
    let (rect, _) = ui.allocate_exact_size(galley.size() + CHIP_PADDING, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(3), palette.surface);
    ui.painter().galley(
        rect.min + Vec2::new(CHIP_PADDING.x / 2.0, CHIP_PADDING.y / 2.0),
        galley,
        palette.ink,
    );
}

/// A hint that includes a gesture rather than a key, e.g. `Ctrl+滚轮`.
///
/// Kept separate so the distinction survives in the source: one of these is
/// rebindable and read from the table, the other is a mouse gesture the window
/// owns and cannot remap. Rendering them identically is fine; storing them
/// identically would invite a settings screen to offer to rebind a wheel.
pub fn gesture_hint(
    response: egui::Response,
    palette: Theme,
    label: &str,
    gesture: &str,
) -> egui::Response {
    hint(response, palette, label, gesture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chip_is_sized_for_its_text() {
        // A chip that ignores its text would clip `Ctrl+Shift+Z`, which is one of
        // the defaults -- so the sizing is a property, not a preference.
        //
        // Measured inside a real frame: the text layout needs the font atlas, and
        // asking for it before the first pass panics rather than returning zero.
        let ctx = egui::Context::default();
        let palette = Theme::quick();
        let mut measure = |chord: &str| -> f32 {
            let mut width = 0.0;
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let galley = ui.painter().layout_no_wrap(
                        chord.to_string(),
                        FontId::monospace(10.5),
                        palette.ink,
                    );
                    width = galley.size().x + CHIP_PADDING.x;
                });
            });
            width
        };
        let short = measure("Ctrl+K");
        let long = measure("Ctrl+Shift+Z");
        assert!(long > short, "short={short} long={long}");
        assert!(short > 0.0);
    }
}
