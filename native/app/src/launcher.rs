//! The launcher window: the action panel, as a real viewport of this process.
//!
//! ## Split from the note shell on purpose
//!
//! A note owns a RichEdit island, an autosave pump and a keyboard that belongs to
//! the control. The launcher owns none of that: it is egui all the way down, it
//! never saves anything, and its keyboard is egui's. Sharing `NoteShell` would
//! mean carrying a dozen unused fields and a close path that destroys a window
//! which must survive being closed.
//!
//! ## Where the reveal happens
//!
//! In [`Handle::toggle`], which runs on the hotkey thread. It reads the cursor,
//! hits the cached monitor layout, computes the clamped rectangle and moves the
//! window -- the same four steps the S1 spike proved at p95 48.9 ms (§5.8: "按键时
//! 无可做事"). The content refresh is queued to the UI thread and is *not* on the
//! reveal path: the window that appears already holds the last drawn panel.
//!
//! Hiding is the other way round. `park` releases the keyboard with `SetFocus`,
//! and `GetFocus` answers for the calling thread, so hiding from the hotkey thread
//! would move the window and leave it owning the keyboard -- invisible and still
//! typing. So a hide is a request: the UI thread parks it.
//!
//! ## Why `epoch`
//!
//! A hide request and a show can cross: press the key, press it again before the
//! UI thread wakes. The hide carries the epoch it was issued at, and is dropped if
//! a newer show has happened since. Without it the panel would be parked right
//! after being put on screen, and the user sees a key that does nothing.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use hub_core::{place, Command, Placement, Rect, Table};
use hub_ui::launcher;
use hub_ui::{LauncherAction, LauncherItem, LauncherState};
use hub_win::MonitorCache;

use crate::bench::{self, Samples};
use crate::hub::Request;
use crate::log::Log;
use hub_core::Store;

mod recent;

/// Title of the panel window. Also the only way back to its `HWND`: eframe 0.31
/// does not expose it through `App` (see `hub_win::window::find_by_title`).
pub const TITLE: &str = "easyflowhub-launcher";

/// Frames drawn off-screen before the first reveal.
///
/// Three for the same reason the spike uses three: the first allocates the atlas
/// and compiles shaders, the second uploads the glyphs the first touched, the
/// third is what a reveal will actually look like.
const WARM_FRAMES: u32 = 3;

/// While the panel is visible, look at the focus this often.
///
/// A click outside is delivered to the panel as a focus change, and eframe does
/// wake on it -- but only if the viewport was already awake. 5 Hz is enough to
/// notice within one glance, and costs nothing measurable: the panel is on screen
/// for seconds at a time.
const BLUR_POLL: Duration = Duration::from_millis(200);

pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("launcher")
}

/// The hotkey thread's half: geometry, show, focus.
pub struct Handle {
    spec: hub_core::PanelSpec,
    monitors: Arc<Mutex<MonitorCache>>,
    hwnd: AtomicIsize,
    visible: AtomicBool,
    epoch: AtomicU64,
    inbox: Arc<Mutex<Vec<Request>>>,
    ctx: egui::Context,
    log: Arc<Log>,
}

impl Handle {
    pub fn new(
        spec: hub_core::PanelSpec,
        monitors: Arc<Mutex<MonitorCache>>,
        inbox: Arc<Mutex<Vec<Request>>>,
        ctx: egui::Context,
        log: Arc<Log>,
    ) -> Arc<Self> {
        Arc::new(Self {
            spec,
            monitors,
            hwnd: AtomicIsize::new(0),
            visible: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            inbox,
            ctx,
            log,
        })
    }

    /// Called by the shell once the OS window exists.
    pub fn attach(&self, hwnd: isize) {
        self.hwnd.store(hwnd, Ordering::SeqCst);
    }

