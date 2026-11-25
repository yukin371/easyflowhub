//! The quick-note shell (Phase 1, batch 1a).
//!
//! This is the product window under construction, **not** the spike. The spike
//! measures a reveal path; this draws the chrome a user will actually see, and the
//! two are kept apart so that "the title strip made the p95 worse" is a question
//! with an answer instead of a guess. The spike is deleted at T-Gate 0; this file
//! stays.
//!
//! ## Window creation order is copied from S1 on purpose
//!
//! `with_visible(false)` → find the handle → place in **physical** pixels → show
//! without activation. That is the order the 48.9 ms p95 was measured in. Batch 1a
//! changes content only, so if a reveal ever looks slower the window path is not
//! the suspect.
//!
//! ## What "close" means here
//!
//! There is no tray yet, so closing the window ends the process (decision D7 in
//! [the batch plan](../../../docs/plans/2026-11-25-phase1-quicknote-shell.md)).
//! The log line already carries `windows_before/after` so batch 1b can switch to
//! "destroy the viewport, keep the process" without touching the criterion.

use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use hub_core::{place_note, NoteAppearance, NoteSpec, PixelRect, SpikeSettings, STRIP_HEIGHT};
use hub_editor::Editor;
use hub_ui::quicknote::{self, StripState};
use hub_ui::Palette;

use crate::esc;
use crate::keys;
use crate::log::Log;

pub const NOTE_TITLE: &str = "easyflowhub-note";

pub struct Config {
    pub settings: SpikeSettings,
    pub scale: f32,
    pub lite: bool,
    pub log: Arc<Log>,
}

