//! Q3: the note's own edge is the resize grip.
//!
//! ## Why the edge is drawn by hand
//!
//! The window is undecorated (`with_decorations(false)`), and winit's resize
//! border does not exist without a frame: measured 2026-11-26, `WM_NCHITTEST`
//! answers `HTCLIENT` for every edge even though the style carries
//! `WS_THICKFRAME`. So the OS never starts a resize loop, and there is nothing to
//! inherit. The same measurement is why the *move* loop is hand-drawn too (see
//! `note/drag.rs`) -- one environment, one answer.
//!
//! ## Where the press is seen
//!
//! Two windows, because the note is two windows. The strip is egui's, and a press
//! there is the same `Sense::drag()` the title-bar move uses. Everything below the
//! strip is the RichEdit island, which owns the mouse: egui never sees a press
//! there. That half arrives through `hub_editor`'s press hook, which can **claim**
//! the message -- the control must not place the caret and start a selection while
//! the window is being resized under the pointer.
//!
//! ## Why it follows the cursor per frame
//!
//! Same reason as the move: there is no modal loop to hand the gesture to. Each
//! frame asks where the window *is* and applies the distance the cursor travelled
//! since the previous frame, so a late or missed frame costs one frame of lag
//! instead of a permanent offset. The button itself comes from
//! `GetAsyncKeyState`: during an edge drag the mouse messages go to the island's
//! control, and egui's `pointer.primary_down()` knows only about presses that
//! landed in egui's own window.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

use hub_core::{MIN_NOTE_HEIGHT, MIN_NOTE_WIDTH};
use hub_editor::{Press, Shape};

use crate::log::Log;

use super::NoteShell;

/// Width of the grip, in logical pixels.
///
/// Five is narrow enough that the band reads as "the edge" and wide enough to hit
/// without aiming: a native resize border is `SM_CXSIZEFRAME +
/// SM_CXPADDEDBORDER` (8 px at 100%), and this note has no border to hide behind,
/// so a little more than the visible edge is the honest target.
pub const BAND: f32 = 5.0;

/// Which edges of the window the gesture moves.
///
/// No `top`: the top edge is the title strip, and a press there already means
/// "move the window" (Q10). Two meanings for the same five pixels would make the
/// most-used gesture fail near the top of the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Grip {
    pub left: bool,
    pub right: bool,
    pub bottom: bool,
}

impl Grip {
    pub fn any(self) -> bool {
        self.left || self.right || self.bottom
    }

    /// For the log: which edges the gesture is holding, in one word.
    pub fn label(self) -> &'static str {
        match (self.left, self.right, self.bottom) {
            (true, false, true) => "bottom-left",
            (false, true, true) => "bottom-right",
            (true, false, false) => "left",
            (false, true, false) => "right",
            (false, false, true) => "bottom",
            _ => "none",
        }
    }
}

/// Which edges a press at `(x, y)` in a `width × height` rectangle grabs.
///
/// The guard is not cosmetic: a window narrower than four bands would answer
/// "both left and right", and the two clauses of [`resized`] would then cancel.
/// A transient 1-pixel client area (measured during creation) is exactly that
/// case, and the honest answer there is "no grip".
pub fn grip_at(x: f32, y: f32, width: f32, height: f32, band: f32) -> Grip {
    if band <= 0.0 || width <= band * 4.0 || height <= band * 4.0 {
        return Grip::default();
    }
    Grip {
        left: x <= band,
        right: x >= width - band,
        // The bottom edge only. The top edge belongs to the strip.
        bottom: y >= height - band,
    }
}

