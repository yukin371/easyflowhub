//! The quick-note window's keyboard path: a keystroke in, a command out.
//!
//! The *table* moved to [`hub_core::shortcuts`] when shortcuts became
//! configurable. What stays here is the part that belongs to a window: two input
//! paths that must not double-fire, and the slot a pressed command waits in until
//! the next frame.
//!
//! Two input paths reach the same table. While the island has focus the keystroke
//! arrives at its subclass proc and never reaches the window's own event handler;
//! after a click on the title strip the focus is on the frame instead. Neither
//! path sees the other's keys, so a table entry needs no de-duplication.
//!
//! What a table entry *does* need is the modifier flags compared exactly. The
//! shipped matcher requires them to agree, which is why `Ctrl+Alt+H` (minimize
//! every note, a global key) is not also a window shortcut and why a typed `d`
//! is a character rather than a close.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

pub use hub_core::{Chord, Command, Scope, Table};
pub use hub_editor::KeyPress;

/// Where a pressed shortcut waits until the next frame.
///
/// The subclass proc runs inside a message handler, so acting there would re-enter
/// window code. A single slot is enough: two shortcuts inside one frame would both
/// be served by the next frame anyway, and dropping the first is better than
/// queueing a close behind a pin and then running both.
///
/// The bus carries its own `ViewportId` because a repaint request without one means
/// "whoever ran last": with several notes open, that is a different window than the
/// one the key was typed into, and the action waits for the note's 500 ms tick.
#[derive(Clone)]
pub struct Bus {
    slot: Arc<AtomicU8>,
    viewport: egui::ViewportId,
}

impl Bus {
    pub fn new(viewport: egui::ViewportId) -> Self {
        Self {
            slot: Arc::new(AtomicU8::new(0)),
            viewport,
        }
    }

    /// Record a shortcut and wake this note's viewport.
    pub fn post(&self, command: Command, ctx: &egui::Context) {
        self.slot.store(command.code(), Ordering::SeqCst);
        ctx.request_repaint_of(self.viewport);
    }

    /// Take the pending command, if any.
    pub fn take(&self) -> Option<Command> {
        Command::from_code(self.slot.swap(0, Ordering::SeqCst))
    }

    /// Build the callback handed to `Editor::on_key`.
    ///
    /// `true` means "consumed": the key must not reach the default handler, or
    /// `Ctrl+D` would beep inside the control as well as closing the window.
    /// `table` is behind a lock rather than cloned per window: rebinding a key in
    /// the workbench has to reach the windows that are already open, and a copy
    /// taken at spawn time would leave every existing window on the old key --
    /// exactly the kind of "I changed it and nothing happened" this feature exists
    /// to avoid.
    pub fn handler(
        &self,
        ctx: egui::Context,
        table: Arc<RwLock<Table>>,
    ) -> Box<dyn Fn(KeyPress) -> bool + Send + Sync> {
        let slot = self.slot.clone();
        let viewport = self.viewport;
        Box::new(move |press| {
            let guard = table.read().unwrap_or_else(|poisoned| poisoned.into_inner());
            match resolve(&guard, press) {
                Some(command) => {
                    slot.store(command.code(), Ordering::SeqCst);
                    ctx.request_repaint_of(viewport);
                    true
                }
                None => false,
            }
        })
    }
}

/// Match a pressed key against the window-scope commands.
///
/// `None` is the important answer for `Ctrl+Z` on a layout where the control wants
/// it, and for every character the user is actually typing.
pub fn resolve(table: &Table, press: KeyPress) -> Option<Command> {
    table.find(
        Scope::Window,
        Chord::new(press.ctrl, press.alt, press.shift, press.vk),
    )
}

