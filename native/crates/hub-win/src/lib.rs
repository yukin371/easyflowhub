//! Platform layer for the native shell: hotkey, monitor layout, DWM, process
//! probes and window placement.
//!
//! Design constraints this crate exists to hold:
//!
//! * **No UI dependency** — the guard rejects `egui`/`eframe` here, so a widget can
//!   never start owning a `HWND` lifecycle decision.
//! * **No `hub-core` dependency** — the edge matrix forbids it, which keeps
//!   knowledge flowing one way: the app computes geometry with `hub-core` and hands
//!   `hub-win` plain numbers.
//! * **Cacheable platform state lives here**, not in the reveal path: the monitor
//!   layout is read once at startup (`MonitorCache`), because enumerating monitors
//!   from a hotkey callback costs tens of milliseconds under mixed DPI.

pub mod dpi;
pub mod dwm;
pub mod hotkey;
pub mod monitor;
pub mod probe;
pub mod tray;
pub mod window;

pub use hotkey::{Binding, Hotkey, HotkeySet, PressHandler};
pub use tray::{Tray, TrayCommand};
pub use monitor::{cursor_pos, MonitorCache, MonitorRect};
pub use probe::{is_remote_session, ProcessProbe};