    pub fn visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }

    pub fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::SeqCst);
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    /// The reveal path. Runs on the hotkey thread; nothing here waits.
    pub fn toggle(&self) {
        let hwnd = self.hwnd.load(Ordering::SeqCst);
        if hwnd == 0 {
            // Pressed between process start and the first frame. Saying so is the
            // difference between "the key is broken" and "it was not ready yet".
            self.log
                .mark(0, "launcher_toggle", "ready=false reason=no_window");
            return;
        }
        let at = Instant::now();
        let epoch = self.epoch.fetch_add(1, Ordering::SeqCst) + 1;

        if self.visible.swap(false, Ordering::SeqCst) {
            // Hiding needs the UI thread (see the module comment); the epoch rides
            // along so a show that happens first is not undone by this hide.
            self.push(Request::LauncherHidden { epoch });
            return;
        }

        let cursor = hub_win::cursor_pos().unwrap_or((0, 0));
        let (work, monitor_scale) = {
            let cache = self
                .monitors
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        let placement = place(cursor, work, &self.spec);

        hub_win::window::move_on_screen(
            hwnd,
            placement.x,
            placement.y,
            placement.width,
            placement.height,
            true,
        );
        let moved_us = at.elapsed().as_micros() as u64;
        let focused = hub_win::window::focus(hwnd);
        let shown_us = at.elapsed().as_micros() as u64;
        self.visible.store(true, Ordering::SeqCst);

        self.log.mark(
            epoch,
            "launcher_shown",
            &format!(
                "shown_ms={:.2} move_ms={:.2} focus_ms={:.2} focused={focused} cursor={},{} monitor_scale={monitor_scale:.2} at=+{}+{} size={}x{} cols={} work={}x{}",
                shown_us as f64 / 1000.0,
                moved_us as f64 / 1000.0,
                (shown_us - moved_us) as f64 / 1000.0,
                cursor.0,
                cursor.1,
                placement.x,
                placement.y,
                placement.width,
                placement.height,
                placement.columns,
                work.width(),
                work.height()
            ),
        );

        self.push(Request::LauncherShown(Shown {
            at,
            shown_us,
            focused,
            placement,
            epoch,
        }));
    }

    fn push(&self, request: Request) {
        self.inbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);
        self.ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
}

/// What the controller has to hand the panel at startup.
///
/// Grouped rather than spread over `hub::Config` so the environment facts a reveal
/// depends on sit next to the code that reads them on the hotkey thread.
pub struct Wiring {
    /// The cached monitor layout. Read on the hotkey thread, where enumerating
    /// displays is forbidden (§5.1.1).
    pub monitors: Arc<Mutex<MonitorCache>>,
    /// Physical width, height and margin of the panel.
    pub spec: hub_core::PanelSpec,
    /// `--launcher-bench N`: reveal the panel N times, print, exit.
    pub bench: Option<usize>,
    /// p95 / p99 budget for the reveal (§7.1: 60 / 100 ms).
    pub budget_p95_ms: f64,
    pub budget_p99_ms: f64,
}

/// The panel as the shell holds it: the window half and the UI half behind one
/// field, so the controller's per-frame code is a call rather than a lock dance.
pub struct Launcher {
    shell: Arc<Mutex<Shell>>,
    handle: Arc<Handle>,
}

impl Launcher {
    /// Build both halves from the controller's own configuration.
    pub fn new(
        config: &crate::hub::Config,
        store: Option<Arc<Mutex<Store>>>,
        table: Arc<RwLock<Table>>,
        inbox: Arc<Mutex<Vec<Request>>>,
        ctx: &egui::Context,
    ) -> Self {
        let wiring = &config.launcher;
        let handle = Handle::new(
            wiring.spec,
            wiring.monitors.clone(),
            inbox.clone(),
            ctx.clone(),
            config.log.clone(),
        );
        let shell = Shell::new(
            Spawn {
                log: config.log.clone(),
                store,
                table,
                lite: config.lite,
                scale: config.scale,
                spec: wiring.spec,
                inbox,
                handle: handle.clone(),
            },
            wiring.bench,
            wiring.budget_p95_ms,
            wiring.budget_p99_ms,
        );
        Self {
            shell: Arc::new(Mutex::new(shell)),
            handle,
        }
    }

