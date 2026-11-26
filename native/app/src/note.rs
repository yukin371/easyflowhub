//! One quick-note window, hosted as an egui viewport.
//!
//! Batch 1a drew this inside the event loop's only window. 1b.2 gives every note
//! its own viewport so that closing one destroys one OS window and leaves the
//! others alone (parity Q5). The process root moved to [`crate::hub`], which is
//! why this file no longer owns `run_native`.
//!
//! ## Window creation order is copied from S1 on purpose
//!
//! `with_visible(false)` → find the handle → place in **physical** pixels → show
//! without activation. That is the order the 48.9 ms p95 was measured in. Batch 1a
//! changes content only, so if a reveal ever looks slower the window path is not
//! the suspect. 1b.2 keeps it for a second reason: a viewport's window only exists
//! once eframe has created it, and `place_hidden` must run before the first frame
//! is composited or the note appears for one frame at the wrong size.
//!
//! ## What "close" means here
//!
//! The note asks its viewport to close; the controller stops declaring the
//! viewport on the next pass, eframe drops the window, and the log records the
//! window count *after* the drop. Neither this file nor the controller keeps a
//! shadow note around: Q5 says the window is gone, so there must be nothing left
//! holding a handle to it.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use std::time::{Duration, Instant};

use hub_core::{
    Command, NoteAppearance, NotePlacement, NoteRow, NoteSpec, PixelRect, SpikeSettings, Store,
    Table, MIN_NOTE_HEIGHT, MIN_NOTE_WIDTH,
};
use hub_editor::Editor;
use hub_ui::quicknote::{self, StripState};
use hub_ui::switcher;
use hub_ui::Palette;

use crate::click;
use crate::keys;
use crate::log::Log;
use crate::save;
use crate::wheel;

mod drag;
mod edge;
mod island;
mod panel;
mod recover;
pub mod size;

/// Prefix of every note window title. The suffix is the note id.
///
/// The title is also the only bridge from a viewport to its `HWND`: eframe 0.31
/// exposes a raw handle on `Frame`, but a deferred viewport's callback only
/// receives a `Context`. Titles are invisible here (no decorations, no taskbar
/// entry), so the suffix costs nothing and keeps the lookup exact instead of
/// "the nth window that looks like a note".
pub const TITLE_PREFIX: &str = "easyflowhub-note";

pub fn title(id: u32) -> String {
    format!("{TITLE_PREFIX}#{id}")
}

/// The egui viewport that hosts note `id`.
///
/// One function for both sides on purpose: the controller declares the viewport,
/// and the island's key and Esc callbacks have to wake *that* viewport. egui's
/// `request_repaint()` means "the viewport that ran the last pass", which with two
/// notes is a coin flip -- the shortcut would sit in its slot until the note's own
/// 500 ms tick, which is the three-press bug in a new costume.
pub fn viewport_id(id: u32) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("note", id))
}

/// How many note windows this process has open right now.
///
/// `top_level_windows()` also returns three windows nobody asked for -- winit's
/// 14×14 event target and the two IME hosts -- so a raw count would move for
/// reasons unrelated to notes and make the Q5 evidence unreadable.
pub fn open_notes() -> usize {
    hub_win::window::top_level_windows()
        .iter()
        .filter(|window| window.title.starts_with(TITLE_PREFIX))
        .count()
}

/// What the controller hands over when it spawns a note.
pub struct Spawn {
    pub id: u32,
    pub spec: NoteSpec,
    /// Resolved by the controller, because the cursor position belongs to the
    /// moment the user asked for a note -- not to the frame the window appears.
    pub placement: NotePlacement,
    pub scale: f32,
    pub settings: Arc<SpikeSettings>,
    pub lite: bool,
    pub log: Arc<Log>,
    /// `None` when the database could not be opened: the note still works, it just
    /// lives in memory, and the save log says so once.
    pub store: Option<Arc<Mutex<Store>>>,
    /// The row to open, for a note that already exists. `None` starts a new one.
    pub existing: Option<NoteRow>,
    /// The resolved shortcut table. Shared *by reference* with the controller and
    /// the workbench: a rebind has to reach the windows that are already open, so
    /// this is a lock rather than a copy taken at spawn time.
    pub shortcuts: Arc<RwLock<Table>>,
    /// C5: the one size every new note opens at, updated when a window is dragged.
    pub size: size::SharedSize,
    /// Where a remembered size is written. The notes read settings once at startup
    /// as a rule (§5.8.1), so this is only ever used by the resize path.
    pub settings_path: Arc<std::path::PathBuf>,
}

