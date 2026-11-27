//! Everything the controller does **once**: open the store, resolve the key table,
//! register it with the OS, install the tray.
//!
//! Split out of `hub.rs` for one reason worth naming: each of these four can fail
//! in a way that is neither fatal nor visible, and each failure is answered by a
//! log line rather than by a returned error. Keeping them together makes "what
//! does a half-broken startup look like" one file to read instead of a search
//! through the loop.
//!
//! None of this runs per frame. The controller owns the results; this module only
//! produces them.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hub_core::{Command, Store, Table};
use hub_win::hotkey::{Binding, HotkeySet, PressHandler};
use hub_win::tray::{Tray, TrayCommand};

use crate::launcher;
use crate::log::Log;

use super::commands;
use super::{Config, Request};

/// The app icon, embedded so the tray needs no file next to the executable.
///
/// This is the `.ico` the Tauri build shipped, moved here because the native build
/// is now the thing that has to draw it. `hub_win::tray` parses the frame it wants;
/// an unreadable asset falls back to the system icon and says so in the log.
const APP_ICON: &[u8] = include_bytes!("../../assets/icon.ico");

/// The tray's hover text.
///
/// It deliberately does **not** name a combination. The tooltip is set once, when
/// the icon is installed, so any key printed here would be the key in force at
/// startup -- and after a rebind it would be a lie the user cannot fix without a
/// restart. The workbench shows the live table.
const TRAY_TOOLTIP: &str = "EasyFlowHub · 快速笔记";

/// Resolve the shortcut table from the settings file and say what it resolved to.
///
/// Logged line by line because this is the only place the effective binding of
/// anything is knowable: the tooltips show it too, but a support question ("my
/// key does nothing") needs the table that was actually in force, including the
/// overrides that were rejected and the conflicts.
pub fn build_shortcuts(config: &Config) -> Table {
    let table = Table::build(&config.settings.shortcuts);
    for (command, chord) in table.globals() {
        config.log.mark(
            0,
            "shortcut",
            &format!("scope=global command={} key={}", command.id(), chord.display()),
        );
    }
    for (command, chord) in table.window() {
        config.log.mark(
            0,
            "shortcut",
            &format!("scope=window command={} key={}", command.id(), chord.display()),
        );
    }
    for note in table.notes() {
        config.log.mark(0, "shortcut_rejected", note);
    }
    table
}

/// Open `notes.db` once for the process.
///
/// A failure is not fatal: the note windows are the product, and a shell that refused
/// to open because the database was locked or corrupt would take the notes down with
/// it. The log line is what makes "my notes are not being saved" answerable -- without
/// it, a silent in-memory note looks exactly like a broken save.
pub fn open_store(config: &Config) -> Option<Arc<Mutex<Store>>> {
    let opened = match &config.store_root {
        Some(root) => Store::open_in(root),
        None => Store::open_default(),
    };
    match opened {
        Ok(store) => {
            config.log.mark(
                0,
                "store",
                &format!(
                    "open=true path={} sqlite={} notes={:?}",
                    store.path().display(),
                    store.sqlite_version(),
                    store.counts().ok().and_then(|counts| counts.notes)
                ),
            );
            Some(Arc::new(Mutex::new(store)))
        }
        Err(error) => {
            config.log.mark(
                0,
                "store",
                &format!("open=false error=\"{error}\" notes=memory-only"),
            );
            None
        }
    }
}

/// One closure per combination: it appends the request and wakes ROOT.
///
/// `request_repaint_of(ROOT)` rather than `request_repaint()`: outside a pass the
/// context's "current viewport" is whatever ran last, and waking a note would leave
/// the request sitting in the queue until that note's own 500 ms tick.
fn press_handler(inbox: Arc<Mutex<Vec<Request>>>, ctx: egui::Context, request: Request) -> PressHandler {
    // A press handler is an `Fn`, so it runs many times and cannot move its
    // payload out. `Request` gained a `String` variant in 2c (opening a note by
    // id), which is what ended the `Copy` it used to have.
    let request = Arc::new(request);
    Arc::new(move || {
        inbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push((*request).clone());
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    })
}