/// The window rectangle after one frame of a gesture.
///
/// `current` and the answer are `(x, y, w, h)` in physical pixels; `min` is the
/// smallest legal *client* size, already scaled. Every edge that is not held keeps
/// its coordinate, which is the whole contract: dragging the left edge must not
/// move the right one.
///
/// Clamping happens here rather than being left to the OS. `with_min_inner_size`
/// is enforced by `WM_GETMINMAXINFO`, so the OS would refuse the width and accept
/// the move -- the window would slide left while its right edge stayed put, which
/// is the jitter this function exists to prevent.
pub fn resized(
    current: (i32, i32, i32, i32),
    grip: Grip,
    delta: (i32, i32),
    min: (i32, i32),
) -> (i32, i32, i32, i32) {
    let (x, y, width, height) = current;
    let (dx, dy) = delta;

    let mut width = match (grip.left, grip.right) {
        (true, false) => width - dx,
        (false, true) => width + dx,
        _ => width,
    };
    let mut x = if grip.left { x + dx } else { x };
    if width < min.0 {
        width = min.0;
        if grip.left {
            // Pin the right edge: it is the one the user is not holding.
            x = current.0 + current.2 - min.0;
        }
    }

    let mut height = if grip.bottom { height + dy } else { height };
    if height < min.1 {
        height = min.1;
    }

    (x, y, width.max(0), height.max(0))
}

/// What the island's press hook writes and the frame loop reads.
///
/// The hook runs inside the control's message procedure: it must answer "is this
/// press mine?" before the caret moves, so it cannot wait for a frame. It answers,
/// stores the grip, and asks for the frame that starts the gesture.
pub struct Signal {
    /// Grip width in physical pixels; `5 * ppp`, updated when the window's scale
    /// changes.
    band: AtomicI32,
    grip: Mutex<Option<Grip>>,
    /// Armoured late: the shell is built before the note's viewport runs, and the
    /// hook cannot ask for a repaint without a context. It is set the moment the
    /// island exists, which is before any press can reach the hook.
    ctx: Mutex<Option<egui::Context>>,
    viewport: egui::ViewportId,
}

impl Signal {
    pub fn new(viewport: egui::ViewportId, scale: f32) -> Self {
        Self {
            band: AtomicI32::new(physical_band(scale)),
            grip: Mutex::new(None),
            ctx: Mutex::new(None),
            viewport,
        }
    }

    /// Hand over the context that can wake this note's viewport.
    pub fn arm(&self, ctx: &egui::Context) {
        if let Ok(mut slot) = self.ctx.lock() {
            *slot = Some(ctx.clone());
        }
    }

    /// Called when `pixels_per_point` changes. The message coordinates are
    /// physical, so the band has to be too.
    pub fn set_scale(&self, scale: f32) {
        self.band.store(physical_band(scale), Ordering::Relaxed);
    }

    /// The press hook itself. `true` claims the message.
    pub fn on_press(&self, press: Press) -> bool {
        let Some(grip) = self.grip_of(&press) else {
            return false;
        };
        if let Ok(mut slot) = self.grip.lock() {
            *slot = Some(grip);
        }
        // The frame that starts the gesture. Without it the press would sit in the
        // slot until some unrelated input produced a repaint -- and the gesture
        // would begin with the cursor already somewhere else.
        if let Ok(ctx) = self.ctx.lock() {
            if let Some(ctx) = ctx.as_ref() {
                ctx.request_repaint_of(self.viewport);
            }
        }
        true
    }

    /// The hover hook: the same band, answering with a cursor instead of claiming
    /// a press.
    pub fn on_hover(&self, press: Press) -> Option<Shape> {
        let grip = self.grip_of(&press)?;
        Some(match (grip.left || grip.right, grip.bottom) {
            (true, true) if grip.left => Shape::NorthEastSouthWest,
            (true, true) => Shape::NorthWestSouthEast,
            (true, false) => Shape::Horizontal,
            _ => Shape::Vertical,
        })
    }

    fn grip_of(&self, press: &Press) -> Option<Grip> {
        let band = self.band.load(Ordering::Relaxed) as f32;
        let grip = grip_at(
            press.x as f32,
            press.y as f32,
            press.width as f32,
            press.height as f32,
            band,
        );
        grip.any().then_some(grip)
    }

    fn take(&self) -> Option<Grip> {
        self.grip.lock().ok().and_then(|mut slot| slot.take())
    }
}

fn physical_band(scale: f32) -> i32 {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    (BAND * scale).round().max(1.0) as i32
}

/// The gesture in progress: the edges being held, and where the cursor was on the
/// previous frame.
#[derive(Debug, Default)]
pub(super) struct Resize {
    grip: Grip,
    cursor: (i32, i32),
}

