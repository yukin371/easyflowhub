//! Pure logic shared by the native shell: panel geometry maths, the spike's
//! settings shape, the note-text rules that decide what a save writes, and the
//! store that reads and writes the product's `notes.db`.
//!
//! Nothing here may touch a platform API. `scripts/check-native-architecture.mjs`
//! enforces that (no win32 bindings, no GUI toolkit — and it scans comments too, so
//! even naming them in prose trips it), and the reason is not purity for its own
//! sake: every millisecond spent in the reveal path has to be provably free of OS
//! calls, so the clamping rules live here with unit tests instead of inside the
//! hotkey thread.
//!
//! [`store`] is here because the design says persistence lives in `hub-core::store`
//! (section 5.7, rule 3) and nowhere else: the UI never holds a connection, and SQL
//! never escapes this crate. SQLite is a file format, not a platform call, so it
//! does not violate the rule above.

pub mod clock;
pub mod island;
pub mod layout;
pub mod note_text;
pub mod settings;
pub mod shortcuts;
pub mod store;
pub mod translation;

pub use island::PixelRect;
pub use layout::{
    place, place_note, NotePlacement, NoteSpec, PanelSpec, Placement, Rect, MIN_NOTE_HEIGHT,
    MIN_NOTE_WIDTH, STRIP_HEIGHT,
};
pub use settings::{
    NoteAppearance, NoteColors, QuickNoteSettings, Rgb, SettingsSource, SpikeSettings,
    TranslationSettings,
};
pub use shortcuts::{Chord, Command, Scope, Table};
pub use store::{new_note_id, Counts, NoteRow, NoteWrite, SettingRow, Store, StoreError};