pub struct NoteShell {
    id: u32,
    title: String,
    viewport: egui::ViewportId,
    spec: NoteSpec,
    placement: NotePlacement,
    log: Arc<Log>,
    lite: bool,
    hwnd: Option<isize>,
    /// Set once the first handle lookup missed, so a note that can never attach
    /// does not write the same line ten times a second.
    waited: bool,
    strip: StripState,
    palette: Palette,
    appearance: NoteAppearance,
    /// The theme last pushed into the island. `EM_SETCHARFORMAT(SCF_ALL)` reformats
    /// the whole document, so repeating it every frame is not free: it re-runs
    /// shaping on all ten thousand characters and, while it does, the caret is
    /// hidden. Comparing first is what stops a 500 ms refresh from blinking it.
    last_theme: Option<hub_editor::Theme>,
    editor: Option<Editor>,
    escape: Arc<AtomicBool>,
    esc_presses: Arc<AtomicU32>,
    /// Shortcuts pressed in the island's subclass proc or in egui, waiting for the
    /// next frame. See [`keys`] for why they cannot act where they are seen.
    keys: keys::Bus,
    /// Which key means which command, including the user's overrides. Also what
    /// the strip's hover hints are built from.
    shortcuts: Arc<RwLock<Table>>,
    /// Notches from the island's `WM_MOUSEWHEEL`, plus the 50 ms floor shared with
    /// the egui path. See [`wheel`].
    wheel: wheel::Wheel,
    gate: wheel::Gate,
    /// Clicks in the body, reported by the island because egui cannot see them.
    /// Dismisses the tools panel (Q7). See [`click`].
    clicks: click::Clicks,
    /// Q24: a draft newer than the row, waiting for one of three answers.
    recover: recover::State,
    /// Q10: the window follows the cursor while the strip is held. See [`drag`].
    drag: drag::Drag,
    /// Q3: the same, from one of the window's edges. See [`edge`].
    resize: edge::Resize,
    /// Q3: what the island's press hook writes when a press lands on the band.
    edge: Arc<edge::Signal>,
    /// Alpha last handed to the compositor. `None` until the window is attached, so
    /// the first application is never skipped by the "unchanged" guard.
    last_alpha: Option<u8>,
    /// Autosave: debounce, dirty tracking, and the write itself. See [`crate::save`].
    save: save::Pump,
    /// Kept next to the pump rather than inside it: the switcher reads the list
    /// without touching the save state, and a window that had to ask its own
    /// autosave for a query would couple two unrelated lifetimes.
    store: Option<Arc<Mutex<Store>>>,
    /// `Ctrl+K`. While open the island is hidden and this gets the keys.
    switcher: switcher::State,
    switcher_open: bool,
    /// The list the switcher filters, loaded when it opens, not per frame: a
    /// query keystroke must not become a database read.
    items: Vec<switcher::Item>,
    closing: bool,
    last_island: Option<PixelRect>,
    /// Last `pixels_per_point` this note laid out at, for the transition log.
    last_ppp: f32,
    /// C5: watches the client area and remembers the size the user left it at.
    sizer: size::Sizer,
}

