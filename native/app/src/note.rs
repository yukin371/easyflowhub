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
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use hub_core::{
    NoteAppearance, NotePlacement, NoteSpec, PixelRect, SpikeSettings, MIN_NOTE_HEIGHT,
    MIN_NOTE_WIDTH,
};
use hub_editor::Editor;
use hub_ui::quicknote::{self, StripState};
use hub_ui::Palette;

use crate::esc;
use crate::keys;
use crate::log::Log;

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
    closing: bool,
    last_island: Option<PixelRect>,
    /// Last `pixels_per_point` this note laid out at, for the transition log.
    last_ppp: f32,
}

impl NoteShell {
    pub fn new(spawn: Spawn) -> Self {
        let appearance = spawn.settings.quick_note.appearance.clone();
        let palette = Palette::from_colors(&appearance.colors);
        spawn.log.mark(
            0,
            "note_begin",
            &format!(
                "id={} logical={}x{} placement={}x{}+{}+{} scale={:.2}",
                spawn.id,
                spawn.spec.width,
                spawn.spec.height,
                spawn.placement.width,
                spawn.placement.height,
                spawn.placement.x,
                spawn.placement.y,
                spawn.scale
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
            strip: StripState::default(),
            palette,
            appearance,
            last_theme: None,
            editor: None,
            escape: Arc::new(AtomicBool::new(false)),
            esc_presses: Arc::new(AtomicU32::new(0)),
            keys: keys::Bus::new(viewport_id(spawn.id)),
            closing: false,
            last_island: None,
            last_ppp: 0.0,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn is_closing(&self) -> bool {
        self.closing
    }

    /// True once the OS window was found and placed.
    pub fn attached(&self) -> bool {
        self.hwnd.is_some()
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

        // Shortcuts seen on the egui side. While the island has focus these keys
        // never reach winit, so the two paths cannot both fire for one press.
        if let Some(press) = keys::egui_press(ctx) {
            if let Some(action) = keys::resolve(press) {
                self.keys.post(action, ctx);
            }
        }
        if let Some(action) = self.keys.take() {
            self.act(ctx, action);
            if self.closing {
                return;
            }
        }

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
        }

        let output = quicknote::draw(ctx, &mut self.strip, self.palette);

        if output.actions.drag {
            // Handing the drag to winit is what makes the whole strip a title bar
            // without a per-slot hit region: the OS moves the window from here on.
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if output.actions.toggle_pin {
            self.toggle_pin(ctx);
        }
        if output.actions.toggle_tools {
            self.toggle_tools();
        }

        // The island is a native child window: it must be told the hole's size in
        // physical pixels, or the strip's 32 px would be covered by text. The
        // rectangle is clamped to the OS client area because egui's first frame
        // reports a 10000 px placeholder screen rect; a child window built from
        // that is not "a bit large", it is one the compositor has to clip.
        let (client_w, client_h) = hub_win::window::client_size(self.hwnd.unwrap_or(0));
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

        // A shell must not spin: egui repaints on input, and the island paints
        // itself. Without this the note would draw 60 fps of identical frames.
        ctx.request_repaint_after(Duration::from_millis(500));
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
        hub_win::window::show_without_activation(hwnd);
        // A second note is created while the user is still in the first one, so the
        // foreground has to be moved explicitly. Q19 asks for the caret in the text
        // on open; `SetFocus` on an inactive window fails and the first keystroke
        // would go to the previous note.
        let foreground = hub_win::window::focus(hwnd);
        self.hwnd = Some(hwnd);
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

    /// Translate the user's logical appearance into the editor's physical theme.
    ///
    /// `pixels_per_point` is the only place the scale is applied: `font_pt` is in
    /// points and must *not* be multiplied (that bug rendered 16.5 pt at 150%),
    /// while the inset is in logical pixels and must be.
    fn theme_for(&self, ctx: &egui::Context) -> hub_editor::Theme {
        let ppp = ctx.pixels_per_point().max(0.1);
        hub_editor::Theme {
            background: colorref(self.palette.panel),
            foreground: colorref(self.palette.ink),
            font_face: self.appearance.font_family.clone(),
            font_pt: self.appearance.font_pt,
            line_spacing: self.appearance.line_spacing,
            padding: (
                (self.appearance.padding_x as f32 * ppp).round() as i32,
                (self.appearance.padding_y as f32 * ppp).round() as i32,
            ),
            caret_width: (self.appearance.caret_width as f32 * ppp).round() as i32,
        }
    }

    fn sync_editor(&mut self, ctx: &egui::Context, rect: PixelRect) {
        let theme = self.theme_for(ctx);
        let hwnd = self.hwnd.unwrap_or(0);

        match self.editor.as_mut() {
            None => match Editor::create(hwnd, rect, &theme) {
                Ok(mut editor) => {
                    editor.on_escape(esc::handler(
                        self.escape.clone(),
                        self.esc_presses.clone(),
                        ctx.clone(),
                        self.viewport,
                    ));
                    editor.on_key(self.keys.handler(ctx.clone()));
                    // Focus first: `EM_SETSEL`/`EM_SCROLLCARET` are no-ops without it,
                    // so the caret used to be parked at 0 and the first screenful
                    // was the only one that ever scrolled.
                    editor.focus();
                    editor.caret_to_end();
                    editor.scroll_caret();
                    editor.apply_caret_width(theme.caret_width);
                    // Q19's criterion: focused on open, caret at the end. Logged
                    // because "the first keystroke went somewhere else" is otherwise
                    // indistinguishable from a key that was never delivered.
                    self.log.mark(
                        0,
                        "editor_focus",
                        &format!(
                            "id={} focused={} caret={:?}",
                            self.id,
                            editor.is_focused(),
                            editor.caret()
                        ),
                    );
                    self.last_theme = Some(theme.clone());
                    self.log.mark(
                        0,
                        "island_created",
                        &format!(
                            "id={} rect={}x{}+{}+{} ppp={:.2} pt={} pad={}x{} caret={}",
                            self.id,
                            rect.width,
                            rect.height,
                            rect.x,
                            rect.y,
                            ctx.pixels_per_point(),
                            self.appearance.font_pt,
                            theme.padding.0,
                            theme.padding.1,
                            theme.caret_width
                        ),
                    );
                    self.editor = Some(editor);
                }
                Err(error) => self.log.mark(0, "island_failed", &error),
            },
            Some(editor) => {
                if self.last_theme.as_ref() != Some(&theme) {
                    editor.apply_theme(&theme);
                    self.last_theme = Some(theme);
                }
                if editor.set_rect(rect) {
                    self.log.mark(
                        0,
                        "island_rect",
                        &format!(
                            "id={} {}x{}+{}+{}",
                            self.id, rect.width, rect.height, rect.x, rect.y
                        ),
                    );
                }
            }
        }
        self.last_island = Some(rect);
    }

    /// Run one shortcut. Split from the table so the table stays a pure function:
    /// everything that touches a window handle or the control happens here.
    fn act(&mut self, ctx: &egui::Context, action: keys::Action) {
        self.log
            .mark(0, "note_key", &format!("id={} action={action:?}", self.id));
        match action {
            keys::Action::Close => self.close(ctx, "shortcut"),
            keys::Action::TogglePin => self.toggle_pin(ctx),
            keys::Action::ToggleTools => self.toggle_tools(),
            keys::Action::Undo => self.history(true),
            keys::Action::Redo => self.history(false),
        }
    }

    fn toggle_pin(&mut self, ctx: &egui::Context) {
        self.strip.pinned = !self.strip.pinned;
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.strip.pinned {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
        self.log.mark(
            0,
            "note_pin",
            &format!("id={} pinned={}", self.id, self.strip.pinned),
        );
    }

    fn toggle_tools(&mut self) {
        self.strip.tools_open = !self.strip.tools_open;
        self.log.mark(
            0,
            "note_tools",
            &format!("id={} open={}", self.id, self.strip.tools_open),
        );
    }

    /// Undo/redo through the control, not through a shadow history.
    ///
    /// `Ctrl+Z` used to be left to RichEdit; routing it through the table too is
    /// what makes the egui path (focus on the title strip) behave like the island
    /// path. `can=` is logged because a redo with nothing to redo would otherwise
    /// look identical to a working one.
    fn history(&self, undo: bool) {
        let Some(editor) = self.editor.as_ref() else {
            self.log.mark(0, "note_history", "editor=none");
            return;
        };
        let available = if undo {
            editor.can_undo()
        } else {
            editor.can_redo()
        };
        if available {
            if undo {
                editor.undo(1);
            } else {
                editor.redo(1);
            }
        }
        self.log.mark(
            0,
            "note_history",
            &format!(
                "id={} {} can={available} len={}",
                self.id,
                if undo { "undo" } else { "redo" },
                editor.text().chars().count()
            ),
        );
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
        // Close destroys this viewport's window. The controller needs one more pass
        // to stop declaring it and log the count after the drop, and the request
        // below is what produces that pass instead of waiting for a mouse move.
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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

/// egui stores RGB; Windows colour values are `0x00BBGGRR`.
fn colorref(color: egui::Color32) -> u32 {
    color.r() as u32 | (color.g() as u32) << 8 | (color.b() as u32) << 16
}
