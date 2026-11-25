//! The spike application: warm-up, reveal state machine, bench driver.
//!
//! ## Where the reveal actually happens
//!
//! The hotkey thread positions and shows the window itself, then asks the UI thread
//! to refresh the content. That split is the whole point of the design (§5.8:
//! "按键时无可做事"): `SetWindowPos` on an already-created, already-rendered window
//! costs a few hundred microseconds, while waiting for the render loop to wake costs
//! tens of milliseconds — measured at 30–47 ms on this machine before the split.
//!
//! So there are two timestamps per reveal, and they answer different questions:
//!
//! * `shown`   — hotkey_down → window moved on screen. **This is the gate.** What it
//!   shows is the previously presented frame, which after the warm frames is the
//!   same panel; §5.8 calls this "首帧感知 ≈ 0" via the cached frame.
//! * `refresh` — hotkey_down → the repaint caused by this reveal finished. It bounds
//!   how stale the visible content can be, and is reported rather than gated, because
//!   it depends on when the UI thread happens to run.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hub_core::{place, PanelSpec, PixelRect, Placement, Rect, SpikeSettings};
use hub_editor::Editor;
use hub_ui::PanelState;

use crate::{bench, editor_bench, log::Log};

pub const WINDOW_TITLE: &str = "easyflowhub-native-spike";

/// Frames drawn before the window is ever shown.
///
/// Three, not one: the first compiles shaders and allocates the atlas, the second
/// uploads the glyphs touched by the first, and the third is what a real reveal will
/// look like. Measuring after one warm frame would charge a reveal with work that
/// only ever happens once per process.
const WARM_FRAMES: u32 = 3;

#[derive(Debug, Clone, Copy)]
pub struct RevealRequest {
    /// `Instant` of the hotkey message, taken on the hotkey thread.
    pub at: Instant,
    /// Hotkey_down → window on screen, measured on the hotkey thread.
    pub shown_us: u64,
    pub placement: Placement,
    pub seq: u64,
}

/// Where the hotkey thread hands a reveal to the UI thread.
pub struct Bus {
    pending: Mutex<Option<RevealRequest>>,
    egui: Mutex<Option<egui::Context>>,
    seq: AtomicU64,
}

impl Bus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            pending: Mutex::new(None),
            egui: Mutex::new(None),
            seq: AtomicU64::new(0),
        })
    }

    pub fn attach(&self, ctx: egui::Context) {
        *self.egui.lock().unwrap() = Some(ctx);
    }

    pub fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Called from the hotkey thread after the window is already on screen.
    ///
    /// Order matters: publish first, then wake, otherwise the frame the wake causes
    /// sees nothing and the refresh waits for the following repaint.
    pub fn push(&self, request: RevealRequest) {
        *self.pending.lock().unwrap() = Some(request);
        if let Some(ctx) = self.egui.lock().unwrap().as_ref() {
            ctx.request_repaint();
        }
    }

    pub fn take(&self) -> Option<RevealRequest> {
        self.pending.lock().unwrap().take()
    }

    pub fn has_pending(&self) -> bool {
        self.pending.lock().unwrap().is_some()
    }
}

/// Everything decided before the event loop starts.
pub struct Config {
    pub settings: SpikeSettings,
    pub spec: PanelSpec,
    pub scale: f32,
    pub lite: bool,
    pub monitors: Arc<Mutex<hub_win::MonitorCache>>,
    pub bus: Arc<Bus>,
    pub log: Arc<Log>,
    pub budget_p95_ms: f64,
    pub budget_p99_ms: f64,
    /// Diagnostic only: skip the CJK font to attribute its memory cost.
    pub skip_font: bool,
    /// Show the panel once after warm-up and keep it open.
    pub demo: bool,
    /// See `Args::burst`.
    pub burst: usize,
    /// Draw the text island instead of the action grid (S2).
    pub editor: bool,
    /// Type N timed characters into the island and exit.
    pub editor_bench: Option<usize>,
    /// S2 gate: p95 / p99 for "keystroke → character painted".
    pub editor_budget_p95_ms: f64,
    pub editor_budget_p99_ms: f64,
}

#[derive(Debug, Clone, Copy)]
enum Phase {
    Parked,
    /// Window is on screen; `frame` is the update index that painted its content.
    Shown { request: RevealRequest, frame: u64 },
}

