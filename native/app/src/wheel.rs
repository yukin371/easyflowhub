//! `Ctrl+wheel` opacity, and the throttle that makes one gesture one step
//! (parity Q28).
//!
//! ## Why this is not in [`crate::keys`]
//!
//! The shipped note listens for `wheel` on the window and calls `preventDefault`;
//! there is no keyboard event involved. On the native side the two surfaces deliver
//! it through two different channels:
//!
//! | Pointer over | Channel | Who would otherwise act on it |
//! |---|---|---|
//! | the text island | `WM_MOUSEWHEEL`, `MK_CONTROL` set | RichEdit **zooms the text** — `Ctrl+wheel` is its zoom gesture |
//! | the title strip | winit → `egui::Event::MouseWheel` | egui **zooms the whole UI** (`Options::zoom_with_keyboard`) |
//!
//! Both defaults are wrong here and both are turned off: the control's by
//! intercepting the message in its subclass proc, egui's by clearing
//! `zoom_with_keyboard` once on the shared `Context`. What is left is this module:
//! turn a delta into a direction, and let the shell step the opacity.
//!
//! ## Why a 50 ms floor
//!
//! A notched wheel sends one 120-unit message per notch, and the window under the
//! pointer decides how many messages arrive — a precision wheel or a trackpad sends
//! a burst of small deltas for one flick. Without the floor one flick crosses 0..100
//! in a single gesture, which Q28 rules out ("a fast continuous scroll does not skip
//! steps"). 50 ms is the shipped constant, not a value chosen here.

use std::sync::atomic::{AtomicI8, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Two notches closer together than this are one notch.
pub const STEP_INTERVAL: Duration = Duration::from_millis(50);

/// Which way the user turned the wheel.
///
/// `Up` means "away from me", which is what Windows reports as a positive
/// `WM_MOUSEWHEEL` delta and egui as a positive `raw_scroll_delta.y`. In the
/// shipped note that direction *raises* the opacity (`deltaY < 0 → +0.2`), even
/// though "scroll up" reads like "less of everything" — copying it exactly is the
/// point of parity, and inventing the opposite would make this the only note app
/// where the gesture is inverted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notch {
    Up,
    Down,
}

impl Notch {
    fn code(self) -> i8 {
        match self {
            Notch::Up => 1,
            Notch::Down => -1,
        }
    }

    fn from_code(code: i8) -> Option<Self> {
        match code {
            1 => Some(Notch::Up),
            -1 => Some(Notch::Down),
            _ => None,
        }
    }
}

/// Which surface the notch came from.
///
/// Carried into the log because the two channels are the fragile part of this
/// feature: "the wheel did nothing" and "the wheel went to the other window" look
/// identical without it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `WM_MOUSEWHEEL` at the RichEdit island's subclass proc.
    Island,
    /// `egui::Event::MouseWheel`, i.e. the pointer was over the title strip.
    Egui,
}

/// The 50 ms floor, as a state machine with an injected clock.
///
/// Injected rather than read from `Instant::now()` so the rule is testable; the
/// shell feeds it egui's frame time. A swallowed notch deliberately does **not**
/// extend the window: the shipped handler returns before touching `lastWheelTime`,
/// so a continuous flick is sampled at 20 Hz rather than sliding 50 ms at a time.
#[derive(Debug, Default)]
pub struct Gate {
    last_ms: Option<u64>,
}

impl Gate {
    /// `true` when this notch may step.
    pub fn allow(&mut self, now_ms: u64) -> bool {
        if let Some(last) = self.last_ms {
            if now_ms.saturating_sub(last) < STEP_INTERVAL.as_millis() as u64 {
                return false;
            }
        }
        self.last_ms = Some(now_ms);
        true
    }
}

/// Where a notch from the island waits until the next frame.
///
/// Same shape as [`crate::keys::Bus`] for the same reason: the subclass proc runs
/// inside a message handler, so it records the direction and wakes *this* viewport.
#[derive(Clone)]
pub struct Wheel {
    slot: Arc<AtomicI8>,
    viewport: egui::ViewportId,
}