    /// The half the hotkey thread uses.
    pub fn handle(&self) -> Arc<Handle> {
        self.handle.clone()
    }

    fn with<R>(&self, f: impl FnOnce(&mut Shell) -> R) -> R {
        let mut shell = self
            .shell
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut shell)
    }

    pub fn refresh_recent(&self) {
        self.with(|shell| shell.refresh_recent());
    }

    pub fn shown(&self, ctx: &egui::Context, shown: Shown) {
        self.with(|shell| shell.on_shown(ctx, shown));
    }

    pub fn hidden(&self, epoch: u64) {
        self.with(|shell| shell.hide_if_current(epoch, "hotkey"));
    }

    /// Park the panel now, from the UI thread. Called before an action opens a
    /// window, so the panel cannot cover what the keystroke just opened.
    pub fn hide(&self, reason: &str) {
        self.with(|shell| shell.hide(reason));
    }

    /// Declare the panel's viewport for this pass.
    ///
    /// Deferred, like a note's: the window has to exist before a hotkey can move
    /// it, and it must keep existing while hidden -- so the controller re-declares
    /// it every frame and the shell decides what to draw.
    pub fn viewport(&self, ctx: &egui::Context) {
        let builder = self.with(|shell| shell.builder());
        let shell = self.shell.clone();
        ctx.show_viewport_deferred(viewport_id(), builder, move |ctx, _class| {
            let mut shell = shell
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            shell.frame(ctx);
        });
    }

    /// `--launcher-bench`: one step per pass, and the report once it is done.
    pub fn bench(&self, ctx: &egui::Context) -> Option<(String, bool)> {
        self.with(|shell| shell.bench_tick(ctx));
        self.with(|shell| shell.bench_result())
    }
}

/// The press handler for `Command::Launcher`.
///
/// Not a queue entry: it moves the window on the hotkey thread, because waiting
/// for the UI thread to wake is 30-47 ms of the 60 ms budget (§5.8).
pub fn binding(chord: &hub_core::Chord, handle: Arc<Handle>) -> hub_win::hotkey::Binding {
    hub_win::hotkey::Binding::new(
        chord.display(),
        Arc::new(move || handle.toggle()) as hub_win::hotkey::PressHandler,
    )
}

pub struct Spawn {
    pub log: Arc<Log>,
    pub store: Option<Arc<Mutex<Store>>>,
    pub table: Arc<RwLock<Table>>,
    pub lite: bool,
    pub scale: f32,
    pub spec: hub_core::PanelSpec,
    pub inbox: Arc<Mutex<Vec<Request>>>,
    pub handle: Arc<Handle>,
}

/// A reveal waiting for its first painted frame.
#[derive(Debug, Clone, Copy)]
struct Pending {
    at: Instant,
    shown_us: u64,
    epoch: u64,
}

/// What the hotkey thread measured about a reveal it has already performed.
///
/// Also the payload of `Request::LauncherShown`: the UI thread rebuilds nothing it
/// would have to re-derive, so the column count cannot disagree with the size of
/// the window the user is looking at.
#[derive(Debug, Clone, Copy)]
pub struct Shown {
    pub at: Instant,
    pub shown_us: u64,
    pub focused: bool,
    pub placement: Placement,
    pub epoch: u64,
}

/// The automated reveal loop (`--launcher-bench`).
pub struct Bench {
    total: usize,
    completed: usize,
    interval: Duration,
    last: Option<Instant>,
    samples: Samples,
    start: hub_win::ProcessProbe,
    budget_p95_ms: f64,
    budget_p99_ms: f64,
}

