//! Theme tokens: the one place a colour or a radius is written down.
//!
//! ## Why this is not just a struct with seven colours
//!
//! The previous version had `panel`, `panel_strong`, `border`, three `ink`s and an
//! `accent`, and every call site that needed a fifth thing did the arithmetic
//! itself: `panel_strong.gamma_multiply(0.72)` for an unselected card, `0.55` for
//! a list row, `0.6` for a switcher row, `accent.gamma_multiply(0.28)` for a
//! filled button. Four files, four coefficients, one meaning. That is a palette,
//! not a theme, and it fails in a specific way:
//!
//! **A multiplier has no direction.** `gamma_multiply(0.72)` darkens. On this dark
//! panel a card 28% darker than the input box happens to look like "one step
//! below", so the numbers passed review. On a light theme the same line paints a
//! card *darker than the page under it* -- a hole, not a card. Every derived
//! colour here is therefore produced by `lift(base, ink, t)`, which moves a colour
//! *towards the text colour*. Dark theme: text is light, so lifted surfaces are
//! lighter. Light theme: text is dark, so lifted surfaces are darker. One rule,
//! correct in both directions, and no call site gets to pick a coefficient.
//!
//! **Seven slots cannot name nine roles.** `panel_strong` was simultaneously the
//! search field, a card, a selected tab and a key chip. Wanting the search field
//! to look different from a card meant editing two unrelated screens. The roles
//! are now separate fields, so "make hover quieter" is a one-line change in
//! [`Theme::build`] instead of a hunt.
//!
//! ## The constraint that shapes all of this
//!
//! The quick note lets the user supply their own colours (`NoteColors`, seven
//! values, shipped defaults in `hub-core`). So every derived token has to be
//! computable *from those seven* -- a theme file cannot ask the user for fifteen
//! values, and a derived token that only exists for the built-in palettes would
//! disappear the moment someone typed a hex code. That is why `from_colors` and
//! `standard` both funnel through one `build`: there is exactly one derivation
//! rule and no way to construct a theme without it.

use egui::{Color32, FontData, FontDefinitions, FontFamily};
use hub_core::{NoteColors, Rgb};
use std::sync::Arc;

/// How far a hovered surface moves towards the text colour. Small on purpose:
/// hover has to be noticeable in peripheral vision without pulling the eye off
/// the query box, which is where the user is actually looking.
const HOVER_LIFT: f32 = 0.06;

/// Pressed. Twice the hover lift, because a press that looks like a hover reads
/// as "the click did not register".
const ACTIVE_LIFT: f32 = 0.12;

/// Where `surface_soft` sits between `canvas` and `surface`.
///
/// `0.55` is measured against the shipped pair (#181B21 / #212530): the card lands
/// on #1D2028, one visible step above the page and one below the search field.
/// Closer to 0 and the cards dissolve into the page; past 0.8 they compete with
/// the search field for attention.
const SOFT_BLEND: f32 = 0.55;

/// Alpha of an accent-tinted fill (selected card, filled button).
const ACCENT_WASH: f32 = 0.16;

/// Danger red. Fixed rather than derived: the quick note has no error state, so
/// there is no user colour to derive it from, and a user-chosen accent must not
/// silently turn the delete button green.
const DANGER: Color32 = Color32::from_rgb(214, 106, 116);

/// Corner radii, in points. Nine distinct values were in use (3, 4, 7, 8, 10, 11);
/// four are enough to say "chip, row, card, panel" and the difference between 10
/// and 11 is not a decision anyone can defend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Radii {
    /// Key chips and other inline furniture.
    pub chip: u8,
    /// List rows, hint bubbles.
    pub row: u8,
    /// Cards and the search field.
    pub card: u8,
    /// The panel itself and modal cards.
    pub panel: u8,
}

