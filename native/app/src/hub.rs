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
//! counter and the list of live shells. It never renders a note and never touches
//! a RichEdit; a note's frame runs in that viewport's own callback, which keeps
//! the "one window, one island" pairing visible in the code.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use hub_core::{place_note, NoteSpec, Rect, SpikeSettings};
use hub_win::hotkey::{Hotkey, PressHandler};
use hub_win::MonitorCache;

use crate::log::Log;
use crate::note::{self, NoteShell, Shared};

/// Title of the controller window. Never shown, never activated.
pub const HUB_TITLE: &str = "easyflowhub-hub";

/// The shipped "new note" combination (parity Q18).
const NEW_NOTE_COMBO: &str = "Ctrl+Alt+N";

/// Same debounce the spike uses: QQ/WeChat hook the same combinations, so one
/// physical press can arrive as two or three `WM_HOTKEY` messages.
const HOTKEY_DEBOUNCE: Duration = Duration::from_millis(250);

/// A note request that has to be executed on the event-loop thread.
///
/// The hotkey thread cannot create a window (win32 windows are thread-affine and
/// egui's `Context` must be driven from the loop), so it appends here and wakes
/// the loop. Batch 1b.2 has one request; the tray will add the others.
#[derive(Debug, Clone, Copy)]
pub enum Request {
    NewNote,
}

pub struct Config {
    pub settings: SpikeSettings,
    pub scale: f32,
    pub lite: bool,
    pub log: Arc<Log>,
}

/// Create the controller and run until the last note is gone.
pub fn run(config: Config) -> ! {
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
        Box::new(move |cc| Ok(Box::new(Hub::new(cc, config)))),
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
    notes: Vec<Arc<Shared>>,
    inbox: Arc<Mutex<Vec<Request>>>,
    next_id: u32,
    spawned: u32,
    /// Kept alive for the process lifetime: dropping the handle would not stop the
    /// message loop, and an owned field is what makes that visible.
    _hotkey: Option<Hotkey>,
    /// Id whose window is expected to be gone, and how many notes were left.
    /// Logged one pass later, because the OS destroys the window at the end of the
    /// pass that stops declaring it.
    released: Option<(u32, usize)>,
}

impl Hub {
    fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        let ctx = cc.egui_ctx.clone();

        // Fonts live on the `Context`, which every viewport shares. Installing
        // them per note would rebuild the atlas once per window for no gain.
        match hub_ui::theme::install_fonts(&ctx) {
            Some(font) => config.log.mark(0, "fonts", &format!("cjk={font}")),
            None => config
                .log
                .mark(0, "fonts", "no CJK font found; text renders as tofu"),
        }

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

        let mut hub = Self {
            log: config.log,
            settings: Arc::new(config.settings),
            lite: config.lite,
            scale: config.scale,
            notes: Vec::new(),
            inbox: Arc::new(Mutex::new(Vec::new())),
            next_id: 0,
            spawned: 0,
            _hotkey: None,
            released: None,
        };
        hub._hotkey = register_new_note(hub.inbox.clone(), ctx.clone(), &hub.log);
        hub.log.mark(
            0,
            "hub_start",
            &format!(
                "notes=1 lite={} scale={:.2} root={HUB_TITLE}",
                hub.lite, hub.scale
            ),
        );
        hub.spawn(&ctx);
        hub
    }

    /// Resolve geometry and create a note. Called on the event-loop thread only.
    fn spawn(&mut self, ctx: &egui::Context) {
        let id = self.next_id;
        self.next_id += 1;
        self.spawned += 1;

        let spec = NoteSpec {
            width: self.settings.quick_note.width as f32,
            height: self.settings.quick_note.height as f32,
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
        }
    }
}

impl eframe::App for Hub {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Never seen: the root is 1×1 and parked. Kept opaque so a compositor that
        // ignores the parking spot does not show through to the desktop.
        [0.0, 0.0, 0.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                // No tray yet, so there is nothing left for the user to click: the
                // parked root would be an invisible process that only Task Manager
                // can close, and Ctrl+Alt+N would reopen a note nobody can see the
                // state of. Closing the last note therefore ends the run, and the
                // log says so instead of letting "it kept running" or "it vanished"
                // look like a crash. The tray batch flips this line's decision, not
                // its fields.
                self.log
                    .mark(0, "hub_exit", "reason=no_notes tray=pending hotkey=stops");
                std::process::exit(0);
            }
        }

        let requests: Vec<Request> = {
            let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
            inbox.drain(..).collect()
        };
        for request in requests {
            match request {
                Request::NewNote => self.spawn(ctx),
            }
        }

        self.prune();

        for handle in &self.notes {
            let (id, builder) = {
                let shell = handle.lock();
                (shell.id(), shell.builder())
            };
            let shell = handle.clone();
            ctx.show_viewport_deferred(
                egui::ViewportId::from_hash_of(("note", id)),
                builder,
                move |ctx, _class| {
                    shell.lock().frame(ctx);
                },
            );
        }

        // A note whose window has not been found yet cannot request its own frames,
        // so the controller keeps nudging until every note is attached.
        if self.notes.iter().any(|handle| !handle.lock().attached()) {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

/// `Ctrl+Alt+N` → append a request and wake the event loop.
///
/// The hotkey thread owns no window and must not run egui code, so it does the two
/// things that are thread-safe: push to a mutex-guarded queue and ask for a
/// repaint. `request_repaint_of(ROOT)` rather than `request_repaint()`: outside a
/// pass the context's "current viewport" is whatever ran last, and waking a note
/// would leave the request sitting in the queue.
fn register_new_note(inbox: Arc<Mutex<Vec<Request>>>, ctx: egui::Context, log: &Log) -> Option<Hotkey> {
    let handler: PressHandler = Arc::new(move || {
        inbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(Request::NewNote);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    });

    match Hotkey::spawn(NEW_NOTE_COMBO, HOTKEY_DEBOUNCE, handler) {
        Ok(hotkey) => {
            log.mark(
                0,
                "hotkey",
                &format!("combo={NEW_NOTE_COMBO} registered=true thread={}", hotkey.thread_id()),
            );
            Some(hotkey)
        }
        // Not fatal: another program owning the combination is the user's problem to
        // see, not a reason to refuse to open a note. The log is the only place it
        // can show up until there is a tray or a settings pane.
        Err(error) => {
            log.mark(
                0,
                "hotkey",
                &format!("combo={NEW_NOTE_COMBO} registered=false {error}"),
            );
            None
        }
    }
}