impl NoteShell {
    pub fn new(spawn: Spawn) -> Self {
        let appearance = spawn.settings.quick_note.appearance.clone();
        let palette = Palette::from_colors(&appearance.colors);
        // A note opens at the configured alpha (normally 100%). Applying it here as
        // well as in `attach` is not redundant: this is the value the *label* shows
        // on the very first frame, before the OS window exists.
        let strip = StripState {
            opacity: appearance.initial_opacity(),
            ..StripState::default()
        };
        // The id in the log is the database id from here on: a note that cannot be
        // found again by its window ordinal is not persisted, it is a scratch pad.
        let store = spawn.store.clone();
        let sizer = size::Sizer::new(spawn.size.clone(), spawn.settings_path.clone());
        let save = save::Pump::new(spawn.store, spawn.existing.as_ref());
        // Q24: decided here, drawn on the first frame that has a content rectangle.
        let recover = recover::start(&store, spawn.existing.as_ref(), &spawn.log);
        spawn.log.mark(
            0,
            "note_begin",
            &format!(
                "id={} logical={}x{} placement={}x{}+{}+{} scale={:.2} opacity={:.2}",
                spawn.id,
                spawn.spec.width,
                spawn.spec.height,
                spawn.placement.width,
                spawn.placement.height,
                spawn.placement.x,
                spawn.placement.y,
                spawn.scale,
                strip.opacity
            ),
        );
        spawn.log.mark(
            0,
            "note_doc",
            &format!(
                "id={} note_id={} loaded={} store={}",
                spawn.id,
                save.id(),
                spawn.existing.is_some(),
                if save.disabled() { "off" } else { "on" }
            ),
        );

        Self {
            id: spawn.id,
            title: title(spawn.id),
            viewport: viewport_id(spawn.id),
            spec: spawn.spec,
            placement: spawn.placement,
            log: spawn.log,
            lite: spawn.lite,
            hwnd: None,
            waited: false,
            strip,
            palette,
            appearance,
            last_theme: None,
            editor: None,
            escape: Arc::new(AtomicBool::new(false)),
            esc_presses: Arc::new(AtomicU32::new(0)),
            keys: keys::Bus::new(viewport_id(spawn.id)),
            shortcuts: spawn.shortcuts,
            wheel: wheel::Wheel::new(viewport_id(spawn.id)),
            gate: wheel::Gate::default(),
            clicks: click::Clicks::new(),
            drag: drag::Drag::default(),
            resize: edge::Resize::default(),
            edge: Arc::new(edge::Signal::new(viewport_id(spawn.id), spawn.scale)),
            last_alpha: None,
            save,
            store,
            recover,
            switcher: switcher::State::default(),
            switcher_open: false,
            items: Vec::new(),
            closing: false,
            last_island: None,
            last_ppp: 0.0,
            sizer,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    /// The shortcut table, as a snapshot of this instant.
    fn table(&self) -> RwLockReadGuard<'_, Table> {
        self.shortcuts.read().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn is_closing(&self) -> bool {
        self.closing
    }

    /// True once the OS window was found and placed.
    pub fn attached(&self) -> bool {
        self.hwnd.is_some()
    }

    /// Ask this note to close from outside -- the tray, or a global shortcut.
    ///
    /// The same path as Esc on purpose: one function decides what "close" means, so
    /// a tray command cannot grow a second, slightly different way to destroy a
    /// window (that difference is how "the tray's close leaks a window" happens).
    pub fn request_close(&mut self, ctx: &egui::Context, reason: &str) {
        self.close(ctx, reason);
    }

    /// `Ctrl+Alt+H`, shrink side. False when the window does not exist yet.
    pub fn minimize(&mut self) -> bool {
        self.hwnd.is_some_and(hub_win::window::minimize)
    }

    /// `Ctrl+Alt+H`, bring-back side.
    pub fn restore(&mut self) -> bool {
        self.hwnd.is_some_and(hub_win::window::restore)
    }

    /// Collapsed or not. An unattached note counts as *not* collapsed: it has no
    /// window to have been minimized, and the toggle should not report "everything is
    /// minimized" while a note is still being created.
    pub fn minimized(&self) -> bool {
        self.hwnd.is_some_and(hub_win::window::is_minimized)
    }

    /// Give this note the keyboard (after a restore-all).
    pub fn focus(&mut self) {
        if let Some(hwnd) = self.hwnd {
            hub_win::window::focus(hwnd);
        }
    }

    /// The viewport description, re-applied every pass.
    ///
    /// egui patches a builder by diffing it against the one it recorded, so a
    /// constant builder is a no-op after the window exists -- which is what keeps
    /// a user-resized window from snapping back to `spec` on the next frame.
    pub fn builder(&self) -> egui::ViewportBuilder {
        egui::ViewportBuilder::default()
            .with_title(&self.title)
            .with_inner_size([self.spec.width, self.spec.height])
            .with_min_inner_size([MIN_NOTE_WIDTH, MIN_NOTE_HEIGHT])
            .with_visible(false)
            .with_decorations(false)
            .with_resizable(true)
            .with_taskbar(false)
            .with_always_on_top()
    }

    /// One frame of this note. Called from the deferred viewport callback.
    pub fn frame(&mut self, ctx: &egui::Context) {
        if self.closing {
            return;
        }

        // Alt+F4 and anything else that closes the OS window behind egui's back.
        // Treated as a close request rather than an error: the window is already
        // going away, and logging it is what keeps it distinguishable from Esc.
        if ctx.input(|input| input.viewport().close_requested()) {
            self.close(ctx, "os");
            return;
        }

        if self.hwnd.is_none() && !self.attach() {
            // eframe creates the window before the first callback, so a miss means
            // the lookup raced the creation. Ask again instead of parking the note
            // half-built: without a handle there is no island and no close.
            if !self.waited {
                self.waited = true;
                self.log
                    .mark(0, "note_window", &format!("id={} waiting_for_handle", self.id));
            }
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }

        // Esc arrives through the island's subclass proc, which cannot re-enter
        // window code; the flag defers it to here. The repaint request that comes
        // with it is what makes closing take one press instead of three.
        if self.escape.swap(false, Ordering::SeqCst) {
            let presses = self.esc_presses.swap(0, Ordering::SeqCst);
            self.log
                .mark(0, "note_esc", &format!("id={} presses={presses}", self.id));
            self.close(ctx, "esc");
            return;
        }

        // The switcher owns the keyboard while it is open. `Ctrl+K` is the same
        // key that opened it (Q25), and `Esc` must close *only* the switcher
        // (Q13) -- so while it is open the table below is not consulted at all,
        // rather than consulted and then overruled.
        if self.switcher_open {
            let toggle = keys::egui_press(ctx)
                .map(|press| keys::resolve(&self.table(), press))
                .is_some_and(|command| command == Some(Command::Switcher));
            if toggle {
                self.close_switcher(ctx);
            }
        } else {
            // Shortcuts seen on the egui side. While the island has focus these keys
            // never reach winit, so the two paths cannot both fire for one press.
            if let Some(press) = keys::egui_press(ctx) {
                let table = self.table();
                if let Some(command) = keys::resolve(&table, press) {
                    self.keys.post(command, ctx);
                }
            }
            if let Some(command) = self.keys.take() {
                self.act(ctx, command);
                if self.closing {
                    return;
                }
            }
        }

        // `Ctrl+wheel` (Q28). Two channels, one gate: over the island the message is
        // `WM_MOUSEWHEEL` and arrives at the control's subclass proc (RichEdit would
        // otherwise zoom the text), over the title strip it arrives as an egui
        // scroll event (egui's own Ctrl+wheel zoom is disabled in `Hub::new`).
        // Only one of them can fire for a given notch -- they are different windows.
        let now_ms = ctx.input(|input| (input.time * 1000.0) as u64);
        if let Some(notch) = self.wheel.take() {
            self.nudge(notch, now_ms, wheel::Source::Island);
        } else if let Some(notch) = wheel::egui_notch(ctx) {
            self.nudge(notch, now_ms, wheel::Source::Egui);
        }
        self.apply_alpha("frame");

        // No zoom factor is applied here on purpose. `set_zoom_factor` is global to
        // the `Context`, so one note would rescale every other note's window; with
        // viewports on different monitors that is not a rounding difference, it is
        // the wrong scale everywhere. winit reports the window's own
        // `pixels_per_point` a frame or two after creation (the window is created
        // while still parked on the old monitor), and every rectangle below is
        // recomputed from it each frame, so the island corrects itself.
        let ppp = ctx.pixels_per_point().max(0.1);
        if (ppp - self.last_ppp).abs() > 0.01 {
            self.log.mark(
                0,
                "note_ppp",
                &format!(
                    "id={} ppp={ppp:.2} (was {:.2})",
                    self.id, self.last_ppp
                ),
            );
            self.last_ppp = ppp;
            // The press hook measures in physical pixels, so its band has to follow
            // the scale when the note is dragged to a monitor with another one.
            self.edge.set_scale(ppp);
        }

        // Collapsed windows report a degenerate client area -- measured 190x31 whole
        // window, which turned into `island_rect 237x1` -- and laying the RichEdit out
        // for 1 px reflows it for no one. Skipping the frame keeps the island at its
        // real rectangle, so restoring does not have to rebuild it; the restore path
        // asks for a repaint explicitly.
        if self.minimized() {
            return;
        }

        // Q7, before the strip is drawn: a click in the body dismisses the tools
        // panel, and the panel must be gone in *this* frame. Deferring it to the
        // next autosave tick would make a dismissal take up to 900 ms.
        self.take_body_click();

        // A snapshot, not a guard: the draw call needs `&mut self` for the strip and
        // the switcher, and a read guard held across that is a borrow the compiler
        // is right to refuse. Eleven chords of `String` once per frame is cheaper
        // than the alternative of threading the table through every call.
        let table = self.table().clone();
        let output = quicknote::draw(ctx, &mut self.strip, self.palette, &table);

        if output.actions.drag {
            // Q3 before Q10: a press inside the strip's left/right band belongs to
            // the resize. `press_origin`, not `interact_pos`: the band is where the
            // press *landed*, and the pointer may have moved since.
            let origin = ctx.input(|input| input.pointer.press_origin());
            let resizing = origin
                .is_some_and(|point| self.begin_edge_from_strip(point, ctx.screen_rect().size()));
            if !resizing {
                // Q10: the window follows the cursor here, not in an OS move loop. See
                // `note/drag.rs` for why `ViewportCommand::StartDrag` is not used.
                self.begin_drag();
            }
        }
        self.drag_frame(ctx);
        // The strip half of Q3's cursor hint: over the island the control answers
        // its own `WM_SETCURSOR` (see `note/edge.rs`), but the strip belongs to
        // egui, and a grip that is only discoverable below y = 32 would read as a
        // bug at the top of the edge.
        if !self.resize.active() && !self.switcher_open {
            if let Some(point) = ctx.input(|input| input.pointer.hover_pos()) {
                // Logical on both sides: the strip is laid out in points, and this
                // band is the same `BAND` the press path measures in points.
                let narrow = ctx.screen_rect().width() - edge::BAND;
                if point.x <= edge::BAND || point.x >= narrow {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
            }
        }
        // Q3: the other half -- an edge press that landed on the island, which egui
        // never sees. See `note/edge.rs`.
        self.resize_frame(ctx);
        if output.actions.toggle_pin {
            self.toggle_pin();
        }
        if output.actions.toggle_tools {
            self.toggle_tools("button");
        }

        if self.switcher_open {
            let action = switcher::draw(
                ctx,
                self.palette,
                &mut self.switcher,
                &self.items,
                output.content,
            );
            match action {
                switcher::Action::Close => self.close_switcher(ctx),
                switcher::Action::Pick(id) => {
                    self.close_switcher(ctx);
                    self.switch_to(&id);
                }
                switcher::Action::New(title) => {
                    self.close_switcher(ctx);
                    self.start_new(&title);
                }
                switcher::Action::None => {}
            }
            // The document is untouched while the panel is open, but a debounce
            // armed before it opened still has to fire: the panel must not become
            // a way to postpone a save indefinitely.
            let wait = self.save.tick(
                Instant::now(),
                || self.editor.as_ref().map(|editor| editor.text()).unwrap_or_default(),
                &self.log,
            );
            ctx.request_repaint_after(wait);
            return;
        }

        // The island is a native child window: it must be told the hole's size in
        // physical pixels, or the strip's 32 px would be covered by text. The
        // rectangle is clamped to the OS client area because egui's first frame
        // reports a 10000 px placeholder screen rect; a child window built from
        // that is not "a bit large", it is one the compositor has to clip.
        let (client_w, client_h) = hub_win::window::client_size(self.hwnd.unwrap_or(0));
        // C5: remember the size the user left the window at. See `note/size.rs`.
        self.track_size(ctx, client_w, client_h, ppp);
        let client = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(client_w as f32 / ppp, client_h as f32 / ppp),
        );
        let content = output.content.intersect(client);
        let rect = PixelRect::from_logical(
            [
                content.min.x,
                content.min.y,
                content.width(),
                content.height(),
            ],
            ppp,
        );
        self.sync_editor(ctx, rect);

        // Q24: an unanswered draft owns the frame -- after the editor exists (the
        // card needs something to hide), before the switcher and the save tick.
        if self.recover_frame(ctx, output.content) {
            return;
        }

        // Autosave. The pump decides whether this frame has to read the document at
        // all, and returns when it next needs a look -- asking for a fixed interval
        // either saves late or copies the whole document for nothing.
        let wait = self.save.tick(
            Instant::now(),
            || self.editor.as_ref().map(|editor| editor.text()).unwrap_or_default(),
            &self.log,
        );

        // A shell must not spin: egui repaints on input, and the island paints
        // itself. Without this the note would draw 60 fps of identical frames.
        ctx.request_repaint_after(wait);
    }

    /// Find, place, show and focus this note's OS window -- once.
    fn attach(&mut self) -> bool {
        let Some(hwnd) = hub_win::window::find_by_title(&self.title) else {
            return false;
        };
        let dwm = hub_win::dwm::apply(hwnd, self.lite);
        hub_win::window::place_hidden(
            hwnd,
            self.placement.x,
            self.placement.y,
            self.placement.width,
            self.placement.height,
        );
        // Alpha lands **before** the window is shown, on purpose. The window still
        // exists invisibly at this point (`with_visible(false)` in the builder), and
        // a note configured to open at 40% must not appear at 100% for one frame
        // and then fade -- that is a transparency *jump*, which B3 rules out.
        self.hwnd = Some(hwnd);
        self.apply_alpha("attach");
        hub_win::window::show_without_activation(hwnd);
        // A second note is created while the user is still in the first one, so the
        // foreground has to be moved explicitly. Q19 asks for the caret in the text
        // on open; `SetFocus` on an inactive window fails and the first keystroke
        // would go to the previous note.
        let foreground = hub_win::window::focus(hwnd);
        self.log.mark(
            0,
            "note_window",
            &format!(
                "id={} {}x{}+{}+{} dwm={dwm:?} foreground={foreground}",
                self.id,
                self.placement.width,
                self.placement.height,
                self.placement.x,
                self.placement.y
            ),
        );
        true
    }

    /// One `Ctrl+wheel` notch, after the 50 ms floor.
    fn nudge(&mut self, notch: wheel::Notch, now_ms: u64, from: wheel::Source) {
        if !self.gate.allow(now_ms) {
            self.log.mark(
                0,
                "note_wheel",
                &format!(
                    "id={} notch={notch:?} path={from:?} stepped=false reason=throttle",
                    self.id
                ),
            );
            return;
        }
        let before = self.strip.opacity;
        self.strip.opacity = quicknote::step_opacity(before, notch == wheel::Notch::Up);
        self.log.mark(
            0,
            "note_wheel",
            &format!(
                "id={} notch={notch:?} path={from:?} opacity={:.2} (was {:.2}) label={}",
                self.id,
                self.strip.opacity,
                before,
                quicknote::opacity_label(self.strip.opacity)
            ),
        );
    }

    /// Run one shortcut. Split from the table so the table stays a pure function:
    /// everything that touches a window handle or the control happens here.
    fn act(&mut self, ctx: &egui::Context, command: keys::Command) {
        self.log.mark(
            0,
            "note_key",
            &format!(
                "id={} command={} key={}",
                self.id,
                command.id(),
                self.table().hint(command)
            ),
        );
        match command {
            keys::Command::Switcher => self.open_switcher(ctx),
            keys::Command::CloseNote => self.close(ctx, "shortcut"),
            keys::Command::TogglePin => self.toggle_pin(),
            keys::Command::ToggleTools => self.toggle_tools("shortcut"),
            keys::Command::ToggleGhost => self.toggle_ghost(),
            keys::Command::Undo => self.history(true),
            keys::Command::Redo => self.history(false),
            // The rest are global commands: the table never yields them from a
            // window keystroke, and reaching one here would mean the scope check
            // was skipped. Logged rather than silently ignored.
            other => self.log.mark(
                0,
                "note_key_unhandled",
                &format!("id={} command={}", self.id, other.id()),
            ),
        }
    }

    /// Q5's criterion, walked up to the point where only the OS can finish it.
    ///
    /// `open_notes` here still counts *this* window: the drop happens at the end of
    /// the pass. The controller records the count after it (`note_destroyed`); one
    /// line alone could not distinguish "asked to close" from "closed".
    fn close(&mut self, ctx: &egui::Context, reason: &str) {
        if self.closing {
            return;
        }
        self.closing = true;
        // Before anything else: the debounce is not a reason to drop the last
        // keystroke. Reading first and saving second, because the editor is gone the
        // moment the viewport closes.
        let text = self.editor.as_ref().map(|editor| editor.text());
        if let Some(text) = text {
            self.save.flush(&text, "close", &self.log);
        }
        self.log.mark(
            0,
            "note_close",
            &format!(
                "id={} reason={reason} open_notes={} island={}x{}+{}+{}",
                self.id,
                open_notes(),
                self.last_island.map(|r| r.width).unwrap_or(0),
                self.last_island.map(|r| r.height).unwrap_or(0),
                self.last_island.map(|r| r.x).unwrap_or(0),
                self.last_island.map(|r| r.y).unwrap_or(0),
            ),
        );
        // `send_viewport_cmd_to`, not `send_viewport_cmd`: the latter targets "the
        // viewport of this pass", and `close` is not always called from one. The tray
        // and `Ctrl+Alt+D` close notes from the *controller's* pass, where the current
        // viewport is ROOT -- which would ask eframe to close the hub window, and
        // eframe ends the process when the root closes. Measured 2026-11-25: closing
        // every note from the tray killed the process even though `hub_idle` (a
        // surviving tray) had just been logged.
        //
        // The controller needs one more pass to stop declaring the viewport and log
        // the count after the drop; the repaint below is what produces that pass
        // instead of waiting for a mouse move.
        ctx.send_viewport_cmd_to(self.viewport, egui::ViewportCommand::Close);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
}

/// A note shell the viewport callback can reach.
///
/// `Context::show_viewport_deferred` requires its callback to be `Send + Sync`,
/// so the shell is handed over behind a `Mutex`. Nothing actually crosses threads:
/// eframe calls the callback from the event-loop thread, and every handle inside
/// the shell is thread-affine to it. The `unsafe` impls assert exactly that, and
/// [`Shared::lock`] checks it instead of trusting this comment.
pub struct Shared {
    thread: std::thread::ThreadId,
    shell: Mutex<NoteShell>,
}

// SAFETY: `NoteShell` owns an `HWND` and a RichEdit subclass procedure, both only
// valid from the thread that created them. `Shared` never hands the shell to
// another thread -- `Send + Sync` exists only to satisfy egui's callback bound --
// and `lock()` panics if it is ever called from somewhere else.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    pub fn new(shell: NoteShell) -> Arc<Self> {
        Arc::new(Self {
            thread: std::thread::current().id(),
            shell: Mutex::new(shell),
        })
    }

    pub fn lock(&self) -> MutexGuard<'_, NoteShell> {
        assert_eq!(
            std::thread::current().id(),
            self.thread,
            "a note shell is only valid on the event-loop thread that created its island"
        );
        self.shell.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