pub struct SpikeApp {
    config: Config,
    hotkey: Arc<hub_win::Hotkey>,
    panel: PanelState,
    hwnd: isize,
    frames: u64,
    warm_left: u32,
    phase: Phase,
    samples: bench::Samples,
    bench_total: Option<usize>,
    completed: usize,
    last_cycle: Option<Instant>,
    cycle_interval: Duration,
    prewarm_done: bool,
    /// Footprint captured right after construction, for the leak delta.
    start_probe: hub_win::ProcessProbe,
    demo_pending: bool,
    burst_pending: usize,
    /// Physical size of the window, so a size change is logged instead of silently
    /// costing the first frame a resize.
    window_size: (i32, i32),
    /// The RichEdit island, created on the first drawn frame.
    editor: Option<Editor>,
    /// Set by the island's subclass proc when Esc is pressed; consumed by `update`
    /// so the window code is never re-entered from inside the message handler.
    escape: Arc<std::sync::atomic::AtomicBool>,
    editor_bench: Option<usize>,
}

impl SpikeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config, bench_total: Option<usize>) -> Self {
        let ctx = cc.egui_ctx.clone();

        if config.skip_font {
            config.log.mark(0, "fonts", "skipped (--no-font)");
        } else {
            match hub_ui::theme::install_fonts(&ctx) {
                Some(font) => config.log.mark(0, "fonts", &format!("cjk={font}")),
                None => config
                    .log
                    .mark(0, "fonts", "no CJK font found; panel renders tofu"),
            }
        }


        let hwnd = match hub_win::window::find_by_title(WINDOW_TITLE) {
            Some(handle) => {
                let applied = hub_win::dwm::apply(handle, config.lite);
                config.log.mark(0, "dwm", &format!("applied={applied:?}"));
                handle
            }
            None => {
                config.log.mark(0, "dwm", "window handle not found");
                std::process::exit(1);
            }
        };

        // Position and size the window now; on reveal only the position may change,
        // so the first visible frame never has to catch up with a resize.
        let cursor = hub_win::cursor_pos().unwrap_or((0, 0));
        let (work, monitor_scale) = {
            let monitors = config.monitors.lock().unwrap();
            let monitor = monitors.at(cursor.0, cursor.1);
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
        let placement = place(cursor, work, &config.spec);
        hub_win::window::place_hidden(
            hwnd,
            placement.x,
            placement.y,
            placement.width,
            placement.height,
        );
        // Visible but off-screen: a hidden window receives no redraws, which would
        // starve both the warm frames and every later content refresh.
        hub_win::window::show_without_activation(hwnd);
        hub_win::window::park(hwnd);

        config.log.mark(
            0,
            "hidden_window",
            &format!(
                "{}x{}+{}+{} cols={} monitor_scale={monitor_scale:.2} work={}x{}",
                placement.width,
                placement.height,
                placement.x,
                placement.y,
                placement.columns,
                work.width(),
                work.height()
            ),
        );

        let hotkey = Self::start_hotkey(&config, hwnd);
        config.log.mark(
            0,
            "hotkey",
            &format!("{} registered", config.settings.hotkey),
        );

        let visible_ms = config.settings.bench_visible_ms.max(80);

        Self {
            panel: PanelState::new(placement.columns, config.lite),
            hotkey: Arc::new(hotkey),
            hwnd,
            frames: 0,
            warm_left: WARM_FRAMES,
            phase: Phase::Parked,
            samples: bench::Samples::default(),
            bench_total,
            completed: 0,
            last_cycle: None,
            cycle_interval: Duration::from_millis(visible_ms + 160),
            prewarm_done: false,
            start_probe: hub_win::ProcessProbe::capture(),
            demo_pending: config.demo,
            burst_pending: config.burst,
            window_size: (placement.width, placement.height),
            editor: None,
            escape: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            editor_bench: config.editor_bench,
            config,
        }
    }

