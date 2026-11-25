//! The quick-note window's shortcut table.
//!
//! Ported from `useQuickNoteShortcuts.ts`, **in the shipped order**, because the
//! order is the specification: `Ctrl+Shift+Z` is redo only because the redo check
//! comes before the close check, and `Ctrl+Shift+D` closes the window only because
//! the shipped close check ignores Shift. Changing the order changes behaviour
//! even though every entry looks independent.
//!
//! Two input paths reach the same table. While the island has focus the keystroke
//! arrives at its subclass proc and never reaches winit; after a click on the
//! title strip the focus is on the egui window instead. Neither path sees the
//! other's keys, so a table entry needs no de-duplication.
//!
//! What is deliberately *not* here: the switcher (`Ctrl+K`) and ghost mode
//! (`Ctrl+Shift+H`). The switcher needs the notes list (Phase 2); ghost mode needs
//! a layered window (batch 1c). Both would be shortcuts that open nothing.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

pub use hub_editor::KeyPress;

/// `VK_*` values the table compares against. Kept here rather than reaching into
/// `hub-editor::keys` for the escape code, so a change to one crate's vocabulary
/// cannot silently renumber a shortcut.
const VK_ESCAPE: u16 = 0x1B;
const VK_D: u16 = 0x44;
const VK_T: u16 = 0x54;
const VK_Z: u16 = 0x5A;
const VK_OEM_PERIOD: u16 = 0xBE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Close,
    TogglePin,
    ToggleTools,
    Undo,
    Redo,
}

impl Action {
    fn code(self) -> u8 {
        match self {
            Action::Close => 1,
            Action::TogglePin => 2,
            Action::ToggleTools => 3,
            Action::Undo => 4,
            Action::Redo => 5,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Action::Close),
            2 => Some(Action::TogglePin),
            3 => Some(Action::ToggleTools),
            4 => Some(Action::Undo),
            5 => Some(Action::Redo),
            _ => None,
        }
    }
}

/// Map a key press to an action, or `None` to let the control have the key.
///
/// `None` is the important answer for `Ctrl+Z`: RichEdit already owns undo, and
/// replacing it to make the table symmetrical would trade the platform's caret and
/// selection handling for nothing.
pub fn resolve(press: KeyPress) -> Option<Action> {
    if press.vk == VK_ESCAPE {
        // The shipped handler does not look at modifiers for Escape, so Ctrl+Esc
        // closes too. Kept as-is: Esc is the "get me out" key.
        return Some(Action::Close);
    }

    // Every remaining entry is Ctrl-based. Alt is excluded because the shipped
    // matcher requires the alt flag to agree, and no entry declares it.
    if !press.ctrl || press.alt {
        return None;
    }

    match (press.vk, press.shift) {
        (VK_Z, false) => Some(Action::Undo),
        (VK_Z, true) => Some(Action::Redo),
        (VK_T, false) => Some(Action::TogglePin),
        // Shift is ignored on purpose -- see the module comment. `Ctrl+Shift+D`
        // would otherwise be a line-duplicate shortcut, which this shell does not
        // implement; until it does, matching the shipped close check exactly is the
        // honest choice.
        (VK_D, _) => Some(Action::Close),
        // Shift+`.` produces `>` in `event.key`, so the shipped check misses it.
        (VK_OEM_PERIOD, false) => Some(Action::ToggleTools),
        _ => None,
    }
}

/// The keys the table answers, as egui sees them.
///
/// Only used on the egui path; the island path already carries a virtual-key code.
/// The event is matched rather than `InputState::key_pressed` so that an
/// auto-repeat while the key is held cannot flip the pin ten times a second.
pub fn egui_press(ctx: &egui::Context) -> Option<KeyPress> {
    ctx.input(|input| {
        for event in input.events.iter().rev() {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            let vk = match key {
                egui::Key::Escape => VK_ESCAPE,
                egui::Key::Z => VK_Z,
                egui::Key::T => VK_T,
                egui::Key::D => VK_D,
                egui::Key::Period => VK_OEM_PERIOD,
                _ => continue,
            };
            return Some(KeyPress {
                vk,
                ctrl: modifiers.ctrl,
                shift: modifiers.shift,
                alt: modifiers.alt,
            });
        }
        None
    })
}

/// Where a pressed shortcut waits until the next `update`.
///
/// The subclass proc runs inside a message handler, so acting there would re-enter
/// window code. A single slot is enough: two shortcuts inside one frame would both
/// be served by the next frame anyway, and dropping the first is better than
/// queueing a close behind a pin and then running both.
#[derive(Clone)]
pub struct Bus {
    slot: Arc<AtomicU8>,
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            slot: Arc::new(AtomicU8::new(0)),
        }
    }
}