impl Wheel {
    pub fn new(viewport: egui::ViewportId) -> Self {
        Self {
            slot: Arc::new(AtomicI8::new(0)),
            viewport,
        }
    }

    /// Callback for `Editor::on_wheel`.
    ///
    /// Returns `true` for any non-zero delta, which is what stops RichEdit from
    /// zooming the body on the same gesture. A zero delta is passed through: it is
    /// not an opacity event, and swallowing it would hide a real scroll.
    pub fn handler(&self, ctx: egui::Context) -> Box<dyn Fn(i32) -> bool + Send + Sync> {
        let slot = self.slot.clone();
        let viewport = self.viewport;
        Box::new(move |delta: i32| {
            let Some(notch) = notch_from_wheel_delta(delta) else {
                return false;
            };
            slot.store(notch.code(), Ordering::SeqCst);
            ctx.request_repaint_of(viewport);
            true
        })
    }

    pub fn take(&self) -> Option<Notch> {
        Notch::from_code(self.slot.swap(0, Ordering::SeqCst))
    }
}

/// `WM_MOUSEWHEEL` HIWORD → direction.
pub fn notch_from_wheel_delta(delta: i32) -> Option<Notch> {
    match delta.cmp(&0) {
        std::cmp::Ordering::Greater => Some(Notch::Up),
        std::cmp::Ordering::Less => Some(Notch::Down),
        std::cmp::Ordering::Equal => None,
    }
}

/// The same decision for the egui path, which only the title strip produces.
///
/// `raw_scroll_delta`, not `smooth_scroll_delta`: smoothing spreads one notch over
/// several frames, and with the 50 ms floor that would step twice for one notch.
/// The modifier is read from the same `InputState` because winit reports it with the
/// event, not from `GetKeyState`.
pub fn egui_notch(ctx: &egui::Context) -> Option<Notch> {
    ctx.input(|input| {
        if !input.modifiers.ctrl {
            return None;
        }
        let y = input.raw_scroll_delta.y;
        if y == 0.0 {
            None
        } else if y > 0.0 {
            Some(Notch::Up)
        } else {
            Some(Notch::Down)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_notch_per_fifty_milliseconds() {
        let mut gate = Gate::default();
        assert!(gate.allow(0), "the first notch is never throttled");
        assert!(!gate.allow(10));
        assert!(!gate.allow(49));
        assert!(gate.allow(50), "exactly 50 ms is allowed");
        // A swallowed notch must not extend the window, or a fast flick would be
        // sampled at a rate that depends on how fast the wheel spins.
        assert!(gate.allow(100));
        assert!(!gate.allow(101));
        assert!(gate.allow(150));
    }

    #[test]
    fn wheel_delta_maps_up_to_more_opacity() {
        // 120 is one WHEEL_DELTA; the sign is all that matters.
        assert_eq!(notch_from_wheel_delta(120), Some(Notch::Up));
        assert_eq!(notch_from_wheel_delta(-120), Some(Notch::Down));
        assert_eq!(notch_from_wheel_delta(0), None);
    }

    #[test]
    fn the_handler_consumes_only_real_notches() {
        let wheel = Wheel::new(egui::ViewportId::ROOT);
        let ctx = egui::Context::default();
        let handler = wheel.handler(ctx);

        assert!(handler(120), "Ctrl+wheel over the island is ours");
        assert_eq!(wheel.take(), Some(Notch::Up));
        assert_eq!(wheel.take(), None, "taking is destructive");

        assert!(!handler(0), "a zero delta is not an opacity event");
    }

    #[test]
    fn a_whole_flick_of_deltas_becomes_one_step_per_window() {
        // What the shell actually does: gate first, then step. Ten small deltas
        // inside one 50 ms window must produce exactly one step.
        let mut gate = Gate::default();
        let mut opacity = 1.0;
        let mut steps = 0;
        for tick in 0..10 {
            let notch = notch_from_wheel_delta(-15).unwrap();
            if gate.allow(tick * 5) {
                opacity = hub_ui::quicknote::step_opacity(opacity, notch == Notch::Up);
                steps += 1;
            }
        }
        assert_eq!(steps, 1, "one flick, one step");
        assert_eq!(opacity, 0.8);
    }
}
