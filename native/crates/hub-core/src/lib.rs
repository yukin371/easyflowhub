//! Pure logic shared by the native shell: panel geometry maths, the spike's
//! settings shape, and the store that reads and writes the product's `notes.db`.
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

pub mod island;
pub mod layout;
pub mod settings;
pub mod store;

pub use island::PixelRect;
pub use layout::{place, PanelSpec, Placement, Rect};
pub use settings::{SettingsSource, SpikeSettings};
pub use store::{Counts, NoteRow, SettingRow, Store, StoreError};
