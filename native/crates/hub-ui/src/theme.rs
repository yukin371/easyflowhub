//! Palette and font setup.
//!
//! The panel is a dark action surface in both themes (§5.4 keeps the launcher out
//! of the document theme), so the palette is fixed here rather than derived from
//! the OS. The quick-note surface is the exception: its colours come from the
//! settings file (`quick_note.appearance.colors`), because the note is the one
//! place where a user stares at the same pixels for minutes at a time.

use egui::{Color32, FontData, FontDefinitions, FontFamily};
use hub_core::{NoteColors, Rgb};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub panel: Color32,
    /// The note's title strip; the launcher uses it for raised cards.
    pub panel_strong: Color32,
    pub border: Color32,
    pub ink: Color32,
    pub ink_soft: Color32,
    pub ink_subtle: Color32,
    pub accent: Color32,
}

impl Palette {
    pub fn standard() -> Self {
        Self {
            panel: Color32::from_rgb(24, 27, 33),
            panel_strong: Color32::from_rgb(33, 37, 45),
            border: Color32::from_rgb(51, 57, 68),
            ink: Color32::from_rgb(232, 238, 247),
            ink_soft: Color32::from_rgb(168, 178, 194),
            ink_subtle: Color32::from_rgb(124, 134, 150),
            accent: Color32::from_rgb(126, 165, 214),
        }
    }

    /// `lite` keeps the same layout but drops the extra surfaces so nothing has
    /// to be composited twice.
    pub fn lite() -> Self {
        Self {
            panel: Color32::from_rgb(20, 22, 27),
            panel_strong: Color32::from_rgb(20, 22, 27),
            border: Color32::from_rgb(46, 51, 60),
            ink: Color32::from_rgb(226, 232, 240),
            ink_soft: Color32::from_rgb(158, 167, 181),
            ink_subtle: Color32::from_rgb(120, 130, 145),
            accent: Color32::from_rgb(126, 165, 214),
        }
    }

    /// The quick-note window, from the user's `appearance.colors`.
    ///
    /// `styles.css` defines the shipped tokens as `rgba(...)` over `--manager-bg`;
    /// the defaults in `NoteColors` already carry the composited value, so no
    /// blending happens here. Doing the maths once, in `hub-core`, keeps "the
    /// native note is slightly bluer than the Tauri one" from becoming an
    /// unexplained visual difference.
    pub fn from_colors(colors: &NoteColors) -> Self {
        Self {
            panel: rgb(colors.panel),
            panel_strong: rgb(colors.strip),
            border: rgb(colors.border),
            ink: rgb(colors.ink),
            ink_soft: rgb(colors.ink_soft),
            ink_subtle: rgb(colors.ink_subtle),
            accent: rgb(colors.accent),
        }
    }

    /// The shipped quick-note palette, i.e. the default `NoteColors`.
    pub fn quick() -> Self {
        Self::from_colors(&NoteColors::default())
    }
}

fn rgb(color: Rgb) -> Color32 {
    Color32::from_rgb(color[0], color[1], color[2])
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
