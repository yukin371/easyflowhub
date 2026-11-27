//! The process root: a parked controller window that owns the note viewports.
//!
//! ## Why the root window is not a note
//!
//! eframe binds the event loop to the root viewport: closing it ends the process.
//! If the first note *were* the root, closing that note would take every other
//! note down with it -- the opposite of parity Q5. So the root is a 1×1 window
//! parked off every monitor, and every note is a deferred viewport of its own.
//!
//! The root stays **visible while parked** (`hub_win::window::park`, the same
//! trick the spike used): hiding it stops `WM_PAINT` delivery, and a root that
//! cannot paint cannot declare viewports, so the notes would freeze rather than
//! close.
//!
//! ## What the controller owns
//!
//! Only the things that outlive one note: the log, the settings, the spawn
//! counter, the list of live shells, the tray icon and the global shortcuts. It
//! never renders a note and never touches a RichEdit; a note's frame runs in that
//! viewport's own callback, which keeps the "one window, one island" pairing
//! visible in the code.
//!
//! ## Two entrances, one queue
//!
//! The hotkey thread and the tray's message window both run off the event-loop
//! thread (a hotkey thread of its own, and the tray's window proc, which the loop
//! dispatches between passes). Neither may create a window or run egui code, so
//! both push a [`Request`] into one queue and ask ROOT for a repaint. One queue also
//! means one ordering: a tray click and a keystroke cannot interleave halfway through
//! a spawn.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use hub_core::{
    place_note, Command, NoteRow, NoteSpec, Rect, SpikeSettings, Store, Table,
};
use hub_ui::LauncherAction;
use hub_win::hotkey::HotkeySet;
use hub_win::tray::Tray;
use hub_win::MonitorCache;

use crate::launcher;
use crate::log::Log;
use crate::manager;
use crate::note::{self, NoteShell, Shared};

mod boot;
mod commands;

/// Title of the controller window. Never shown, never activated.
pub const HUB_TITLE: &str = "easyflowhub-hub";

/// Same debounce the spike uses: QQ/WeChat hook the same combinations, so one
/// physical press can arrive as two or three `WM_HOTKEY` messages.
const HOTKEY_DEBOUNCE: Duration = Duration::from_millis(250);

/// Work that has to be executed on the event-loop thread.
///
/// The hotkey thread cannot create a window (win32 windows are thread-affine and
/// egui's `Context` must be driven from the loop), and the tray's window proc runs
/// *inside* message dispatch, where re-entering the controller would run a frame
/// inside a frame. Both append here and wake the loop instead.
#[derive(Debug, Clone)]
pub enum Request {
    NewNote,
    CloseAll,
    ToggleVisible,
    /// `Ctrl+Alt+M`: the workbench window.
    Manager,
    /// Open an existing note in a new window -- the manager's "打开".
    OpenNote(String),
    /// A key was rebound. Handled here because the table, the settings file and
    /// the OS registrations all belong to the controller.
    Rebind { command: Command, chord: String },
    /// The panel is already on screen; rebuild its rows and draw it. The payload
    /// is what the hotkey thread measured, so the shell never re-derives it.
    LauncherShown(launcher::Shown),
    /// Hide the panel, unless a newer show has happened since `epoch`.
    LauncherHidden { epoch: u64 },
    /// Something was clicked or Entered in the panel.
    LauncherAction(LauncherAction),
    Quit,
}

pub struct Config {
    pub settings: SpikeSettings,
    pub scale: f32,
    pub lite: bool,
    pub log: Arc<Log>,
    /// `--store-root`: the app data directory to use instead of `%APPDATA%`.
    ///
    /// The only reason this exists is that developing against the live database
    /// means every test note shows up in the user's list. The gate runs on a copy;
    /// so does a person trying a change.
    pub store_root: Option<PathBuf>,
    /// `--note-id`: open an existing note instead of a new one.
    pub note_id: Option<String>,
    /// `--resident`: start without creating a note.
    ///
    /// The controller is the same either way -- this only decides whether the first
    /// frame spawns a window. It exists because the login entry must own the keys
    /// and the tray without putting a note on screen, and because "nothing is open"
    /// with a tray is a resting state, not a reason to exit (`settle`).
    pub resident: bool,
    /// Where the settings file lives. Needed by the workbench to persist a
    /// rebind; the notes only ever read the values loaded at startup.
    pub settings_path: PathBuf,
    /// `--no-font`: skip the CJK font, so the font+atlas share of the footprint
    /// can be attributed instead of guessed. Diagnostic only.
    pub no_font: bool,
    /// `--font-heap`: hold the font in the heap, the C1 control run.
    pub font_heap: bool,
    /// Everything the panel needs from the environment: the cached monitor
    /// layout (the reveal path must not enumerate displays, §5.1.1), the panel's
    /// physical size, and the `--launcher-bench` budget.
    pub launcher: launcher::Wiring,
}

