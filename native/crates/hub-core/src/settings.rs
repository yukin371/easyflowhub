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

/// Note window size, logical pixels.
///
/// Field names match the shipped Tauri `QuickNoteSettings` on purpose: when the
/// two apps eventually read one settings file, `quick_note.width` must already
/// mean the same thing on both sides. Translating between `note_width` and
/// `quick_note.width` is the kind of mapping that silently stops being applied.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct QuickNoteSettings {
    pub width: u32,
    pub height: u32,
}

impl Default for QuickNoteSettings {
    fn default() -> Self {
        Self {
            width: 400,
            height: 300,
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
}