    /// The reveal path, minus the paint.
    ///
    /// Runs on the hotkey thread. It does exactly four things — read the cursor, hit
    /// the cached monitor layout, compute the clamped rectangle, move the window — and
    /// none of them waits on the UI thread.
    fn start_hotkey(config: &Config, hwnd: isize) -> hub_win::Hotkey {
        let bus = config.bus.clone();
        let log = config.log.clone();
        let monitors = config.monitors.clone();
        let spec = config.spec;

        let on_press: hub_win::PressHandler = Arc::new(move || {
            let at = Instant::now();
            let seq = bus.next_seq();
            let cursor = hub_win::cursor_pos().unwrap_or((0, 0));
            let (work, monitor_scale) = {
                let cache = monitors.lock().unwrap();
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
            let placement = place(cursor, work, &spec);

            // The window already holds a fully drawn panel, so this single call is
            // what the user perceives as "it appeared".
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

            log.mark(
                seq,
                "reveal_shown",
                &format!(
                    "shown_ms={:.2} move_ms={:.2} focus_ms={:.2} focused={focused} cursor={},{} monitor_scale={monitor_scale:.2} at=+{}+{} size={}x{} cols={}",
                    shown_us as f64 / 1000.0,
                    moved_us as f64 / 1000.0,
                    (shown_us - moved_us) as f64 / 1000.0,
                    cursor.0,
                    cursor.1,
                    placement.x,
                    placement.y,
                    placement.width,
                    placement.height,
                    placement.columns
                ),
            );

            bus.push(RevealRequest {
                at,
                shown_us,
                placement,
                seq,
            });
        });

        match hub_win::Hotkey::spawn(
            &config.settings.hotkey,
            Duration::from_millis(config.settings.hotkey_debounce_ms),
            on_press,
        ) {
            Ok(hotkey) => hotkey,
            Err(message) => {
                config.log.mark(0, "hotkey_failed", &message);
                eprintln!("{message}");
                std::process::exit(3);
            }
        }
    }

    fn draw_panel(&mut self, ctx: &egui::Context) {
        if !self.config.editor {
            hub_ui::panel::draw(ctx, &mut self.panel);
            return;
        }

        let island = hub_ui::panel::draw_editor(ctx, &mut self.panel);
        let rect = PixelRect::from_logical(
            [island.min.x, island.min.y, island.width(), island.height()],
            ctx.pixels_per_point(),
        );
        self.sync_editor(ctx, rect);
    }

    /// Create the island on first use, then only move it when the rect changed.
    ///
    /// The child window lives in the parent's client area, so the panel's own
    /// position never has to be forwarded here - only the layout inside it.
    fn sync_editor(&mut self, ctx: &egui::Context, rect: PixelRect) {
        let palette = self.panel.palette;
        let theme = hub_editor::Theme {
            background: colorref(palette.panel_strong),
            foreground: colorref(palette.ink),
            font_face: "Microsoft YaHei UI",
            font_pt: 11.0 * ctx.pixels_per_point(),
        };

        match self.editor.as_mut() {
            None => match Editor::create(self.hwnd, rect, &theme) {
                Ok(mut editor) => {
                    let escape = self.escape.clone();
                    editor.on_escape(Box::new(move || {
                        escape.store(true, std::sync::atomic::Ordering::SeqCst);
                    }));
                    editor.set_text(&editor_bench::corpus_text(120));
                    editor.caret_to_end();
                    // Focus is not cosmetic: `EM_SCROLLCARET` is a no-op while the
                    // control lacks it, so without this the caret stops following
                    // typed text at the bottom of the visible area. The panel was
                    // just opened by a keystroke, so the editor is where input
                    // belongs.
                    editor.focus();
                    self.config.log.mark(
                        0,
                        "editor_created",
                        &format!(
                            "class={} rect={}x{}+{}+{} limit={}",
                            editor.class_name(),
                            rect.width,
                            rect.height,
                            rect.x,
                            rect.y,
                            editor.limit()
                        ),
                    );
                    self.editor = Some(editor);
                }
                Err(error) => {
                    self.config.log.mark(0, "editor_failed", &error);
                    self.config.editor = false;
                }
            },
            Some(editor) => {
                editor.apply_theme(&theme);
                if editor.set_rect(rect) {
                    self.config.log.mark(
                        0,
                        "editor_rect",
                        &format!("{}x{}+{}+{}", rect.width, rect.height, rect.x, rect.y),
                    );
                }
            }
        }
    }

    /// S2 evidence run: identity, pixels, scroll, undo, then two typed distributions.
    fn run_editor_bench(&mut self, samples: usize) -> ! {
        let Some(editor) = self.editor.as_mut() else {
            eprintln!("editor bench: island was not created");
            std::process::exit(1);
        };

        let mut evidence = editor_bench::capture(editor);
        evidence.burst_ms = hub_editor::measure::burst_chars(
            editor.hwnd(),
            &editor_bench::corpus_text(200),
            200,
        ) as f64
            / 1000.0;
        let burst_chars = editor.char_count();

        let one_k = editor_bench::type_bench(
            editor,
            1_000,
            samples,
            std::time::Duration::from_millis(60),
        );
        // Half the samples at 10k: the question is "does it degrade", and 100 is
        // already enough to separate a flat curve from a linear one.
        let ten_k = editor_bench::type_bench(
            editor,
            10_000,
            samples / 2,
            std::time::Duration::from_millis(60),
        );

        print!(
            "{}---EDITOR-JSON---\n{}\n",
            editor_bench::report(&evidence, &one_k, &ten_k),
            editor_bench::report_json(
                &evidence,
                &one_k,
                &ten_k,
                self.config.editor_budget_p95_ms,
                self.config.editor_budget_p99_ms,
            )
        );
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }

        self.config.log.mark(
            0,
            "editor_bench_done",
            &format!(
                "chars_after_burst={burst_chars} type_p95_ms={:.2} ten_k_p95_ms={:.2}",
                one_k.p95_ms(),
                ten_k.p95_ms()
            ),
        );

        let ok = one_k.within(
            self.config.editor_budget_p95_ms,
            self.config.editor_budget_p99_ms,
        ) && ten_k.p95_ms() <= one_k.p95_ms().max(0.1) * 2.0;
        std::process::exit(if ok { 0 } else { 2 });
    }