impl Bus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a shortcut and wake the UI thread.
    pub fn post(&self, action: Action, ctx: &egui::Context) {
        self.slot.store(action.code(), Ordering::SeqCst);
        ctx.request_repaint();
    }

    /// Take the pending action, if any.
    pub fn take(&self) -> Option<Action> {
        Action::from_code(self.slot.swap(0, Ordering::SeqCst))
    }

    /// Build the callback handed to `Editor::on_key`.
    ///
    /// `true` means "consumed": the key must not reach `DefSubclassProc`, or
    /// `Ctrl+D` would beep inside the control as well as closing the window.
    pub fn handler(&self, ctx: egui::Context) -> Box<dyn Fn(KeyPress) -> bool + Send + Sync> {
        let slot = self.slot.clone();
        Box::new(move |press| match resolve(press) {
            Some(action) => {
                slot.store(action.code(), Ordering::SeqCst);
                ctx.request_repaint();
                true
            }
            None => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(vk: u16) -> KeyPress {
        KeyPress {
            vk,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    fn ctrl(vk: u16) -> KeyPress {
        KeyPress {
            vk,
            ctrl: true,
            shift: false,
            alt: false,
        }
    }

    #[test]
    fn escape_closes_even_with_modifiers() {
        assert_eq!(resolve(press(VK_ESCAPE)), Some(Action::Close));
        let mut shift_esc = press(VK_ESCAPE);
        shift_esc.ctrl = true;
        shift_esc.shift = true;
        assert_eq!(
            resolve(shift_esc),
            Some(Action::Close),
            "the shipped handler checks `event.key === 'Escape'` without reading modifiers"
        );
    }

    #[test]
    fn ctrl_z_is_undo_and_shift_switches_it_to_redo() {
        assert_eq!(resolve(ctrl(VK_Z)), Some(Action::Undo));
        let mut redo = ctrl(VK_Z);
        redo.shift = true;
        assert_eq!(resolve(redo), Some(Action::Redo));
    }

    #[test]
    fn ctrl_t_toggles_pin_only_without_shift() {
        assert_eq!(resolve(ctrl(VK_T)), Some(Action::TogglePin));
        let mut shifted = ctrl(VK_T);
        shifted.shift = true;
        assert_eq!(
            resolve(shifted),
            None,
            "`matchesShortcutEvent` requires the shift flag to agree with 'Ctrl+T'"
        );
    }

    #[test]
    fn ctrl_d_closes_and_ignores_shift_like_the_shipped_check() {
        assert_eq!(resolve(ctrl(VK_D)), Some(Action::Close));
        let mut shifted = ctrl(VK_D);
        shifted.shift = true;
        assert_eq!(
            resolve(shifted),
            Some(Action::Close),
            "the shipped close check is `commandKey && key === 'd'`; Shift+D lowercases to 'd'"
        );
    }

    #[test]
    fn ctrl_period_toggles_tools_but_shift_period_does_not() {
        assert_eq!(resolve(ctrl(VK_OEM_PERIOD)), Some(Action::ToggleTools));
        let mut shifted = ctrl(VK_OEM_PERIOD);
        shifted.shift = true;
        assert_eq!(
            resolve(shifted),
            None,
            "Shift+. reports '>' as `event.key`, so the shipped check misses it"
        );
    }

    #[test]
    fn plain_letters_and_alt_chords_are_left_to_the_control() {
        assert_eq!(resolve(press(VK_D)), None, "typing 'd' must not close");
        assert_eq!(resolve(press(VK_Z)), None);
        assert_eq!(resolve(press(VK_OEM_PERIOD)), None, "a typed '.' is a dot");

        let mut alt_d = ctrl(VK_D);
        alt_d.alt = true;
        assert_eq!(resolve(alt_d), None, "Ctrl+Alt+D belongs to the global hotkeys");
    }

    #[test]
    fn the_bus_keeps_only_the_latest_shortcut() {
        let bus = Bus::new();
        let ctx = egui::Context::default();
        assert_eq!(bus.take(), None, "an empty bus must not invent an action");

        bus.post(Action::TogglePin, &ctx);
        bus.post(Action::Close, &ctx);
        assert_eq!(
            bus.take(),
            Some(Action::Close),
            "the later shortcut wins; a stale pin must not run after the window closed"
        );
        assert_eq!(bus.take(), None, "taking is destructive");
    }

    #[test]
    fn the_handler_consumes_only_what_the_table_answers() {
        let bus = Bus::new();
        let ctx = egui::Context::default();
        let handler = bus.handler(ctx);

        assert!(handler(ctrl(VK_D)), "Ctrl+D is ours");
        assert_eq!(bus.take(), Some(Action::Close));

        assert!(
            !handler(press(VK_D)),
            "a typed 'd' must fall through to RichEdit, not be swallowed"
        );
        assert_eq!(bus.take(), None);
    }
}
