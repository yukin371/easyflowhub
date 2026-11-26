//! A click in the body, carried from the control to the note's own frame.
//!
//! Q7's last clause is "a click outside the panel closes it", and almost all of
//! "outside" is the body — which is a RichEdit **child window** covering that
//! rectangle. egui never sees the press: the message goes to the child and stays
//! there. So the report has to come from the subclass proc, and from there it can
//! only be a flag: that proc runs inside a message handler and must not re-enter
//! window code.
//!
//! The flag then has to wake *this* note. A bare `request_repaint` wakes whichever
//! viewport runs next, and the click would sit unread until the 900 ms autosave
//! timer happened to draw the window — that is the "dismisses it a second later"
//! shape of bug, and it is why [`Clicks`] carries a viewport id.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Counts body clicks until the note's frame reads them.
#[derive(Clone)]
pub struct Clicks {
    count: Arc<AtomicU32>,
}

impl Clicks {
    pub fn new() -> Self {
        Self {
            count: Arc::new(AtomicU32::new(0)),
        }
    }

    /// The callback handed to `Editor::on_click`.
    pub fn handler(
        &self,
        ctx: egui::Context,
        viewport: egui::ViewportId,
    ) -> Box<dyn Fn() + Send + Sync> {
        let count = self.count.clone();
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            ctx.request_repaint_of(viewport);
        })
    }

    /// How many clicks arrived since the last frame, resetting the count.
    ///
    /// Counted rather than a bool so a gate can tell "no click arrived" from "the
    /// frame that read it also cleared the panel" — the two look the same in a
    /// single boolean.
    pub fn take(&self) -> u32 {
        self.count.swap(0, Ordering::SeqCst)
    }
}

impl Default for Clicks {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_is_counted_and_asks_for_a_frame() {
        let clicks = Clicks::new();
        let ctx = egui::Context::default();
        assert!(!ctx.has_requested_repaint(), "fixture must start idle");

        clicks.handler(ctx.clone(), egui::ViewportId::ROOT)();

        assert!(ctx.has_requested_repaint(), "the click must wake the note");
        assert_eq!(clicks.take(), 1);
        // Taking is a reset, not a read: a panel must not be dismissed twice for
        // one click, and a later frame must not think the body was clicked again.
        assert_eq!(clicks.take(), 0);
    }
}