impl Default for Radii {
    fn default() -> Self {
        Self {
            chip: 4,
            row: 8,
            card: 12,
            panel: 14,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    // ── Background, back to front ────────────────────────────────────────────
    /// The page under everything: window body, panel body.
    pub canvas: Color32,
    /// A card at rest. Between `canvas` and `surface` so that a grid of cards
    /// reads as objects lying on the page rather than as outlined holes in it.
    pub surface_soft: Color32,
    /// The first raised layer: search field, selected tab, key chips, hovered row.
    pub surface: Color32,
    /// `surface` with the pointer over it.
    pub surface_hover: Color32,
    /// `surface` while held down.
    pub surface_active: Color32,
    /// What a modal draws over the page behind it.
    pub scrim: Color32,

    // ── Text ────────────────────────────────────────────────────────────────
    pub ink: Color32,
    pub ink_soft: Color32,
    pub ink_subtle: Color32,
    /// Present but not actionable -- an empty slot, a disabled menu entry.
    pub ink_disabled: Color32,

    // ── Lines ───────────────────────────────────────────────────────────────
    /// Hairline between regions.
    pub border: Color32,
    /// A line that has to survive being one pixel on a high-DPI screen.
    pub border_strong: Color32,
    /// What a focused control is outlined with.
    pub focus_ring: Color32,

    // ── Accent ──────────────────────────────────────────────────────────────
    pub accent: Color32,
    /// A wash of accent over `canvas`: the fill of a selected card or a primary
    /// button, dark enough that `ink` still reads on top of it.
    pub accent_soft: Color32,
    /// Text drawn *on* a solid accent fill. Chosen by contrast, not by taste.
    pub on_accent: Color32,

    // ── Status ──────────────────────────────────────────────────────────────
    pub danger: Color32,
    /// Danger as a wash, for a destructive button's background.
    pub danger_soft: Color32,

    // ── Shape ───────────────────────────────────────────────────────────────
    pub radius: Radii,
}

impl Theme {
    /// The launcher and workbench palette.
    pub fn standard() -> Self {
        Self::build(
            Color32::from_rgb(24, 27, 33),
            Color32::from_rgb(33, 37, 45),
            Color32::from_rgb(51, 57, 68),
            Color32::from_rgb(232, 238, 247),
            Color32::from_rgb(168, 178, 194),
            Color32::from_rgb(124, 134, 150),
            Color32::from_rgb(126, 165, 214),
        )
    }

    /// `lite` drops the extra composited layers.
    ///
    /// It used to make `surface` identical to `canvas`, which was fine when cards
    /// were outlines and wrong the moment they became fills: every card would have
    /// vanished. The last two numbers keep one step of separation and drop the
    /// rest, which is the actual cost saving -- one flat fill instead of two
    /// stacked ones, not "no contrast at all".
    pub fn lite() -> Self {
        Self::build(
            Color32::from_rgb(20, 22, 27),
            Color32::from_rgb(28, 31, 38),
            Color32::from_rgb(46, 51, 60),
            Color32::from_rgb(226, 232, 240),
            Color32::from_rgb(158, 167, 181),
            Color32::from_rgb(120, 130, 145),
            Color32::from_rgb(126, 165, 214),
        )
    }

    /// The quick-note window, from the user's `appearance.colors`.
    ///
    /// `styles.css` defines the shipped tokens as `rgba(...)` over `--manager-bg`;
    /// the defaults in `NoteColors` already carry the composited value, so no
    /// blending happens here. Doing the maths once, in `hub-core`, keeps "the
    /// native note is slightly bluer than the Tauri one" from becoming an
    /// unexplained visual difference.
    pub fn from_colors(colors: &NoteColors) -> Self {
        Self::build(
            rgb(colors.panel),
            rgb(colors.strip),
            rgb(colors.border),
            rgb(colors.ink),
            rgb(colors.ink_soft),
            rgb(colors.ink_subtle),
            rgb(colors.accent),
        )
    }

    /// The shipped quick-note palette, i.e. the default `NoteColors`.
    pub fn quick() -> Self {
        Self::from_colors(&NoteColors::default())
    }

    /// The only constructor. Every derived token is computed here, once, so no
    /// caller can be holding a theme whose `surface_hover` was not derived from
    /// its own `surface`.
    fn build(
        canvas: Color32,
        surface: Color32,
        border: Color32,
        ink: Color32,
        ink_soft: Color32,
        ink_subtle: Color32,
        accent: Color32,
    ) -> Self {
        Self {
            canvas,
            surface_soft: mix(canvas, surface, SOFT_BLEND),
            surface,
            surface_hover: lift(surface, ink, HOVER_LIFT),
            surface_active: lift(surface, ink, ACTIVE_LIFT),
            scrim: Color32::from_black_alpha(96),

            ink,
            ink_soft,
            ink_subtle,
            // Towards the page, not away from it: disabled text should recede into
            // the background it is disabled on, and "recede" is a different
            // direction on each theme.
            ink_disabled: mix(canvas, ink_subtle, 0.55),

            border,
            border_strong: lift(border, ink, 0.18),
            focus_ring: accent,

            accent,
            accent_soft: mix(canvas, accent, ACCENT_WASH),
            on_accent: pick_on(accent, canvas, ink),

            danger: DANGER,
            danger_soft: mix(canvas, DANGER, ACCENT_WASH),

            radius: Radii::default(),
        }
    }
}

fn rgb(color: Rgb) -> Color32 {
    Color32::from_rgb(color[0], color[1], color[2])
}

/// `t = 0` gives `a`, `t = 1` gives `b`.
///
/// Straight sRGB interpolation. Not "correct" in a colorimetric sense, but this is
/// one theme deriving nine surfaces from seven values, not a colour-managed
/// pipeline: what matters is that the steps are monotonic and predictable, and
/// that no step can leave the sRGB cube.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let at = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(at(a.r(), b.r()), at(a.g(), b.g()), at(a.b(), b.b()))
}

/// Move `base` towards the text colour by `t`.
///
/// This is the whole theme system in one function: the sign of the change comes
/// from `ink`, so a light theme needs no second rule. See the module comment for
/// what `gamma_multiply` got wrong.
fn lift(base: Color32, ink: Color32, t: f32) -> Color32 {
    mix(base, ink, t)
}

/// WCAG relative luminance.
fn luminance(c: Color32) -> f32 {
    let channel = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// WCAG contrast ratio, 1.0 … 21.0.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Text colour for a filled `background`: whichever of the two candidates reads
/// better on it.
///
/// Contrast, not a brightness threshold. The shipped accent (#7EA5D6) is bright
/// enough that a "if it is dark, use white text" rule picks white and lands on
/// 2.2:1; the panel colour reaches 7.1:1 on the same fill. The threshold version
/// looked reasonable and produced unreadable buttons.
fn pick_on(background: Color32, dark: Color32, light: Color32) -> Color32 {
    if contrast(background, dark) >= contrast(background, light) {
        dark
    } else {
        light
    }
}

/// Windows ships Microsoft YaHei. Which file to pick is a policy question, and
/// the *loading* of it is a platform question -- so the candidates live here (the
/// UI decides what it wants to look like) while `hub-win::font` does the mapping.
pub const CJK_CANDIDATES: [&str; 3] = [
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyhl.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
];

/// Install a CJK font that is already mapped into the address space.
///
/// This is the path that matters: epaint keeps `FontData` in a `Cow`, and a
/// `Cow::Borrowed` buffer becomes an `ab_glyph::FontRef` -- zero copies, glyphs
/// read straight from the file pages. See `hub_win::font` for why that is worth a
/// win32 call.
///
/// Chinese text with the default egui font stack renders as tofu, which would
/// make the timing meaningless in a different way: the atlas would have nothing
/// to rasterise. A missing font is logged by the caller and the panel still runs.
pub fn install_mapped(ctx: &egui::Context, bytes: &'static [u8], index: u32) {
    let mut data = FontData::from_static(bytes);
    data.index = index;
    install(ctx, data);
}

/// Install a CJK font from bytes this process owns.
///
/// Kept as the fallback for a mapping that failed, and as the `--font-heap`
/// baseline the C1 gate compares against: same face, same glyphs, one full copy
/// in the heap (plus the clone epaint makes).
pub fn install_owned(ctx: &egui::Context, bytes: Vec<u8>, index: u32) {
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    install(ctx, data);
}

fn install(ctx: &egui::Context, data: FontData) {
    let mut definitions = FontDefinitions::default();
    definitions
        .font_data
        .insert("cjk".to_string(), Arc::new(data));
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        definitions
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk".to_string());
    }
    ctx.set_fonts(definitions);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::standard()
    }

