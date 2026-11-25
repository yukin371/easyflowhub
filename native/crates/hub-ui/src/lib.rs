//! Launcher panel rendering for the S1 spike.
//!
//! Scope is deliberately narrow: enough real content (header, search row, action
//! grid, footer hints) that the first frame has to rasterise the same glyph mix a
//! real panel would. A "hello world" panel would warm the atlas for a handful of
//! characters and make the 60 ms measurement meaningless.

pub mod panel;
pub mod quicknote;
pub mod theme;

pub use panel::{first_screen_text, PanelState, PANEL_PADDING};
pub use quicknote::{opacity_label, slots, StripState};
pub use theme::Palette;