pub struct Shell {
    state: LauncherState,
    hwnd: Option<isize>,
    handle: Arc<Handle>,
    log: Arc<Log>,
    store: Option<Arc<Mutex<Store>>>,
    table: Arc<RwLock<Table>>,
    inbox: Arc<Mutex<Vec<Request>>>,
    recent: Vec<recent::Recent>,
    /// Frames still owed to the off-screen warm-up.
    warm_left: u32,
    /// Whether the frame drawn last pass was the first one of a reveal.
    drawn: bool,
    /// Set once the OS window reports focus while visible, so the very first
    /// blur check cannot fire on a panel that has not been activated yet.
    focused_once: bool,
    /// The reveal whose first painted frame is still owed.
    pending: Option<Pending>,
    /// `--launcher-bench`: a hit counter that also makes the shell the driver.
    bench: Option<Bench>,
    /// Previous query, so a keystroke is logged once instead of every frame.
    last_query: String,
    /// Whether `launcher_ready` has been logged for the reveal in progress.
    keyboard_logged: bool,
    /// The window's size and margin, in **physical** pixels (`desired * scale`).
    spec: hub_core::PanelSpec,
    /// Needed to turn the physical spec into the viewport's own points, which is
    /// the unit eframe gives to `with_inner_size`.
    scale: f32,
}

impl Shell {
    pub fn new(spawn: Spawn, bench_total: Option<usize>, budget_p95_ms: f64, budget_p99_ms: f64) -> Self {
        let mut shell = Self {
            state: LauncherState::new(4, spawn.lite),
            hwnd: None,
            spec: spawn.spec,
            scale: if spawn.scale > 0.1 { spawn.scale } else { 1.0 },
            log: spawn.log,
            store: spawn.store,
            table: spawn.table,
            inbox: spawn.inbox,
            handle: spawn.handle,
            recent: Vec::new(),
            warm_left: WARM_FRAMES,
            drawn: false,
            focused_once: false,
            pending: None,
            last_query: String::new(),
            keyboard_logged: false,
            bench: bench_total.map(|total| Bench {
                total,
                completed: 0,
                interval: Duration::from_millis(400),
                last: None,
                samples: Samples::default(),
                start: hub_win::ProcessProbe::capture(),
                budget_p95_ms,
                budget_p99_ms,
            }),
        };
        shell.refresh_recent();
        shell.state.open_hint = shell.hint(Command::Launcher);
        shell
    }

    /// Size the window in points, at the scale of the monitor it starts on.
    pub fn builder(&self) -> egui::ViewportBuilder {
        let width = self.state_width() / self.scale;
        let height = self.state_height() / self.scale;
        egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([width, height])
            .with_visible(false)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top()
    }

    /// Physical width the window is created at.
    ///
    /// `place` may shrink it on a small work area; the panel is not resized on
    /// reveal because that would put a resize on the reveal path (§5.8), and a
    /// 1366-wide screen is the only case that needs it -- there `move_on_screen`
    /// sets the smaller size directly.
    fn state_width(&self) -> f32 {
        self.spec.desired_width
    }

    fn state_height(&self) -> f32 {
        self.spec.desired_height
    }

    /// Reload the recent list from the store (see [`recent`]).
    pub fn refresh_recent(&mut self) {
        recent::refresh(self);
    }

    fn hint(&self, command: Command) -> String {
        self.table
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .hint(command)
    }

    fn items(&self) -> Vec<LauncherItem> {
        recent::items(self)
    }

    /// Create, position and park the OS window -- once.
    fn attach(&mut self, ctx: &egui::Context) -> bool {
        let Some(hwnd) = hub_win::window::find_by_title(TITLE) else {
            return false;
        };
        let dwm = hub_win::dwm::apply(hwnd, false);
        let width = self.state_width().round() as i32;
        let height = self.state_height().round() as i32;
        hub_win::window::place_hidden(hwnd, 0, 0, width, height);
        // Visible but off-screen: a hidden window receives no redraws (see
        // `hub_win::window`), which would starve both the warm frames and every
        // later refresh.
        hub_win::window::show_without_activation(hwnd);
        let released = hub_win::window::park(hwnd);
        self.hwnd = Some(hwnd);
        self.handle.attach(hwnd);
        self.log.mark(
            0,
            "launcher_window",
            &format!(
                "parked=true dwm={dwm:?} focus_released={released} size={width}x{height} scale={:.2}",
                self.scale
            ),
        );
        // The warm frames are what the reveal will show; ask for them explicitly
        // because a parked viewport gets no repaints of its own.
        ctx.request_repaint_of(viewport_id());
        true
    }

