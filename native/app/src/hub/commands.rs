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
use hub_ui::LauncherAction;
use hub_ui::manager::Tab;

use super::boot::register_hotkeys;
use super::{Hub, Request};
use crate::manager;
use crate::todocard;

/// The queue entry a global command posts.
///
/// Lives here with the handlers it dispatches to: every command that is not in
/// this match is either a launcher press (handled on the hotkey thread) or a
/// window-scoped key (handled inside the note), and both are deliberate omissions
/// rather than forgotten arms.
pub(super) fn request_for(command: Command) -> Option<Request> {
    match command {
        Command::NewNote => Some(Request::NewNote),
        Command::CloseAll => Some(Request::CloseAll),
        Command::ToggleVisible => Some(Request::ToggleVisible),
        Command::OpenManager => Some(Request::Manager),
        _ => None,
    }
}

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

    /// Open the desktop todo card, or focus the one already open.
    ///
    /// One card, for the same reason there is one workbench: two cards over the same
    /// list disagree the moment one of them ticks something.
    pub(super) fn open_todo_card(&mut self, ctx: &egui::Context) {
        if self.todo_card.is_some() {
            ctx.send_viewport_cmd_to(todocard::viewport_id(), egui::ViewportCommand::Focus);
            ctx.request_repaint_of(todocard::viewport_id());
            self.log.mark(0, "todo_card", "action=focus");
            return;
        }
        self.todo_card = Some(todocard::Shell::new(todocard::Spawn {
            log: self.log.clone(),
            store: self.store.clone(),
            inbox: self.inbox.clone(),
        }));
        self.log.mark(0, "todo_card", "action=open");
        ctx.request_repaint();
    }

    /// Open the workbench on a given section.
    ///
    /// The launcher's cards for 待办 / 回收站 / 设置 all land here. An already-open
    /// workbench is switched rather than duplicated -- two windows showing the same
    /// list would disagree the moment one of them deleted a row.
    pub(super) fn open_manager_tab(&mut self, ctx: &egui::Context, tab: Tab) {
        self.open_manager(ctx);
        if let Some(shell) = self.manager.as_mut() {
            shell.show_tab(tab);
        }
        ctx.send_viewport_cmd_to(manager::viewport_id(), egui::ViewportCommand::Focus);
        ctx.request_repaint_of(manager::viewport_id());
        self.log
            .mark(0, "note_manager", &format!("action=tab tab={}", tab.id()));
    }

    /// What the action panel asked for.
    ///
    /// The panel is hidden *before* the effect runs, not after: spawning a note
    /// moves the foreground to it, and a launcher that is still on screen at that
    /// point would sit on top of the window it just opened.
    pub(super) fn launcher_action(&mut self, ctx: &egui::Context, action: LauncherAction) {
        match action {
            LauncherAction::None => {}
            // Esc is a dismissal, not a hand-over: the keyboard goes back to the
            // application the panel was summoned over.
            LauncherAction::Close => self.launcher.dismiss("esc"),
            LauncherAction::NewNote => {
                self.hide_launcher("action:new_note");
                self.spawn(ctx, None);
            }
            LauncherAction::OpenManager(tab) => {
                self.hide_launcher("action:manager");
                self.open_manager_tab(ctx, tab);
            }
            LauncherAction::OpenNote(id) => {
                self.hide_launcher("action:open_note");
                self.open_note(ctx, &id, "launcher");
            }
            // The three translation actions stay inside the panel: the request
            // threads, the card and the captured selection all live in that shell,
            // and routing them through the controller would mean handing out four
            // fields it has no other use for.
            LauncherAction::Translate => self.launcher.translate(ctx),
            LauncherAction::ReplaceSelection => self.launcher.replace_selection(ctx),
            LauncherAction::CopyTranslation => self.launcher.copy_translation(),
        }
    }

    fn hide_launcher(&mut self, reason: &str) {
        self.launcher.hide(reason);
    }

    /// Open a note the workbench picked, in a window of its own.
    pub(super) fn open_note(&mut self, ctx: &egui::Context, id: &str, source: &str) {
        let row = self.store.as_ref().and_then(|store| {
            let guard = store.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.note(id).ok().flatten()
        });
        match row {
            Some(row) => {
                self.log
                    .mark(0, "note_open", &format!("id={id} source={source}"));
                self.spawn(ctx, Some(row));
            }
            None => self
                .log
                .mark(0, "note_open", &format!("id={id} source={source} found=false")),
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
            self._hotkeys = register_hotkeys(
                self.inbox.clone(),
                ctx.clone(),
                &self.log,
                &snapshot,
                self.launcher.handle(),
            );
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
