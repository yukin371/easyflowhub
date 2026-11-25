//! Settings the spike needs at startup.
//!
//! The point of loading them once here is the rule from §5.8.1: **the reveal path
//! must not read settings**. A file read is a syscall that can block on a cold
//! cache or a network profile; doing it when the hotkey fires is exactly how a
//! 60 ms budget becomes 200 ms.
//!
//! The real product reads `notes.db`; the spike reads a small JSON file so that
//! the measurement is not contaminated by storage work that S3 will measure
//! separately.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// One colour, `[r, g, b]`.
///
/// Arrays instead of `#rrggbb` strings because the value crosses into two
/// consumers that both speak bytes: egui's `Color32` and GDI's colourref. A hex
/// parser here would be one more place for "why is my colour off by one" to hide.
pub type Rgb = [u8; 3];

/// Colours of the quick-note surface.
///
/// Defaults are the sampled `--manager-*` tokens the Tauri build uses, so an
/// untouched config looks like the shipped note rather than like a second design.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct NoteColors {
    /// Text island background, also the window body.
    pub panel: Rgb,
    /// Title strip.
    pub strip: Rgb,
    /// Hairlines under the strip and around buttons.
    pub border: Rgb,
    /// Body text.
    pub ink: Rgb,
    /// The `100%` label and inactive glyphs.
    pub ink_soft: Rgb,
    /// Glyphs that are neither hovered nor active.
    pub ink_subtle: Rgb,
    /// Active button, ghost label, and the caret if a self-drawn one ever lands.
    pub accent: Rgb,
}

impl Default for NoteColors {
    fn default() -> Self {
        Self {
            panel: [13, 21, 34],
            strip: [31, 46, 68],
            border: [38, 55, 72],
            ink: [220, 230, 244],
            ink_soft: [175, 190, 210],
            ink_subtle: [138, 155, 178],
            accent: [134, 171, 220],
        }
    }
}

/// Typography and spacing of the note body.
///
/// Every value here is *logical*: the editor multiplies by the monitor scale
/// when it builds the physical rectangle. `font_pt` is the exception — points
/// are already device independent, which is exactly why the old code that
/// multiplied it by `pixels_per_point` rendered 16.5 pt on a 150% screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NoteAppearance {
    pub font_family: String,
    pub font_pt: f32,
    /// Multiple of the font's natural line height. `1.0` keeps RichEdit's own
    /// spacing, which is what the body is tuned to: measured at 150% with this
    /// face and 11.5 pt, one line is 26.7 logical px (a 22 px glyph plus 18 px of
    /// leading), and 1.35 put 32 px between glyphs — visibly airy for a scratch pad.
    pub line_spacing: f32,
    /// Inset between the island edge and the text, logical pixels.
    ///
    /// 18 is the shipped `EditorTextarea`'s `padding: 18` (`quick-note` variant),
    /// and it is what makes the body line up with the strip: the opacity label
    /// sits at 16, so the text starts 2 px to its right, which is the shipped
    /// relationship. The 12/10 pair used earlier left the body 4 px *left* of
    /// the label — a small misfit that reads as "unfinished" without being
    /// nameable.
    pub padding_x: i32,
    pub padding_y: i32,
    /// Caret thickness in **logical** pixels. `0` leaves the system value alone.
    ///
    /// Non-zero writes the **system-wide** caret width (`SPI_SETCARETWIDTH`) and
    /// restores the previous value when the editor is dropped: Windows has no
    /// per-window caret width, and a 1 px caret on a dark background is the
    /// complaint this field answers. See `hub_editor::Theme::caret_width`.
    pub caret_width: i32,
    /// Opacity a **new** note opens at, `0.0..=1.0` (parity Q28/Q29).
    ///
    /// The only field in this block that is **user state, not theme decoration**:
    /// it is the initial value of the window's alpha, read once when the note is
    /// created. From then on each note owns its own value, because `Ctrl+wheel`
    /// dims the window under the pointer -- a shared "current opacity" would dim
    /// every open note at once, which the shipped build never does.
    ///
    /// Defaults to 1.0; `0.0` is a legal setting and opens the note in ghost mode.
    pub window_alpha: f32,
    pub colors: NoteColors,
}