/// Create the controller and run until the user quits from the tray.
///
/// `instance` is the shell's claim on the session (one shell per session). It is only
/// passed in, never taken here: `main` has to hold it *before* opening the log, so
/// that a second launch forwards instead of truncating the first one's evidence.
pub fn run(config: Config, instance: hub_win::instance::Guard) -> ! {
    let options = eframe::NativeOptions {
        // 1×1 is deliberate: the window exists to own a message queue, and any
        // area it has is area it must paint. It is parked off-screen on the first
        // frame, before eframe reveals it.
        viewport: egui::ViewportBuilder::default()
            .with_title(HUB_TITLE)
            .with_inner_size([1.0, 1.0])
            .with_visible(false)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false),
        vsync: false,
        ..Default::default()
    };

    let result = eframe::run_native(
        HUB_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(Hub::new(cc, config, instance)))),
    );

    match result {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

struct Hub {
    log: Arc<Log>,
    settings: Arc<SpikeSettings>,
    lite: bool,
    scale: f32,
    /// One connection for the whole process, shared by every note window.
    ///
    /// `None` if the database could not be opened: the notes keep working and say
    /// `store=off` once, which is the Phase 1 behaviour. Refusing to open a window
    /// because SQLite objected would be the worse failure.
    store: Option<Arc<Mutex<Store>>>,
    notes: Vec<Arc<Shared>>,
    inbox: Arc<Mutex<Vec<Request>>>,
    next_id: u32,
    spawned: u32,
    /// Which key means which command, including the user's overrides. One table
    /// for the process, behind a lock so a rebind in the workbench reaches the
    /// notes that are already open.
    shortcuts: Arc<RwLock<Table>>,
    /// Kept alive for the process lifetime: dropping the handle would not stop the
    /// message loop, and an owned field is what makes that visible.
    _hotkeys: Option<HotkeySet>,
    /// Also owned, and also a lifetime: dropping it removes the icon from the
    /// notification area, which is exactly what would happen by accident if this
    /// field were dropped when the last note closed.
    tray: Option<Tray>,
    /// The workbench, while it is open. `None` means the window does not exist:
    /// closing it drops the shell, which is what makes a later `Ctrl+Alt+M`
    /// create a fresh one instead of revealing a stale list.
    manager: Option<manager::Shell>,
    /// The action panel: a viewport of this process, pre-created and parked, so a
    /// reveal is a move rather than a window creation (§5.8).
    launcher: launcher::Launcher,
    /// Where a rebind is written. Read-modify-write on the file rather than on the
    /// in-memory settings: the notes hold their own copy of the geometry and
    /// appearance, and a second source of truth for keys is the thing this batch
    /// exists to remove.
    settings_path: PathBuf,
    /// C5: the size the next note opens at.
    ///
    /// A cell rather than a field of `settings` because `settings` is a startup
    /// snapshot behind an `Arc` that every note reads without a lock; a resize has
    /// to reach the *next* spawn in the same process, not only the next launch.
    note_size: note::size::SharedSize,
    /// Id whose window is expected to be gone, and how many notes were left.
    /// Logged one pass later, because the OS destroys the window at the end of the
    /// pass that stops declaring it.
    released: Option<(u32, usize)>,
    /// This process's claim on the session.
    ///
    /// Held for the process lifetime, and that is the whole point: dropping it would
    /// release the mutex and let the next launch become a second shell -- a bug that
    /// would only show up as "two tray icons again, sometimes".
    _instance: Arc<hub_win::instance::Guard>,
}

impl Hub {
    fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        instance: hub_win::instance::Guard,
    ) -> Self {
        let ctx = cc.egui_ctx.clone();
        // Read before the struct literal moves the fields it needs: whether to open
        // a window on the first pass is the only difference between the two modes.
        let resident = config.resident;

        // Fonts live on the `Context`, which every viewport shares. Installing
        // them per note would rebuild the atlas once per window for no gain.
        //
        // The *product* path honours `--no-font` too: measuring the font's cost on
        // the spike while the product loads it differently would be measuring a
        // different program (this is what C1 tripped over).
        crate::font::install_and_log(&ctx, &config.log, config.no_font, config.font_heap);

        // `Ctrl+wheel` belongs to window opacity (Q28), not to UI zoom. egui turns
        // that gesture into `set_zoom_factor`, and the zoom factor is global to the
        // `Context` -- so with this left on, one notch over the title strip would
        // dim the note *and* resize its strip, and every other note with it.
        ctx.options_mut(|options| options.zoom_with_keyboard = false);
        config
            .log
            .mark(0, "egui_zoom", "ctrl_wheel=false (opacity owns the gesture)");

        match hub_win::window::find_by_title(HUB_TITLE) {
            Some(hwnd) => {
                let released = hub_win::window::park(hwnd);
                config.log.mark(
                    0,
                    "hub_window",
                    &format!("parked=true focus_released={released}"),
                );
            }
            None => config
                .log
                .mark(0, "hub_window", "parked=false reason=handle_not_found"),
        }

        let table = boot::build_shortcuts(&config);
        let store = boot::open_store(&config);
        // Resolved before the first window exists: a note that cannot find its row
        // must open as a new note and say so, not open empty and overwrite the id.
        let existing = config.note_id.as_ref().map(|id| match store.as_ref() {            Some(store) => {
                let found = store.lock().ok().and_then(|store| store.note(id).ok().flatten());
                config.log.mark(
                    0,
                    "note_load",
                    &format!(
                        "requested={id} found={} reason={}",
                        found.is_some(),
                        if found.is_some() { "explicit" } else { "missing" }
                    ),
                );
                found
            }
            None => None,
        });

        // Read before `config.settings` is moved into the `Arc` below. The cell is
        // what C5 updates: a note that is dragged must change the size of the note
        // opened after it, without a restart.
        let note_size = note::size::shared(
            config.settings.quick_note.width,
            config.settings.quick_note.height,
        );

        // One queue for the hotkey thread, the tray and the launcher's own clicks,
        // created before any of them so they all post into the same ordering.
        let inbox: Arc<Mutex<Vec<Request>>> = Arc::new(Mutex::new(Vec::new()));
        let shortcuts = Arc::new(RwLock::new(table));
        let launcher = launcher::Launcher::new(&config, store.clone(), shortcuts.clone(), inbox.clone(), &ctx);

        let mut hub = Self {
            log: config.log,
            settings: Arc::new(config.settings),
            lite: config.lite,
            scale: config.scale,
            store,
            shortcuts,
            settings_path: config.settings_path,
            note_size,
            notes: Vec::new(),
            inbox,
            next_id: 0,
            spawned: 0,
            _hotkeys: None,
            tray: None,
            manager: None,
            launcher,
            released: None,
            _instance: Arc::new(instance),
        };
        hub._hotkeys = boot::register_hotkeys(
            hub.inbox.clone(),
            ctx.clone(),
            &hub.log,
            &hub.shortcuts.read().unwrap_or_else(|poisoned| poisoned.into_inner()),
            hub.launcher.handle(),
        );
        hub.tray = boot::install_tray(hub.inbox.clone(), ctx.clone(), &hub.log);
        // Listening starts here rather than in `run`: the doorbell is already wired
        // (the event was created with the mutex), so a launch that arrived while the
        // shell was still booting is still waiting on it. See `hub_win::instance`.
        boot::watch_instance(hub._instance.clone(), hub.launcher.handle(), &hub.log);
        hub.log.mark(
            0,
            "hub_start",
            &format!(
                "notes={} resident={resident} lite={} scale={:.2} root={HUB_TITLE} tray={} launcher=viewport",
                if resident { 0 } else { 1 },
                hub.lite,
                hub.scale,
                if hub.tray.is_some() { "on" } else { "off" }
            ),
        );
        if resident {
            // Nothing to open. `settle` is the same decision the last note's close
            // makes: with a tray this is a resting state, without one it is a
            // windowless process nobody can reach, so it ends here instead of
            // sitting invisible with no icon.
            hub.settle();
        } else {
            hub.spawn(&ctx, existing.flatten());
        }
        hub
    }

    /// Resolve geometry and create a note. Called on the event-loop thread only.
    ///
    /// `existing` is the row to open, or `None` for a new note. The row travels with
    /// the spawn rather than being fetched inside the shell: the shell has no business
    /// blocking its first frame on a query, and the controller is where the store
    /// lives.
    fn spawn(&mut self, ctx: &egui::Context, existing: Option<NoteRow>) {
        let id = self.next_id;
        self.next_id += 1;
        self.spawned += 1;

        // C5: the remembered size, not the startup one. Read here rather than at
        // `Hub::new` so a drag in window 2 already applies to window 3.
        let (width, height) = {
            let size = self
                .note_size
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *size
        };

        let spec = NoteSpec {
            width: width as f32,
            height: height as f32,
            margin: self.settings.margin.max(0),
        };

        // Read the cursor *now*: the point of a quick note is that it appears where
        // the user is, not where they were when the frame got around to it.
        let cursor = hub_win::cursor_pos().unwrap_or((0, 0));
        let (work, monitor_scale) = {
            let cache = MonitorCache::refresh();
            let monitor = cache.at(cursor.0, cursor.1);
            (
                Rect::new(
                    monitor.work.0,
                    monitor.work.1,
                    monitor.work.2,
                    monitor.work.3,
                ),
                monitor.scale,
            )
        };
        let placement = place_note(cursor, work, spec, monitor_scale);

        self.notes.push(Shared::new(NoteShell::new(note::Spawn {
            id,
            spec,
            placement,
            scale: monitor_scale,
            settings: self.settings.clone(),
            lite: self.lite,
            log: self.log.clone(),
            store: self.store.clone(),
            existing,
            shortcuts: self.shortcuts.clone(),
            size: self.note_size.clone(),
            settings_path: Arc::new(self.settings_path.clone()),
        })));
        self.log.mark(
            0,
            "note_spawn",
            &format!(
                "id={id} live={} cursor={},{} monitor_scale={monitor_scale:.2} work={}x{}",
                self.notes.len(),
                cursor.0,
                cursor.1,
                work.width(),
                work.height()
            ),
        );
        // The panel's recent list is a snapshot, and a spawn is the only moment it
        // can go stale without a note window telling anyone.
        self.launcher.refresh_recent();
        ctx.request_repaint();
    }

    /// Drop the notes that asked to close and remember the one to log about.
    fn prune(&mut self) {
        let mut live = Vec::with_capacity(self.notes.len());
        let mut closed: Vec<u32> = Vec::new();
        for handle in self.notes.drain(..) {
            let (id, closing) = {
                let shell = handle.lock();
                (shell.id(), shell.is_closing())
            };
            if closing {
                closed.push(id);
            } else {
                live.push(handle);
            }
        }
        self.notes = live;
        if let Some(id) = closed.last().copied() {
            self.released = Some((id, self.notes.len()));
            self.launcher.refresh_recent();
        }
    }

    fn handle(&mut self, ctx: &egui::Context, request: Request) {
        match request {
            Request::NewNote => self.spawn(ctx, None),
            Request::CloseAll => self.close_all(ctx),
            Request::ToggleVisible => self.toggle_visible(ctx),
            Request::Manager => self.open_manager(ctx),
            Request::OpenNote(id) => self.open_note(ctx, &id, "manager"),
            Request::Rebind { command, chord } => self.rebind(ctx, command, &chord),
            Request::LauncherShown(shown) => self.launcher.shown(ctx, shown),
            Request::LauncherHidden { epoch } => self.launcher.hidden(epoch),
            Request::LauncherAction(action) => self.launcher_action(ctx, action),
            Request::Quit => self.quit(),
        }
    }

    fn close_all(&mut self, ctx: &egui::Context) {
        let notes: Vec<Arc<Shared>> = self.notes.clone();
        for handle in &notes {
            handle.lock().request_close(ctx, "all");
        }
        self.log
            .mark(0, "note_close_all", &format!("notes={}", notes.len()));
    }

    /// `Ctrl+Alt+H`: minimize every note, or bring them all back.
    ///
    /// The direction is chosen the way the shipped build chooses it -- "if anything is
    /// still active, collapse; otherwise restore" -- so pressing it twice always
    /// returns to the state it started in, even if the user restored one note by hand.
    fn toggle_visible(&mut self, ctx: &egui::Context) {
        let notes: Vec<Arc<Shared>> = self.notes.clone();
        if notes.is_empty() {
            self.log.mark(0, "notes_visible", "action=none notes=0");
            return;
        }
        let any_active = notes.iter().any(|handle| !handle.lock().minimized());
        if any_active {
            for handle in &notes {
                handle.lock().minimize();
            }
        } else {
            for handle in &notes {
                handle.lock().restore();
            }
            // Same as the shipped build: focus follows the first note back, otherwise
            // the keyboard lands wherever it happened to be.
            if let Some(first) = notes.first() {
                first.lock().focus();
            }
            // A collapsed note skips its own frames (its client area is degenerate),
            // so the restore is what has to ask for the first frame back. `SW_RESTORE`
            // produces one anyway; asking makes the redraw independent of that.
            for handle in &notes {
                ctx.request_repaint_of(note::viewport_id(handle.lock().id()));
            }
        }
        self.log.mark(
            0,
            "notes_visible",
            &format!(
                "action={} notes={}",
                if any_active { "minimize" } else { "restore" },
                notes.len()
            ),
        );
    }

    /// Nothing is open. With a tray that is a resting state; without one it is a
    /// process the user cannot see or reach, so it ends.
    ///
    /// The decision is made from evidence (`self.tray`) rather than from a flag, which
    /// is why a failed `Shell_NotifyIconW` degrades to the 1b.2 behaviour instead of
    /// leaving an invisible zombie behind.
    fn settle(&mut self) {
        match self.tray.as_ref() {
            Some(tray) => self
                .log
                .mark(0, "hub_idle", &format!("notes=0 tray=0x{:X}", tray.hwnd())),
            None => {
                self.log
                    .mark(0, "hub_exit", "reason=no_notes tray=none hotkey=stops");
                std::process::exit(0);
            }
        }
    }

    /// Leave. The tray icon goes first: `std::process::exit` runs no destructors, and
    /// a leftover icon stays in the notification area until the user hovers it.
    fn quit(&mut self) {
        let notes = self.notes.len();
        if let Some(tray) = self.tray.as_mut() {
            tray.remove();
        }
        self.log
            .mark(0, "hub_exit", &format!("reason=tray notes={notes}"));
        std::process::exit(0);
    }
}

