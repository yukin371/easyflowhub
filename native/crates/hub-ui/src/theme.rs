//! Palette and font setup.
//!
//! The panel is a dark action surface in both themes (§5.4 keeps the launcher out
//! of the document theme), so the palette is fixed here rather than derived from
//! the OS. The accent is the only thing that could later follow the user's choice.

use egui::{Color32, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub panel: Color32,
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

    /// The quick-note window, sampled from the shipped `--quick-*` tokens.
    ///
    /// `styles.css` defines them as `rgba(...)` over `--manager-bg`; egui wants
    /// opaque colours, so the translucent ones are composited here. Doing the
    /// maths in one place keeps "the native note is slightly bluer than the Tauri
    /// one" from becoming an unexplained visual difference.
    pub fn quick() -> Self {
        Self {
            // --manager-bg #0d1522
            panel: Color32::from_rgb(13, 21, 34),
            // --manager-panel-strong-opaque #1f2e44
            panel_strong: Color32::from_rgb(31, 46, 68),
            // rgba(166,193,226,0.2) over #0d1522
            border: Color32::from_rgb(38, 55, 72),
            // --manager-ink #dce6f4
            ink: Color32::from_rgb(220, 230, 244),
            // --manager-ink-soft #afbed2
            ink_soft: Color32::from_rgb(175, 190, 210),
            // --manager-ink-muted #8a9bb2
            ink_subtle: Color32::from_rgb(138, 155, 178),
            // --manager-accent #86abdc
            accent: Color32::from_rgb(134, 171, 220),
        }
    }
}

/// Windows ships Microsoft YaHei; loading it is a file read, not a platform call,
/// which is why it belongs here and not in `hub-win`.
const CJK_CANDIDATES: [&str; 3] = [
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyhl.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
];

/// Install a CJK font and return the path that was used, if any.
///
/// Chinese text with the default egui font stack renders as tofu, which would
/// make the timing meaningless in a different way: the atlas would have nothing
/// to rasterise. A missing font is logged by the caller and the panel still runs.
pub fn install_fonts(ctx: &egui::Context) -> Option<String> {
    let (path, bytes) = CJK_CANDIDATES.iter().find_map(|path| {
        std::fs::read(path).ok().map(|bytes| (*path, bytes))
    })?;

    let mut definitions = FontDefinitions::default();
    definitions.font_data.insert(
        "cjk".to_string(),
        Arc::new(FontData::from_owned(bytes)),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        definitions
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk".to_string());
    }
    ctx.set_fonts(definitions);
    Some(path.to_string())
}
