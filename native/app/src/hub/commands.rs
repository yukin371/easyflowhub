//! The controller's command handlers: open the workbench, open a note, rebind a key.
//!
//! Split out of `hub.rs` when the file passed the guard's 800-line cap. The seam is
//! honest: `hub.rs` owns windows and the request queue, and these three are the only
//! handlers that change *state the controller owns* -- the table, the settings file
//! and the OS registrations.
//!
//! They stay `impl Hub` rather than becoming free functions because they touch the
//! same private fields the rest of the controller does; threading the log, store,
//! table and inbox through a free function would be four arguments that always
//! travel together anyway.

use std::time::Duration;

use hub_core::{Chord, Command, Scope, SpikeSettings};

use super::{register_hotkeys, Hub};
use crate::manager;

impl Hub {
    /// `Ctrl+Alt+M`: show the workbench, creating it the first time.
    pub(super) fn open_manager(&mut self, ctx: &egui::Context) {
        if self.manager.is_some() {
            ctx.send_viewport_cmd_to(manager::viewport_id(), egui::ViewportCommand::Focus);
            ctx.request_repaint_of(manager::viewport_id());
            self.log
                .mark(0, "note_manager", "action=focus target=manager");
            return;
        }
        self.manager = Some(manager::Shell::new(manager::Spawn {
            log: self.log.clone(),
            store: self.store.clone(),
            shortcuts: self.shortcuts.clone(),
            inbox: self.inbox.clone(),
        }));
        self.log
            .mark(0, "note_manager", "action=open target=manager");
        ctx.request_repaint();
    }

    /// Open a note the workbench picked, in a window of its own.
    pub(super) fn open_note(&mut self, ctx: &egui::Context, id: &str) {
        let row = self.store.as_ref().and_then(|store| {
            let guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.note(id).ok().flatten()
        });
        match row {
            Some(row) => {
                self.log
                    .mark(0, "note_open", &format!("id={id} source=manager"));
                self.spawn(ctx, Some(row));
            }
            None => self
                .log
                .mark(0, "note_open", &format!("id={id} source=manager found=false")),
        }
    }

    /// Apply a rebind: the table, the settings file, and the OS registrations.
    ///
    /// All three or none of them. The table alone would work until the next start;
    /// the file alone would work only from the next start; the registrations alone
    /// would fight a table that still holds the old key. The order matters too -- a
    /// rejected chord must not reach the file.
    pub(super) fn rebind(&mut self, ctx: &egui::Context, command: Command, key: &str) {
        let Ok(chord) = Chord::parse(key) else {
            self.log.mark(
                0,
                "shortcut_rebind",
                &format!("command={} key={key} rejected=parse", command.id()),
            );
            return;
        };
        {
            let mut table = self
                .shortcuts
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let next = table.rebind(command, chord);
            *table = next;
        }
        let mut settings = SpikeSettings::load(&self.settings_path).0;
        settings
            .shortcuts
            .insert(command.id().to_string(), chord.display());
        if let Err(error) = settings.save(&self.settings_path) {
            self.log.mark(
                0,
                "shortcut_rebind",
                &format!("saved=false error=\"{error}\""),
            );
        }
        if command.scope() == Scope::Global {
            // The old thread has to let go of the combination before the new one can
            // take it: `RegisterHotKey` is first-come, and re-registering
            // immediately produced 1409 while the shipped build was running. Sixty
            // milliseconds is the time the old loop needs to see `WM_QUIT` and
            // unregister.
            if let Some(old) = self._hotkeys.take() {
                old.shutdown();
                std::thread::sleep(Duration::from_millis(60));
            }
            let snapshot = self
                .shortcuts
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            self._hotkeys = register_hotkeys(self.inbox.clone(), ctx.clone(), &self.log, &snapshot);
        }
        self.log.mark(
            0,
            "shortcut_rebind",
            &format!(
                "command={} key={} scope={}",
                command.id(),
                chord.display(),
                if command.scope() == Scope::Global {
                    "global"
                } else {
                    "window"
                }
            ),
        );
    }
}
