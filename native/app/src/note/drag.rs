//! Q10: moving the window by its title strip.
//!
//! ## Why not `ViewportCommand::StartDrag`
//!
//! It is the one-line answer and it does not move the window here. Measured
//! 2026-11-26: `StartDrag` fires (the frame logs the branch), winit posts
//! `WM_NCLBUTTONDOWN` with `HTCAPTION`, and `GetWindowRect` is unchanged when the
//! button comes up. The same probe against a plain `WS_OVERLAPPEDWINDOW` built next
//! to it — `SendMessage(WM_NCLBUTTONDOWN, HTCAPTION)` after a synthetic press and a
//! dozen move steps — also leaves it at its origin, so the move loop is not
//! negotiating with this environment at all, and the note's no-activate/popup style
//! is not the reason.
//!
//! So the window is moved here instead: once per frame, by the distance the cursor
//! has travelled since the previous frame. That needs no modal loop and runs on the
//! same pointer stream the buttons already use.
//!
//! ## Per-frame distance, not distance from the press
//!
//! The first version anchored both the cursor and the window at the press and moved
//! the window to `start_origin + (cursor - start_cursor)`. It is exact when the
//! anchor is taken on the frame the press arrives, and it is wrong by the whole
//! anchor error when it is not: one measured run dragged a window 107 px for a
//! 96 px gesture because the anchor was read before the cursor reached the press
//! point. Adding per-frame deltas cannot accumulate that error — each frame asks
//! where the window *is* and moves it by how far the cursor went — and a late
//! anchor now costs one frame of lag instead of a visible jump.
//!
//! ## The cursor has to come from the screen, not from egui
//!
//! egui reports the pointer in client coordinates, and the client is what is being
//! moved: after the first `SetWindowPos` the pointer's client position is the same
//! as before, so a delta computed from it is zero and the window stays put. Screen
//! coordinates do not chase themselves — `GetCursorPos` is the frame of reference
//! that stays still while the window moves under the cursor.
//!
//! ## Where "a press on a button is not a drag" lives
//!
//! In `quicknote`: the drag surface is registered before the buttons and a press on
//! either button rect is excluded from `drag_started` (see §8.10). This module only
//! runs when that decision has already been made, so it never has to know where the
//! buttons are.

use super::NoteShell;

/// The drag in progress: where the cursor was on the previous frame.
///
/// `None` means "not dragging". The window's own origin is re-read every frame
/// instead of being remembered, so a move made by anything else (a snap, a monitor
/// change) is followed rather than fought.
#[derive(Debug, Default)]
pub(super) struct Drag {
    cursor: Option<(i32, i32)>,
}

impl Drag {
    pub(super) fn active(&self) -> bool {
        self.cursor.is_some()
    }

    /// Remember where the pointer is. `false` when there is no window to move.
    fn begin(&mut self, hwnd: Option<isize>) -> bool {
        if hwnd.is_none() {
            return false;
        }
        self.cursor = Some(hub_win::window::cursor_position());
        true
    }

    /// One frame of following. Returns `false` when the window could not be moved.
    fn follow(&mut self, hwnd: Option<isize>) -> bool {
        let (Some(hwnd), Some(previous)) = (hwnd, self.cursor) else {
            return false;
        };
        let now = hub_win::window::cursor_position();
        self.cursor = Some(now);
        let (dx, dy) = (now.0 - previous.0, now.1 - previous.1);
        if dx == 0 && dy == 0 {
            return true;
        }
        let (x, y) = hub_win::window::client_origin(hwnd);
        hub_win::window::move_to(hwnd, x + dx, y + dy)
    }
}

impl NoteShell {
    /// A press on the strip that is not on a button: start following the cursor.
    pub(super) fn begin_drag(&mut self) {
        if !self.drag.begin(self.hwnd) {
            return;
        }
        self.log
            .mark(0, "note_drag", &format!("id={} phase=begin", self.id));
    }

    /// Called every frame. Does nothing unless a drag is in progress.
    ///
    /// The release is read from egui rather than from `GetKeyState`: egui knows the
    /// press began in this window, so a release outside the window still ends the
    /// drag instead of leaving the window glued to the cursor.
    pub(super) fn drag_frame(&mut self, ctx: &egui::Context) {
        if !self.drag.active() {
            return;
        }
        if !ctx.input(|input| input.pointer.primary_down()) {
            self.drag.cursor = None;
            let origin = self.hwnd.map(hub_win::window::client_origin);
            self.log.mark(
                0,
                "note_drag",
                &format!("id={} phase=end origin={origin:?}", self.id),
            );
            return;
        }
        self.drag.follow(self.hwnd);
    }
}