    #[test]
    fn a_lifted_surface_moves_towards_the_text_colour() {
        // The property the whole design rests on: on a dark theme a lifted surface
        // is lighter, and the same function on a light theme would make it darker,
        // because the direction comes from `ink` and not from a multiplier.
        let dark = theme();
        assert!(
            luminance(dark.surface_hover) > luminance(dark.surface),
            "hover must be a visible step above the resting surface"
        );
        assert!(
            luminance(dark.surface_active) > luminance(dark.surface_hover),
            "press must be a step past hover, or a click reads as ignored"
        );

        // A hand-built light theme: canvas near-white, ink near-black, nothing else
        // changes. If `lift` had a hard-coded direction this assertion would fail.
        let light = Theme::build(
            Color32::from_rgb(248, 249, 251),
            Color32::from_rgb(238, 240, 244),
            Color32::from_rgb(214, 218, 226),
            Color32::from_rgb(26, 29, 35),
            Color32::from_rgb(78, 84, 94),
            Color32::from_rgb(120, 126, 136),
            Color32::from_rgb(52, 112, 188),
        );
        assert!(
            luminance(light.surface_hover) < luminance(light.surface),
            "on a light theme the same lift must darken"
        );
        assert!(
            luminance(light.surface_soft) < luminance(light.canvas),
            "and a card sitting between canvas and surface follows the same rule"
        );
    }