    /// Repaint the panel for a request that is already on screen.
    fn refresh(&mut self, request: RevealRequest, ctx: &egui::Context) {
        let placement = request.placement;

        if self.window_size != (placement.width, placement.height) {
            self.config.log.mark(
                request.seq,
                "resize_on_reveal",
                &format!(
                    "{}x{} -> {}x{}",
                    self.window_size.0, self.window_size.1, placement.width, placement.height
                ),
            );
            self.window_size = (placement.width, placement.height);
        }

        self.panel.columns = placement.columns;
        self.panel.reveal_count = request.seq;
        self.config.log.mark(
            request.seq,
            "reveal_show",
            &format!(
                "ppp={:.2} wake_ms={:.2}",
                ctx.pixels_per_point(),
                request.at.elapsed().as_secs_f64() * 1000.0 - request.shown_us as f64 / 1000.0
            ),
        );

        self.draw_panel(ctx);
        self.phase = Phase::Shown {
            request,
            frame: self.frames,
        };
        ctx.request_repaint();
    }

    /// The update after a refresh: the first frame has been painted and swapped.
    fn settle(&mut self, request: RevealRequest, ctx: &egui::Context) {
        let refresh_us = request.at.elapsed().as_micros() as u64;
        self.samples.push(request.shown_us, refresh_us);
        self.config.log.mark(
            request.seq,
            "first_frame",
            &format!(
                "shown_ms={:.2} refresh_ms={:.2}",
                request.shown_us as f64 / 1000.0,
                refresh_us as f64 / 1000.0
            ),
        );

        if self.bench_total.is_some() {
            hub_win::window::park(self.hwnd);
            self.phase = Phase::Parked;
            self.last_cycle = Some(Instant::now());
            self.completed += 1;

            if let Some(total) = self.bench_total {
                if self.completed >= total {
                    self.finish();
                }
            }
            ctx.request_repaint_after(self.cycle_interval);
        } else {
            self.phase = Phase::Shown {
                request,
                frame: self.frames,
            };
        }
    }

    fn maybe_start_cycle(&mut self, ctx: &egui::Context) {
        let Some(total) = self.bench_total else {
            return;
        };
        if self.completed >= total {
            return;
        }
        if let Some(last) = self.last_cycle {
            let elapsed = last.elapsed();
            if elapsed < self.cycle_interval {
                ctx.request_repaint_after(self.cycle_interval - elapsed);
                return;
            }
        }
        self.hotkey.post_test_press();
    }