impl eframe::App for Hub {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Never seen: the root is 1×1 and parked. Kept opaque so a compositor that
        // ignores the parking spot does not show through to the desktop.
        [0.0, 0.0, 0.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // C1 is about the font *and* the atlas, and from outside the process the
        // two are one indistinguishable delta. The render thread deposits the
        // atlas size every pass so `--probe` can report it at snapshot time.
        crate::probe_json::record_atlas(ctx);

        // Evidence for the window that went away last pass. Measured here rather
        // than in `NoteShell::close` because at that point the window still exists
        // -- "asked to close" and "closed" must not look the same in the log.
        if let Some((id, live)) = self.released.take() {
            let windows = hub_win::window::top_level_windows();
            self.log.mark(
                0,
                "note_destroyed",
                &format!(
                    "id={id} open_notes={} process_windows={} notes={live}",
                    note::open_notes(),
                    windows.len()
                ),
            );
            if live == 0 {
                self.settle();
            }
        }

        let requests: Vec<Request> = {
            let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
            inbox.drain(..).collect()
        };
        for request in requests {
            self.handle(ctx, request);
        }

        self.prune();

        // The workbench is an immediate child viewport: it has no island and no
        // per-window state that has to survive a pass, so a deferred viewport
        // would only add a frame of latency to its own clicks.
        if let Some(shell) = self.manager.as_mut() {
            let builder = shell.builder();
            ctx.show_viewport_immediate(manager::viewport_id(), builder, |ctx, _class| {
                shell.frame(ctx);
            });
        }
        if self.manager.as_ref().is_some_and(|shell| shell.closing()) {
            self.manager = None;
        }

        // The panel is a deferred viewport like a note: its window has to exist
        // before a hotkey can move it, and it must keep existing while hidden.
        self.launcher.viewport(ctx);

        if let Some((report, ok)) = self.launcher.bench(ctx) {
            print!("{report}");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            std::process::exit(if ok { 0 } else { 2 });
        }

        for handle in &self.notes {
            let (id, builder) = {
                let shell = handle.lock();
                (shell.id(), shell.builder())
            };
            let shell = handle.clone();
            ctx.show_viewport_deferred(note::viewport_id(id), builder, move |ctx, _class| {
                shell.lock().frame(ctx);
            });
        }

        // A note whose window has not been found yet cannot request its own frames,
        // so the controller keeps nudging until every note is attached.
        if self.notes.iter().any(|handle| !handle.lock().attached()) {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}