/// Open the note shell and run until it is closed.
pub fn run(config: Config) -> ! {
    let spec = NoteSpec {
        width: config.settings.quick_note.width as f32,
        height: config.settings.quick_note.height as f32,
        margin: config.settings.margin.max(0),
    };

    config.log.mark(
        0,
        "note_start",
        &format!(
            "logical={}x{} min={}x{} strip={} lite={} scale={:.2}",
            spec.width,
            spec.height,
            hub_core::MIN_NOTE_WIDTH,
            hub_core::MIN_NOTE_HEIGHT,
            STRIP_HEIGHT,
            config.lite,
            config.scale
        ),
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(NOTE_TITLE)
            .with_inner_size([spec.width, spec.height])
            .with_min_inner_size([hub_core::MIN_NOTE_WIDTH, hub_core::MIN_NOTE_HEIGHT])
            .with_visible(false)
            .with_decorations(false)
            .with_resizable(true)
            .with_taskbar(false)
            .with_always_on_top(),
        vsync: true,
        ..Default::default()
    };

    let result = eframe::run_native(
        NOTE_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(NoteShell::new(cc, config, spec)))),
    );

    match result {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

struct NoteShell {
    log: Arc<Log>,
    hwnd: isize,
    strip: StripState,
    palette: Palette,
    appearance: NoteAppearance,
    /// The theme last pushed into the island. `EM_SETCHARFORMAT(SCF_ALL)` reformats
    /// the whole document, so repeating it every frame is not free: it re-runs
    /// shaping on all ten thousand characters and, while it does, the caret is
    /// hidden. Comparing first is what stops a 500 ms refresh from blinking it.
    last_theme: Option<hub_editor::Theme>,
    editor: Option<Editor>,
    escape: std::sync::Arc<std::sync::atomic::AtomicBool>,
    esc_presses: Arc<AtomicU32>,
    /// Shortcuts pressed in the island's subclass proc or in egui, waiting for the
    /// next `update`. See [`keys`] for why they cannot act where they are seen.
    keys: keys::Bus,
    closing: bool,
    last_island: Option<PixelRect>,
    ppp_done: bool,
    scale: f32,
}

impl NoteShell {
    fn new(cc: &eframe::CreationContext<'_>, config: Config, spec: NoteSpec) -> Self {
        let ctx = cc.egui_ctx.clone();
        let appearance = config.settings.quick_note.appearance.clone();
        let palette = Palette::from_colors(&appearance.colors);

        match hub_ui::theme::install_fonts(&ctx) {
            Some(font) => config.log.mark(0, "fonts", &format!("cjk={font}")),
            None => config
                .log
                .mark(0, "fonts", "no CJK font found; text renders as tofu"),
        }

        let hwnd = match hub_win::window::find_by_title(NOTE_TITLE) {
            Some(handle) => handle,
            None => {
                config.log.mark(0, "note_fatal", "window handle not found");
                std::process::exit(1);
            }
        };
        let dwm = hub_win::dwm::apply(hwnd, config.lite);
        config.log.mark(0, "dwm", &format!("applied={dwm:?}"));

        // Place before the first frame: a reveal must never have to catch up with
        // a resize, and a window that appears at 0,0 for one frame is worse than
        // one that appears 100 ms late.
        let cursor = hub_win::cursor_pos().unwrap_or((0, 0));
        let (work, monitor_scale) = {
            let cache = hub_win::MonitorCache::refresh();
            let monitor = cache.at(cursor.0, cursor.1);
            (
                hub_core::Rect::new(
                    monitor.work.0,
                    monitor.work.1,
                    monitor.work.2,
                    monitor.work.3,
                ),
                monitor.scale,
            )
        };
        let placement = place_note(cursor, work, spec, monitor_scale);
        hub_win::window::place_hidden(
            hwnd,
            placement.x,
            placement.y,
            placement.width,
            placement.height,
        );
        hub_win::window::show_without_activation(hwnd);
        config.log.mark(
            0,
            "note_window",
            &format!(
                "{}x{}+{}+{} cursor={},{} monitor_scale={monitor_scale:.2} work={}x{} min={}x{}",
                placement.width,
                placement.height,
                placement.x,
                placement.y,
                cursor.0,
                cursor.1,
                work.width(),
                work.height(),
                (hub_core::MIN_NOTE_WIDTH * monitor_scale).round() as i32,
                (hub_core::MIN_NOTE_HEIGHT * monitor_scale).round() as i32,
            ),
        );

        // The zoom factor is applied on the first frame, not here: at creation
        // `pixels_per_point` still reports the system default (1.0), so dividing
        // by it would ask for 1.5 on top of a monitor that is already 1.5 and the
        // island would be laid out at 2.25 -- measured 2026-11-25 as a 15000 px
        // child window. `update` sees the real native scale.

        Self {
            log: config.log,
            hwnd,
            strip: StripState::default(),
            palette,
            appearance,
            last_theme: None,
            editor: None,
            escape: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            esc_presses: Arc::new(AtomicU32::new(0)),
            keys: keys::Bus::new(),
            closing: false,
            last_island: None,
            ppp_done: false,
            scale: monitor_scale,
        }
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

        match self.editor.as_mut() {
            None => match Editor::create(self.hwnd, rect, &theme) {
                Ok(mut editor) => {
                    editor.on_escape(esc::handler(
                        self.escape.clone(),
                        self.esc_presses.clone(),
                        ctx.clone(),
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
                        &format!("focused={} caret={:?}", editor.is_focused(), editor.caret()),
                    );
                    self.last_theme = Some(theme.clone());
                    self.log.mark(
                        0,
                        "island_created",
                        &format!(
                            "rect={}x{}+{}+{} ppp={:.2} pt={} pad={}x{} caret={}",
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
                        &format!("{}x{}+{}+{}", rect.width, rect.height, rect.x, rect.y),
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
            .mark(0, "note_key", &format!("action={action:?}"));
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
        self.log
            .mark(0, "note_pin", &format!("pinned={}", self.strip.pinned));
    }

    fn toggle_tools(&mut self) {
        self.strip.tools_open = !self.strip.tools_open;
        self.log.mark(
            0,
            "note_tools",
            &format!("open={}", self.strip.tools_open),
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
                "{} can={available} len={}",
                if undo { "undo" } else { "redo" },
                editor.text().chars().count()
            ),
        );
    }

    /// Q5's criterion, logged from the first day it can be observed.
    fn close(&mut self, ctx: &egui::Context, reason: &str) {        if self.closing {
            return;
        }
        self.closing = true;
        let before = hub_win::window::top_level_windows().len();
        self.log.mark(
            0,
            "note_close",
            &format!(
                "reason={reason} windows_before={before} windows_after={} island={}x{}+{}+{}",
                before.saturating_sub(1),
                self.last_island.map(|r| r.width).unwrap_or(0),
                self.last_island.map(|r| r.height).unwrap_or(0),
                self.last_island.map(|r| r.x).unwrap_or(0),
                self.last_island.map(|r| r.y).unwrap_or(0),
            ),
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for NoteShell {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let color = self.palette.panel;
        [
            color.r() as f32 / 255.0,
            color.g() as f32 / 255.0,
            color.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.closing {
            return;
        }

        // Esc arrives through the island's subclass proc, which cannot re-enter
        // window code; the flag defers it to here. The repaint request that comes
        // with it is what makes closing take one press instead of three.
        if self
            .escape
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            let presses = self
                .esc_presses
                .swap(0, std::sync::atomic::Ordering::SeqCst);
            self.log
                .mark(0, "note_esc", &format!("presses={presses}"));
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

        if !self.ppp_done {
            let native = ctx.pixels_per_point();
            if (native - self.scale).abs() > 0.01 {
                ctx.set_zoom_factor(self.scale / native);
            }
            self.log.mark(
                0,
                "set_ppp",
                &format!(
                    "native_ppp={native:.2} target={:.2} effective={:.2}",
                    self.scale,
                    ctx.pixels_per_point()
                ),
            );
            self.ppp_done = true;
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
        let ppp = ctx.pixels_per_point().max(0.1);
        let (client_w, client_h) = hub_win::window::client_size(self.hwnd);
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
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

/// egui stores RGB; Windows colour values are `0x00BBGGRR`.
fn colorref(color: egui::Color32) -> u32 {
    color.r() as u32 | (color.g() as u32) << 8 | (color.b() as u32) << 16
}