    fn finish(&mut self) -> ! {
        let report = bench::report(
            &self.samples,
            self.config.budget_p95_ms,
            self.config.budget_p99_ms,
        );
        let json = bench::report_json(
            &self.samples,
            self.config.budget_p95_ms,
            self.config.budget_p99_ms,
            None,
        );
        let end_probe = hub_win::ProcessProbe::capture();
        let leak = format!(
            "{{\"kind\":\"native-s1-leak\",\"cycles\":{},\"start_private_mb\":{:.1},\"end_private_mb\":{:.1},\"private_delta_mb\":{:.1},\"start_handles\":{},\"end_handles\":{},\"handles_delta\":{}}}",
            self.samples.shown_us.len(),
            self.start_probe.private_mb(),
            end_probe.private_mb(),
            end_probe.private_mb() - self.start_probe.private_mb(),
            self.start_probe.handles,
            end_probe.handles,
            end_probe.handles as i64 - self.start_probe.handles as i64
        );
        self.config.log.mark(0, "bench_done", "");
        print!("{report}---JSON---\n{json}---LEAK---\n{leak}\n");
        {
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }

        let shown = bench::distribution(&self.samples.shown_us);
        let ok = shown.p95 as f64 / 1000.0 <= self.config.budget_p95_ms
            && shown.p99 as f64 / 1000.0 <= self.config.budget_p99_ms;
        std::process::exit(if ok { 0 } else { 2 });
    }
}

impl eframe::App for SpikeApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frames += 1;

        // Esc is captured by the island's subclass proc, which must not re-enter
        // window code from inside a message handler; the flag defers it here.
        if self
            .escape
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            hub_win::window::park(self.hwnd);
            self.phase = Phase::Parked;
            self.config.log.mark(0, "editor_esc", "panel parked");
            return;
        }

        if !self.prewarm_done {
            // §5.1.1: the scale must be right before the window is ever shown,
            // otherwise the first visible frame is drawn at the wrong size and then
            // snaps. `set_pixels_per_point` takes an *effective* value, so calling it
            // with the monitor scale double-applies it (1.5 native x 1.5 = 2.25);
            // express it as a zoom factor relative to the native scale instead.
            let native = ctx.pixels_per_point();
            if (native - self.config.scale).abs() > 0.01 {
                ctx.set_zoom_factor(self.config.scale / native);
            }
            self.config.log.mark(
                0,
                "set_ppp",
                &format!(
                    "native_ppp={native:.2} target={:.2} effective={:.2} before_show=1",
                    self.config.scale,
                    ctx.pixels_per_point()
                ),
            );
            hub_ui::panel::prewarm(ctx);
            self.config
                .log
                .mark(0, "prewarm", "first-screen glyphs laid out");
            self.prewarm_done = true;
        }

        // A press while the panel is open closes it (interactive mode).
        if matches!(self.phase, Phase::Shown { .. }) && self.config.bus.has_pending() {
            let _ = self.config.bus.take();
            hub_win::window::park(self.hwnd);
            self.phase = Phase::Parked;
            self.config.log.mark(0, "toggle_hide", "");
            return;
        }

        if let Phase::Shown { request, frame } = self.phase {
            if self.frames > frame {
                self.settle(request, ctx);
                return;
            }
        }

        if let Some(request) = self.config.bus.take() {
            self.refresh(request, ctx);
            return;
        }

        if self.warm_left > 0 {
            self.warm_left -= 1;
            self.draw_panel(ctx);
            ctx.request_repaint();
            if self.warm_left == 0 {
                // Baseline for the leak delta must be taken *after* the warm frames:
                // shaders, atlas and the first textures are one-off costs, and
                // charging them to cycle 1 would turn a healthy run into a false leak.
                self.start_probe = hub_win::ProcessProbe::capture();
                self.config.log.mark(
                    0,
                    "warm_done",
                    &format!(
                        "private_mb={:.1} handles={}",
                        self.start_probe.private_mb(),
                        self.start_probe.handles
                    ),
                );
            }
            return;
        }

        if let Some(samples) = self.editor_bench {
            if self.warm_left == 0 && self.editor.is_some() {
                self.run_editor_bench(samples);
            }
            return;
        }

        if self.burst_pending > 0 && self.warm_left == 0 {
            // Three presses inside one debounce window must produce exactly one
            // reveal. Each press is a separate posted message, exactly like a
            // screenshot tool echoing the same combination.
            let presses = self.burst_pending;
            self.burst_pending = 0;
            self.hotkey
                .post_presses(presses, Duration::from_millis(60));
            return;
        }

        if self.demo_pending && self.warm_left == 0 {
            self.demo_pending = false;
            self.hotkey.post_test_press();
            return;
        }

        self.maybe_start_cycle(ctx);
    }
}

/// egui stores RGB; Windows colour values are `0x00BBGGRR`.
fn colorref(color: egui::Color32) -> u32 {
    color.r() as u32 | (color.g() as u32) << 8 | (color.b() as u32) << 16
}