    #[test]
    fn a_card_sits_between_the_page_and_the_search_field() {
        let t = theme();
        let (page, card, field) = (
            luminance(t.canvas),
            luminance(t.surface_soft),
            luminance(t.surface),
        );
        assert!(
            page < card && card < field,
            "card {card:.4} must be a step between page {page:.4} and field {field:.4}"
        );
    }

    #[test]
    fn text_on_the_accent_fill_is_chosen_by_contrast() {
        let t = theme();
        assert_eq!(t.on_accent, t.canvas, "the shipped accent wants dark text");
        assert!(
            contrast(t.accent, t.on_accent) >= 4.5,
            "4.5:1 is the AA floor for body text and there is no reason to miss it"
        );

        // A deep accent flips the choice. This is the case a brightness threshold
        // gets wrong in the other direction, so it is the one worth pinning.
        let deep = Theme::build(
            Color32::from_rgb(18, 20, 25),
            Color32::from_rgb(30, 33, 40),
            Color32::from_rgb(48, 52, 62),
            Color32::from_rgb(235, 240, 248),
            Color32::from_rgb(170, 180, 196),
            Color32::from_rgb(126, 136, 152),
            Color32::from_rgb(36, 64, 120),
        );
        assert_eq!(deep.on_accent, deep.ink, "a dark accent takes light text");
        assert!(contrast(deep.accent, deep.on_accent) >= 4.5);
    }

    #[test]
    fn a_selected_card_still_reads_under_body_text() {
        // `accent_soft` is a wash, so the phrase lands *on* the accent and body
        // text has to survive it. A wash that swallowed `ink` would make the
        // selected row the hardest one to read -- backwards.
        let t = theme();
        assert!(
            contrast(t.accent_soft, t.ink) >= 7.0,
            "body text on a selected card is {}:1",
            contrast(t.accent_soft, t.ink)
        );
        assert!(luminance(t.accent_soft) > luminance(t.canvas), "the wash is visible");
    }

    #[test]
    fn disabled_text_recedes_without_disappearing() {
        let t = theme();
        assert!(
            contrast(t.canvas, t.ink_disabled) >= 2.0,
            "below 2:1 it stops being text and becomes a smudge"
        );
        assert!(contrast(t.canvas, t.ink_disabled) < contrast(t.canvas, t.ink_subtle));
    }

    #[test]
    fn the_two_built_in_themes_share_shape_and_differ_in_surface() {
        let (a, b) = (Theme::standard(), Theme::lite());
        assert_eq!(a.radius, b.radius, "shape is not a per-theme decision");
        assert_ne!(a.surface, b.surface);
        assert!(
            luminance(b.surface) > luminance(b.canvas),
            "lite still needs one step of separation or every card vanishes"
        );
    }

    #[test]
    fn a_user_palette_keeps_its_own_colours_and_derives_the_rest() {
        let colors = NoteColors::default();
        let t = Theme::from_colors(&colors);
        assert_eq!(t.canvas, rgb(colors.panel));
        assert_eq!(t.surface, rgb(colors.strip));
        assert_eq!(t.accent, rgb(colors.accent));
        // Derived, not copied from the user file: the note has no such field.
        assert_ne!(t.surface_soft, t.surface);
        assert!(contrast(t.accent, t.on_accent) >= 4.5);
        assert_eq!(t.radius, Radii::default());
    }

    #[test]
    fn mixing_hits_both_ends_exactly() {
        let a = Color32::from_rgb(10, 20, 30);
        let b = Color32::from_rgb(200, 210, 220);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        // Out-of-range `t` clamps instead of wrapping: a token computed with a
        // typo'd constant should look wrong, not invert.
        assert_eq!(mix(a, b, -3.0), a);
        assert_eq!(mix(a, b, 9.0), b);
    }
}