pub fn register_hotkeys(
    inbox: Arc<Mutex<Vec<Request>>>,
    ctx: egui::Context,
    log: &Log,
    table: &Table,
    launcher: Arc<launcher::Handle>,
) -> Option<HotkeySet> {
    let mut bindings: Vec<Binding> = Vec::new();
    for (command, chord) in table.globals() {
        // The launcher is not a queue entry: its handler moves the window on the
        // hotkey thread (see `launcher::binding`).
        if command == Command::Launcher {
            bindings.push(launcher::binding(&chord, launcher.clone()));
            continue;
        }
        let Some(request) = commands::request_for(command) else {
            log.mark(
                0,
                "hotkey_skipped",
                &format!("command={} reason=no_request", command.id()),
            );
            continue;
        };
        if table.shadowed().contains(&command) {
            // Two commands on one combination cannot both be registered; the
            // table already decided which one owns the key, so registering the
            // loser would only produce a second `RegisterHotKey` failure.
            log.mark(
                0,
                "hotkey_skipped",
                &format!("command={} key={} reason=conflict", command.id(), chord.display()),
            );
            continue;
        }
        bindings.push(Binding::new(
            chord.display(),
            press_handler(inbox.clone(), ctx.clone(), request),
        ));
    }

    match HotkeySet::spawn(bindings, super::HOTKEY_DEBOUNCE) {
        Ok(set) => {
            for combo in set.combos() {
                log.mark(0, "hotkey", &format!("combo={combo} registered=true"));
            }
            // Not fatal, one line each: another program owning one combination is the
            // user's problem to see, not a reason to refuse the other three.
            for failure in set.failures() {
                log.mark(0, "hotkey", &format!("registered=false {failure}"));
            }
            log.mark(
                0,
                "hotkeys",
                &format!(
                    "registered={} failed={} thread={}",
                    set.combos().len(),
                    set.failures().len(),
                    set.thread_id()
                ),
            );
            Some(set)
        }
        Err(error) => {
            log.mark(0, "hotkeys", &format!("registered=0 {error}"));
            None
        }
    }
}

/// Wait for forwarded launches and answer them.
///
/// The first launch owns the shell; every later one rings a doorbell and exits
/// (`hub_win::instance`). Which bell is rung decides the answer, and there are two:
///
/// * **reveal** — a plain launch or `--resident`. `reveal`, not `toggle`, because a
///   second launch asks for the panel rather than for a change to its state.
/// * **note** — `--note`, and the handover from the Manager's "新建快速笔记". The id,
///   when there is one, comes through `pending` because an event cannot carry it.
///
/// Both threads are detached and never joined: they live exactly as long as the
/// process, and their `Arc<Guard>` is what keeps the mutex held, so there is nothing
/// to join *to*. They start after the tray because all three mean "the shell is now
/// reachable", and a forwarded launch that arrives before this point is still waiting
/// on the auto-reset event rather than lost.
pub fn watch_instance(
    guard: Arc<hub_win::instance::Guard>,
    launcher: Arc<launcher::Handle>,
    inbox: Arc<Mutex<Vec<Request>>>,
    ctx: egui::Context,
    settings_path: PathBuf,
    log: &Arc<Log>,
) {
    let reveal_log = Arc::clone(log);
    let note_log = Arc::clone(log);
    let _ = guard.clone().watch(move || {
        // The only trace a forwarded launch leaves: the process that sent it exited
        // without touching the log (it must not), so this line is where "the second
        // double-click did something" becomes visible. `waited_ms` is the whole story
        // of a login race -- a forward that arrives before the panel has a window is
        // queued, not dropped.
        match launcher.reveal_when_ready() {
            Some(waited) => reveal_log.mark(
                0,
                "instance",
                &format!(
                    "action=reveal reason=second-launch waited_ms={:.1}",
                    waited.as_secs_f64() * 1000.0
                ),
            ),
            None => reveal_log.mark(0, "instance", "action=dropped reason=no_window"),
        }
    });

    let _ = guard.watch_note(move || {
        // Reading the request is also what deletes it, so this branch runs at most
        // once per ring -- the auto-reset event already collapses simultaneous
        // launches into one answer.
        let request = match crate::pending::take(&settings_path) {
            Some(id) => {
                note_log.mark(0, "instance", &format!("action=open-note id={id}"));
                Request::OpenNote(id)
            }
            None => {
                note_log.mark(0, "instance", "action=new-note");
                Request::NewNote
            }
        };
        inbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);
        // The ring arrived on a watcher thread while the event loop was parked: this
        // is what turns the queue entry into a window now rather than at the next
        // keystroke the user happens to press.
        ctx.request_repaint();
    });
}

pub fn install_tray(
    inbox: Arc<Mutex<Vec<Request>>>,
    ctx: egui::Context,
    log: &Log,
) -> Option<Tray> {
    let sink = Arc::new(move |command: TrayCommand| {
        let request = match command {
            TrayCommand::NewNote => Request::NewNote,
            TrayCommand::CloseAll => Request::CloseAll,
            TrayCommand::Quit => Request::Quit,
        };
        inbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    });

    match Tray::install(TRAY_TOOLTIP, Some(APP_ICON), sink) {
        Ok(tray) => {
            log.mark(
                0,
                "tray",
                &format!(
                    "installed=true hwnd=0x{:X} icon={} tip={TRAY_TOOLTIP}",
                    tray.hwnd(),
                    tray.icon_source()
                ),
            );
            Some(tray)
        }
        Err(error) => {
            log.mark(0, "tray", &format!("installed=false {error}"));
            None
        }
    }
}