impl Resize {
    pub(super) fn active(&self) -> bool {
        self.grip.any()
    }

    fn begin(&mut self, hwnd: isize, grip: Grip, log: &Log, id: u32) -> bool {
        let (x, y, width, height) = hub_win::window::window_rect(hwnd);
        if width <= 0 || height <= 0 {
            return false;
        }
        self.grip = grip;
        self.cursor = hub_win::window::cursor_position();
        log.mark(
            0,
            "note_resize",
            &format!(
                "id={id} phase=begin grip={} rect={width}x{height}+{x}+{y}",
                grip.label()
            ),
        );
        true
    }

    /// One frame of following. Returns `false` when the window could not be moved.
    fn follow(&mut self, hwnd: isize, min: (i32, i32)) -> bool {
        let now = hub_win::window::cursor_position();
        let delta = (now.0 - self.cursor.0, now.1 - self.cursor.1);
        self.cursor = now;
        if delta == (0, 0) {
            return true;
        }
        let current = hub_win::window::window_rect(hwnd);
        if current.2 <= 0 || current.3 <= 0 {
            return false;
        }
        let (x, y, width, height) = resized(current, self.grip, delta, min);
        hub_win::window::set_frame(hwnd, x, y, width, height)
    }

    fn end(&mut self, hwnd: Option<isize>, log: &Log, id: u32) {
        let grip = std::mem::take(&mut self.grip);
        let rect = hwnd
            .map(hub_win::window::window_rect)
            .unwrap_or((0, 0, 0, 0));
        log.mark(
            0,
            "note_resize",
            &format!(
                "id={id} phase=end grip={} rect={}x{}+{}+{}",
                grip.label(),
                rect.2,
                rect.3,
                rect.0,
                rect.1
            ),
        );
    }
}

impl NoteShell {
    /// The press the island claimed, if any.
    ///
    /// Called only when no gesture is in progress: the hook stores at most one
    /// grip, and the frame that consumes it starts the gesture.
    pub(super) fn edge_press(&self) -> Option<Grip> {
        self.edge.take()
    }

    /// A press on the strip that landed in the left/right band.
    ///
    /// Returns `true` when the press is the resize's, so the caller does not also
    /// start a window move. The strip owns the top edge, so only the vertical
    /// bands are consulted here.
    pub(super) fn begin_edge_from_strip(
        &mut self,
        pointer: egui::Pos2,
        size: egui::Vec2,
    ) -> bool {
        let Some(hwnd) = self.hwnd else {
            return false;
        };
        // Logical units on this side: the strip is laid out in points.
        let grip = Grip {
            // The strip is not the bottom edge, whatever the pointer's y says.
            bottom: false,
            ..grip_at(pointer.x, pointer.y, size.x, size.y, BAND)
        };
        if !grip.any() {
            return false;
        }
        self.resize.begin(hwnd, grip, &self.log, self.id)
    }

