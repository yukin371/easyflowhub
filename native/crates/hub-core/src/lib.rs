//! Pure logic shared by the native shell: panel geometry maths and the spike's
//! settings shape.
//!
//! Nothing here may touch a platform API. `scripts/check-native-architecture.mjs`
//! enforces that (no win32 bindings, no GUI toolkit — and it scans comments too, so
//! even naming them in prose trips it), and the reason is not purity for its own
//! sake: every millisecond spent in the reveal path has to be provably free of OS
//! calls, so the clamping rules live here with unit tests instead of inside the
//! hotkey thread.

pub mod layout;
pub mod settings;

pub use layout::{place, PanelSpec, Placement, Rect};
pub use settings::{SettingsSource, SpikeSettings};