impl Default for NoteAppearance {
    fn default() -> Self {
        Self {
            font_family: "Microsoft YaHei UI".to_string(),
            font_pt: 11.5,
            line_spacing: 1.0,
            padding_x: 18,
            padding_y: 18,
            caret_width: 2,
            window_alpha: 1.0,
            colors: NoteColors::default(),
        }
    }
}

impl NoteAppearance {
    /// Opacity for a note that is being created, clamped to what the compositor
    /// can express.
    ///
    /// The file is hand-editable, so an out-of-range or `NaN` alpha has to be
    /// survivable: `NaN` is the dangerous one, because every comparison against it
    /// is false and a clamp written as `if x < 0 { … } else if x > 1 { … }` would
    /// pass it through to `SetLayeredWindowAttributes`. Falling back to opaque is
    /// the honest direction -- an invisible note with no explanation is worse than
    /// one that ignores a typo.
    pub fn initial_opacity(&self) -> f32 {
        if self.window_alpha.is_finite() {
            self.window_alpha.clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// Note window size, logical pixels.
///
/// Field names match the shipped Tauri `QuickNoteSettings` on purpose: when the
/// two apps eventually read one settings file, `quick_note.width` must already
/// mean the same thing on both sides. Translating between `note_width` and
/// `quick_note.width` is the kind of mapping that silently stops being applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct QuickNoteSettings {
    pub width: u32,
    pub height: u32,
    /// Not in the Tauri struct: an unknown key is ignored there, and `serde(default)`
    /// here means a file without `appearance` still loads with the sampled theme.
    pub appearance: NoteAppearance,
}

impl Default for QuickNoteSettings {
    fn default() -> Self {
        Self {
            width: 400,
            height: 300,
            appearance: NoteAppearance::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SpikeSettings {
    /// Logical size of the launcher panel (§5.1: 440x860, taken from the shipped
    /// `tauri.conf.json` so the spike measures the real shape).
    pub panel_width: f32,
    pub panel_height: f32,
    /// Gap to the work-area edge, physical pixels.
    pub margin: i32,
    /// `Ctrl+Shift+Space` in the product. Overridable so a bench run can move to
    /// a combination that is certainly free.
    pub hotkey: String,
    /// Force the `lite` profile (remote session detection is automatic).
    pub lite: bool,
    /// Two presses closer than this are one press: screenshot tools hook the
    /// same combination and used to make one press toggle the panel three times.
    pub hotkey_debounce_ms: u64,
    /// Keep the panel visible this long during a bench run before hiding it.
    pub bench_visible_ms: u64,
    /// Note window size (parity Q2), logical pixels.
    pub quick_note: QuickNoteSettings,
}

impl Default for SpikeSettings {
    fn default() -> Self {
        Self {
            panel_width: 440.0,
            panel_height: 860.0,
            margin: 12,
            hotkey: "Ctrl+Shift+Space".to_string(),
            lite: false,
            hotkey_debounce_ms: 260,
            bench_visible_ms: 240,
            quick_note: QuickNoteSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSource {
    File,
    Default,
}

impl SettingsSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Default => "default",
        }
    }
}

impl SpikeSettings {
    /// Load from `path`, falling back to defaults.
    ///
    /// A corrupt or unreadable file is **not** an error: refusing to start
    /// because of a settings file is a far worse failure than starting with the
    /// documented defaults, and the caller logs which of the two happened.
    pub fn load(path: &Path) -> (Self, SettingsSource) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return (Self::default(), SettingsSource::Default);
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(settings) => (settings, SettingsSource::File),
            Err(_) => (Self::default(), SettingsSource::Default),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_shipped_panel_size() {
        let settings = SpikeSettings::default();
        assert_eq!(settings.panel_width, 440.0);
        assert_eq!(settings.panel_height, 860.0);
        assert!(!settings.lite);
    }

    #[test]
    fn missing_file_falls_back_to_defaults() {
        let (settings, source) = SpikeSettings::load(Path::new("does/not/exist.json"));
        assert_eq!(source, SettingsSource::Default);
        assert_eq!(settings, SpikeSettings::default());
    }

    #[test]
    fn partial_json_keeps_the_other_defaults() {
        let dir = std::env::temp_dir().join("hub-core-settings-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("partial.json");
        std::fs::write(&path, r#"{"lite": true}"#).unwrap();

        let (settings, source) = SpikeSettings::load(&path);
        assert_eq!(source, SettingsSource::File);
        assert!(settings.lite);
        assert_eq!(settings.panel_height, 860.0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn note_defaults_match_the_shipped_quick_note_window() {
        // 400x300 is what the Tauri build creates; parity Q2 is judged against it.
        let settings = SpikeSettings::default();
        assert_eq!(settings.quick_note.width, 400);
        assert_eq!(settings.quick_note.height, 300);
    }

    #[test]
    fn a_settings_file_can_resize_the_note_without_touching_the_panel() {
        let dir = std::env::temp_dir().join("hub-core-settings-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("quick-note.json");
        std::fs::write(&path, r#"{"quick_note":{"width":500,"height":400}}"#).unwrap();

        let (settings, source) = SpikeSettings::load(&path);
        assert_eq!(source, SettingsSource::File);
        assert_eq!(settings.quick_note.width, 500);
        assert_eq!(settings.quick_note.height, 400);
        // The launcher size must not have moved with it.
        assert_eq!(settings.panel_width, 440.0);
        assert_eq!(settings.panel_height, 860.0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn appearance_defaults_keep_the_sampled_theme() {
        let appearance = NoteAppearance::default();
        // The body palette is the shipped `--manager-*` set, composited once.
        assert_eq!(appearance.colors.panel, [13, 21, 34]);
        assert_eq!(appearance.colors.ink, [220, 230, 244]);
        // The shipped textarea's `padding: 18`, which puts the body 2 px right of
        // the strip label (16) instead of 4 px left of it.
        assert_eq!(appearance.padding_x, 18);
        assert_eq!(appearance.padding_y, 18);
        assert_eq!(appearance.caret_width, 2);
    }

    #[test]
    fn a_colour_override_keeps_the_other_fields() {
        let dir = std::env::temp_dir().join("hub-core-settings-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("colour.json");
        std::fs::write(
            &path,
            r#"{"quick_note":{"appearance":{"colors":{"accent":[255,64,64]}}}}"#,
        )
        .unwrap();

        let (settings, source) = SpikeSettings::load(&path);
        assert_eq!(source, SettingsSource::File);
        assert_eq!(settings.quick_note.appearance.colors.accent, [255, 64, 64]);
        // `serde(default)` on both levels: the untouched colours survive.
        assert_eq!(settings.quick_note.appearance.colors.panel, [13, 21, 34]);
        assert_eq!(settings.quick_note.width, 400);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_json_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("hub-core-settings-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("corrupt.json");
        std::fs::write(&path, "{not json").unwrap();

        let (settings, source) = SpikeSettings::load(&path);
        assert_eq!(source, SettingsSource::Default);
        assert_eq!(settings, SpikeSettings::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_note_opens_fully_opaque_unless_the_file_says_otherwise() {
        assert_eq!(NoteAppearance::default().window_alpha, 1.0);
        assert_eq!(NoteAppearance::default().initial_opacity(), 1.0);
    }

    #[test]
    fn the_opacity_setting_can_open_a_note_in_ghost_mode() {
        let dir = std::env::temp_dir().join("hub-core-settings-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("alpha.json");
        std::fs::write(
            &path,
            r#"{"quick_note":{"appearance":{"window_alpha":0.4}}}"#,
        )
        .unwrap();

        let (settings, source) = SpikeSettings::load(&path);
        assert_eq!(source, SettingsSource::File);
        assert_eq!(settings.quick_note.appearance.initial_opacity(), 0.4);
        // Untouched neighbours keep their defaults, which is what makes the key
        // safe to add to an existing file.
        assert_eq!(settings.quick_note.appearance.font_pt, 11.5);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_hand_edited_opacity_is_clamped_including_nan() {
        // `NaN` is the one that a naive `if x < 0 … else if x > 1 …` clamp lets
        // through, and it would reach `SetLayeredWindowAttributes` as a 0 byte.
        let mut appearance = NoteAppearance::default();
        appearance.window_alpha = f32::NAN;
        assert_eq!(appearance.initial_opacity(), 1.0);

        appearance.window_alpha = 1.7;
        assert_eq!(appearance.initial_opacity(), 1.0);

        appearance.window_alpha = -0.2;
        assert_eq!(appearance.initial_opacity(), 0.0);

        appearance.window_alpha = 0.0;
        assert_eq!(appearance.initial_opacity(), 0.0, "zero is a legal value");
    }
}
