//! The island's Esc handoff: platform key goes in, one egui frame comes out.
//!
//! This is a module rather than four lines inside `spike.rs` because it carries a
//! bug's worth of reasoning. The island's subclass proc runs inside a message
//! handler and must not re-enter window code, so it does not park the panel; it
//! raises a flag and lets `update` do it. That part was always right.
//!
//! What was missing is that **raising a flag does not schedule a frame**. Between
//! reveals this app draws nothing: `settle` requests no repaint, and the RichEdit
//! paints itself, so egui is idle while the user types. The key is also consumed
//! by the subclass proc, so winit never sees it and cannot wake the loop from its
//! side. The flag therefore sat unread until an unrelated message (a mouse move,
//! typically) happened to run `update` -- which is why closing the panel was
//! reported as "needs three presses", with the number depending on what else the
//! mouse was doing.
//!
//! So the callback asks for the frame at the source. The counter exists because
//! the two ways a press can go missing are fixed in different places: a press the
//! IME ate (candidate window open, composition active) never reaches the callback
//! at all, while a press that reaches it is counted here. `presses=` in the log
//! tells the two apart instead of leaving "it happened again" as the evidence.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Build the callback handed to `Editor::on_escape`.
///
/// `viewport` is explicit for the same reason [`crate::keys::Bus`] carries one:
/// with several notes open, a repaint request without a target wakes whichever
/// viewport ran last, and the Esc flag would wait for the note's own timer.
pub fn handler(
    flag: Arc<AtomicBool>,
    presses: Arc<AtomicU32>,
    ctx: egui::Context,
    viewport: egui::ViewportId,
) -> Box<dyn Fn() + Send + Sync> {
    Box::new(move || {
        presses.fetch_add(1, Ordering::SeqCst);
        flag.store(true, Ordering::SeqCst);
        ctx.request_repaint_of(viewport);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raises_the_flag_and_requests_a_frame() {
        let flag = Arc::new(AtomicBool::new(false));
        let presses = Arc::new(AtomicU32::new(0));
        let ctx = egui::Context::default();
        assert!(
            !ctx.has_requested_repaint(),
            "fixture must start with nothing pending, or the assertion below proves nothing"
        );

        handler(flag.clone(), presses.clone(), ctx.clone(), egui::ViewportId::ROOT)();

        assert!(
            flag.load(Ordering::SeqCst),
            "update() reads this flag; without it the panel cannot close"
        );
        assert_eq!(presses.load(Ordering::SeqCst), 1);
        assert!(
            ctx.has_requested_repaint(),
            "without a frame request the flag waits for an unrelated message -- the three-press bug"
        );
    }

    #[test]
    fn counts_every_press_so_a_swallowed_key_is_distinguishable() {
        let flag = Arc::new(AtomicBool::new(false));
        let presses = Arc::new(AtomicU32::new(0));
        let callback = handler(flag, presses.clone(), egui::Context::default(), egui::ViewportId::ROOT);

        callback();
        callback();
        callback();

        assert_eq!(
            presses.load(Ordering::SeqCst),
            3,
            "the count separates 'the IME ate it' from 'the flag waited for a frame'"
        );
    }
}