/// The most recent key press this frame, in the shape the table compares against.
///
/// The conversion lives in the widget crate because the settings screen captures
/// bindings with the same mapping; doing it twice is how one path ends up with a
/// different code for `1` than the other.
pub fn egui_press(ctx: &egui::Context) -> Option<KeyPress> {
    hub_ui::keys::chord_from_egui(ctx).map(|chord| KeyPress {
        vk: chord.vk,
        ctrl: chord.ctrl,
        shift: chord.shift,
        alt: chord.alt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(vk: u16, ctrl: bool, shift: bool, alt: bool) -> KeyPress {
        KeyPress {
            vk,
            ctrl,
            shift,
            alt,
        }
    }

    fn table() -> Table {
        Table::defaults()
    }

    #[test]
    fn escape_closes_whatever_the_modifiers() {
        let table = table();
        assert_eq!(
            resolve(&table, press(0x1B, false, false, false)),
            Some(Command::CloseNote)
        );
        assert_eq!(
            resolve(&table, press(0x1B, true, true, false)),
            Some(Command::CloseNote),
            "the shipped handler never reads the modifier flags for Escape"
        );
    }

    #[test]
    fn ctrl_z_is_undo_and_shift_switches_it_to_redo() {
        let table = table();
        assert_eq!(resolve(&table, press(0x5A, true, false, false)), Some(Command::Undo));
        assert_eq!(
            resolve(&table, press(0x5A, true, true, false)),
            Some(Command::Redo)
        );
    }

    #[test]
    fn ctrl_t_toggles_pin_only_without_shift() {
        let table = table();
        assert_eq!(
            resolve(&table, press(0x54, true, false, false)),
            Some(Command::TogglePin)
        );
        assert_eq!(
            resolve(&table, press(0x54, true, true, false)),
            None,
            "the matcher requires the shift flag to agree"
        );
    }

    #[test]
    fn ctrl_d_closes_and_ignores_shift_like_the_shipped_check() {
        let table = table();
        assert_eq!(
            resolve(&table, press(0x44, true, false, false)),
            Some(Command::CloseNote)
        );
        assert_eq!(
            resolve(&table, press(0x44, true, true, false)),
            None,
            "the table compares modifiers exactly; Shift+Ctrl+D is not Ctrl+D"
        );
    }

    #[test]
    fn a_rebound_command_moves_on_both_paths() {
        // The island and the frame both go through `resolve`, so a rebind is one
        // edit. The old chord must stop answering even though no code changed.
        let mut overrides = std::collections::BTreeMap::new();
        overrides.insert("toggle_pin".to_string(), "Ctrl+Shift+P".to_string());
        let table = Table::build(&overrides);

        assert_eq!(
            resolve(&table, press(0x50, true, true, false)),
            Some(Command::TogglePin)
        );
        assert_eq!(resolve(&table, press(0x54, true, false, false)), None);
    }

    #[test]
    fn plain_letters_and_global_chords_are_left_to_the_control() {
        let table = table();
        assert_eq!(resolve(&table, press(0x44, false, false, false)), None, "typing 'd'");
        assert_eq!(resolve(&table, press(0x5A, false, false, false)), None);
        assert_eq!(
            resolve(&table, press(0x4E, true, false, true)),
            None,
            "Ctrl+Alt+N is a global hotkey, not a window shortcut"
        );
    }

    #[test]
    fn the_bus_keeps_only_the_latest_shortcut() {
        let bus = Bus::new(egui::ViewportId::ROOT);
        let ctx = egui::Context::default();
        assert_eq!(bus.take(), None, "an empty bus must not invent an action");

        bus.post(Command::TogglePin, &ctx);
        bus.post(Command::CloseNote, &ctx);
        assert_eq!(
            bus.take(),
            Some(Command::CloseNote),
            "the later shortcut wins; a stale pin must not run after the window closed"
        );
        assert_eq!(bus.take(), None, "taking is destructive");
    }

    #[test]
    fn the_handler_consumes_only_what_the_table_answers() {
        let bus = Bus::new(egui::ViewportId::ROOT);
        let ctx = egui::Context::default();
        let handler = bus.handler(
            ctx,
            std::sync::Arc::new(std::sync::RwLock::new(Table::defaults())),
        );

        assert!(handler(press(0x44, true, false, false)), "Ctrl+D is ours");
        assert_eq!(bus.take(), Some(Command::CloseNote));

        assert!(
            !handler(press(0x44, false, false, false)),
            "a typed 'd' must fall through to the control, not be swallowed"
        );
        assert_eq!(bus.take(), None);
    }
}