    /// One pass of the viewport. Called every time eframe repaints it.
    pub fn frame(&mut self, ctx: &egui::Context) {
        if self.hwnd.is_none() && !self.attach(ctx) {
            // The window is created by eframe during this pass; a miss means the
            // lookup raced it. Ask again rather than park the panel half-built.
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }

        self.settle();

        if self.handle.visible() && self.focused_once {
            // The keyboard went to another window: that is a click outside as far
            // as the panel is concerned. Asked of the OS rather than of egui,
            // because eframe only learns about activation when an event wakes the
            // viewport, and a panel that waits for that can sit on screen for
            // seconds after the user has moved on.
            let front = hub_win::window::foreground();
            if front != 0 && Some(front) != self.hwnd {
                self.hide("blur");
                return;
            }
        }

        if !self.handle.visible() {
            if self.warm_left == 0 {
                return;
            }
            self.warm_left -= 1;
            ctx.request_repaint_of(viewport_id());
        }

        let action = launcher::draw(ctx, &mut self.state);
        self.drawn = true;

        // The frame the text box took the keyboard. Everything before it is a
        // reveal in progress; everything after is typing that lands.
        if self.handle.visible() && self.state.keyboard && !self.keyboard_logged {
            self.keyboard_logged = true;
            self.log.mark(0, "launcher_ready", "keyboard=true");
        }

        // One line per keystroke, not per frame: the gate has to be able to wait
        // for "the query took effect", and a per-frame line would make that wait
        // succeed on a frame where nothing had been typed yet.
        if self.state.query != self.last_query {
            self.last_query = self.state.query.clone();
            self.log.mark(
                0,
                "launcher_query",
                &format!(
                    "text=\"{}\" matches={}",
                    self.last_query,
                    self.state.visible().len()
                ),
            );
        }

        if self.handle.visible() {
            if Some(hub_win::window::foreground()) == self.hwnd {
                self.focused_once = true;
            }
            ctx.request_repaint_after(BLUR_POLL);
        }

        if !matches!(action, LauncherAction::None) {
            self.log
                .mark(0, "launcher_action", &format!("action={action:?}"));
            self.inbox
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(Request::LauncherAction(action));
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }

    /// The UI-thread half of a reveal: rebuild the rows, take the keyboard, draw.
    fn on_shown(&mut self, ctx: &egui::Context, shown: Shown) {
        self.state.columns = shown.placement.columns;
        self.state.open_hint = self.hint(Command::Launcher);
        self.state.reveal_count = shown.epoch;
        self.state.on_open();
        self.state.set_items(self.items());
        self.pending = Some(Pending {
            at: shown.at,
            shown_us: shown.shown_us,
            epoch: shown.epoch,
        });
        self.drawn = false;
        self.focused_once = false;
        self.keyboard_logged = false;

        if !shown.focused {
            // Second chance for the foreground lock. A global hotkey that came from
            // a real keystroke normally grants it; when it does not, a keystroke
            // into a panel that does not own the keyboard would go to whatever is
            // behind it, so the result is logged rather than assumed.
            let hwnd = self.hwnd.unwrap_or(0);
            let again = if hwnd != 0 {
                hub_win::window::focus(hwnd)
            } else {
                false
            };
            self.log.mark(
                shown.epoch,
                "launcher_focus",
                &format!("first=false retry={again}"),
            );
        }
        ctx.request_repaint_of(viewport_id());
    }

    /// A hide request. Dropped when a newer show has superseded it.
    pub fn hide_if_current(&mut self, epoch: u64, reason: &str) {
        if self.handle.epoch() != epoch {
            self.log.mark(
                0,
                "launcher_hide",
                &format!("reason={reason} skipped=superseded epoch={epoch}"),
            );
            return;
        }
        self.hide(reason);
    }

    pub fn hide(&mut self, reason: &str) {
        if !self.handle.visible() {
            return;
        }
        let hwnd = self.hwnd.unwrap_or(0);
        let released = if hwnd != 0 {
            hub_win::window::park(hwnd)
        } else {
            0
        };
        self.handle.set_visible(false);
        self.focused_once = false;
        self.log.mark(
            0,
            "launcher_hide",
            &format!(
                "reason={reason} focus_released={released} focus_after={}",
                hub_win::window::current_focus()
            ),
        );
    }

    /// Record the first painted frame of the reveal that is settling.
    fn settle(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        if !self.drawn {
            // Nothing was painted, so there is no first frame to time. Put it back
            // and try again next pass rather than record a meaningless sample.
            self.pending = Some(pending);
            return;
        }
        self.drawn = false;
        let refresh_us = pending.at.elapsed().as_micros() as u64;
        self.log.mark(
            pending.epoch,
            "launcher_first_frame",
            &format!(
                "shown_ms={:.2} refresh_ms={:.2}",
                pending.shown_us as f64 / 1000.0,
                refresh_us as f64 / 1000.0
            ),
        );

        let Some(bench) = self.bench.as_mut() else {
            return;
        };
        bench.samples.push(pending.shown_us, refresh_us);
        bench.completed += 1;
        if let Some(hwnd) = self.hwnd {
            hub_win::window::park(hwnd);
        }
        self.handle.set_visible(false);
        self.focused_once = false;
        bench.last = Some(Instant::now());
    }

    /// `--launcher-bench`: press the key again when the previous cycle is done.
    pub fn bench_tick(&mut self, ctx: &egui::Context) {
        let Some(bench) = self.bench.as_ref() else {
            return;
        };
        if bench.completed >= bench.total || self.handle.visible() {
            return;
        }
        if let Some(last) = bench.last {
            let elapsed = last.elapsed();
            if elapsed < bench.interval {
                ctx.request_repaint_of(egui::ViewportId::ROOT);
                return;
            }
        }
        // The same entry point the hotkey thread calls. The OS dispatch is the one
        // part a bench cannot include; everything this process controls is here.
        self.handle.toggle();
    }

    /// The report, once every cycle has landed.
    pub fn bench_result(&mut self) -> Option<(String, bool)> {
        let bench = self.bench.as_ref()?;
        if bench.completed < bench.total {
            return None;
        }
        let end = hub_win::ProcessProbe::capture();
        let report = bench::report(&bench.samples, bench.budget_p95_ms, bench.budget_p99_ms);
        let json = bench::report_json(
            &bench.samples,
            bench.budget_p95_ms,
            bench.budget_p99_ms,
            None,
        );
        let leak = format!(
            "{{\"kind\":\"native-launcher-leak\",\"cycles\":{},\"start_private_mb\":{:.1},\"end_private_mb\":{:.1},\"private_delta_mb\":{:.1},\"start_handles\":{},\"end_handles\":{},\"handles_delta\":{}}}",
            bench.completed,
            bench.start.private_mb(),
            end.private_mb(),
            end.private_mb() - bench.start.private_mb(),
            bench.start.handles,
            end.handles,
            end.handles as i64 - bench.start.handles as i64
        );
        let shown = bench::distribution(&bench.samples.shown_us);
        let ok = shown.p95 as f64 / 1000.0 <= bench.budget_p95_ms
            && shown.p99 as f64 / 1000.0 <= bench.budget_p99_ms;
        Some((format!("{report}---JSON---\n{json}---LEAK---\n{leak}\n"), ok))
    }
}