    /// Start or continue an edge drag. Called once per frame, next to the
    /// title-bar move.
    pub(super) fn resize_frame(&mut self, ctx: &egui::Context) {
        let Some(hwnd) = self.hwnd else {
            return;
        };
        if !self.resize.active() {
            let Some(grip) = self.edge_press() else {
                return;
            };
            if !self.resize.begin(hwnd, grip, &self.log, self.id) {
                return;
            }
        }
        // Read before moving: a press that is released between two frames is a
        // click on the edge, and it must not resize by however much the cursor
        // wandered before the frame noticed.
        if !hub_win::window::left_button_down() {
            self.resize.end(Some(hwnd), &self.log, self.id);
            return;
        }
        let ppp = ctx.pixels_per_point().max(0.1);
        let min = (
            (MIN_NOTE_WIDTH * ppp).round() as i32,
            (MIN_NOTE_HEIGHT * ppp).round() as i32,
        );
        self.resize.follow(hwnd, min);
        // No egui events drive this gesture -- the press belongs to the island --
        // so the next frame has to be asked for.
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_edges_answer_and_the_top_is_neither() {
        let grip = |x: f32, y: f32| grip_at(x, y, 400.0, 300.0, 5.0);
        assert_eq!(grip(2.0, 150.0), Grip { left: true, right: false, bottom: false });
        assert_eq!(grip(398.0, 150.0), Grip { left: false, right: true, bottom: false });
        assert_eq!(grip(200.0, 298.0), Grip { left: false, right: false, bottom: true });
        assert_eq!(
            grip(2.0, 298.0),
            Grip { left: true, right: false, bottom: true },
            "the bottom-left corner holds both edges"
        );
        // The middle of the body is text, not a grip.
        assert_eq!(grip(200.0, 150.0), Grip::default());
        // The top edge is the strip's: pressing 2 px down must mean "move".
        assert_eq!(grip(200.0, 2.0), Grip::default());
        // Just inside the band is the band; just outside is not.
        assert!(grip(5.0, 150.0).any());
        assert!(!grip(6.0, 150.0).any());
    }

    #[test]
    fn a_window_too_small_for_two_bands_has_no_grip() {
        // A 1 px client area exists for a frame during creation, and "both edges
        // at once" would make the two halves of `resized` cancel out.
        assert_eq!(grip_at(0.0, 0.0, 1.0, 1.0, 5.0), Grip::default());
        assert_eq!(grip_at(0.0, 0.0, 20.0, 300.0, 5.0), Grip::default());
        // 21 px is wide enough for one band on each side, so the left edge answers.
        assert_eq!(
            grip_at(0.0, 0.0, 21.0, 300.0, 5.0).left,
            true,
            "the guard must not swallow every narrow window"
        );
    }

    #[test]
    fn the_right_edge_moves_only_the_right_edge() {
        let start = (100, 100, 400, 300);
        let right = Grip { left: false, right: true, bottom: false };
        assert_eq!(resized(start, right, (40, 0), (150, 60)), (100, 100, 440, 300));
        // Pulling past the origin is legal: the window shrinks to the floor, and
        // the origin does not follow, because the left edge is not held.
        assert_eq!(resized(start, right, (-380, 0), (150, 60)), (100, 100, 150, 300));
    }

    #[test]
    fn the_left_edge_keeps_the_right_one_where_it_is() {
        let start = (100, 100, 400, 300);
        let left = Grip { left: true, right: false, bottom: false };
        // Left edge to the left: wider, origin follows, right edge fixed at 500.
        assert_eq!(resized(start, left, (-40, 0), (150, 60)), (60, 100, 440, 300));
        // Left edge to the right: narrower, right edge still at 500.
        let (x, _, w, _) = resized(start, left, (40, 0), (150, 60));
        assert_eq!((x, w), (140, 360));
        assert_eq!(x + w, 500);
        // Past the floor: the width stops at 150 and the right edge must not slide
        // with the cursor -- that is the jitter the clamp in `resized` prevents.
        let (x, _, w, _) = resized(start, left, (900, 0), (150, 60));
        assert_eq!((x, w), (350, 150));
        assert_eq!(x + w, 500);
    }

    #[test]
    fn the_bottom_edge_ignores_horizontal_travel_and_has_a_floor() {
        let start = (100, 100, 400, 300);
        let bottom = Grip { left: false, right: false, bottom: true };
        assert_eq!(resized(start, bottom, (0, 50), (150, 60)), (100, 100, 400, 350));
        assert_eq!(resized(start, bottom, (300, -500), (150, 60)), (100, 100, 400, 60));
    }

    #[test]
    fn a_corner_moves_both_axes_in_one_call() {
        let start = (100, 100, 400, 300);
        let corner = Grip { left: false, right: true, bottom: true };
        assert_eq!(
            resized(start, corner, (25, 15), (150, 60)),
            (100, 100, 425, 315)
        );
        let corner = Grip { left: true, right: false, bottom: true };
        let (x, _, w, h) = resized(start, corner, (-10, 10), (150, 60));
        assert_eq!((x, w, h), (90, 410, 310));
        assert_eq!(x + w, 500, "the right edge is still the anchor");
    }

    #[test]
    fn a_grip_that_holds_no_edge_changes_nothing() {
        let start = (100, 100, 400, 300);
        assert_eq!(resized(start, Grip::default(), (30, 30), (150, 60)), start);
    }
}
