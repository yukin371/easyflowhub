//! The hotkey thread's half of the panel: geometry, show, focus, and the doorbell.
//!
//! Split out of `launcher.rs` for the same reason that file was split out of
//! `hub.rs`: the two halves fail differently. This one runs while the user is
//! pressing a key and owns nothing that draws; the other runs inside a frame and owns
//! every pixel. Keeping them in one file made "what is allowed to block here" a
//! question you answered by reading 900 lines instead of the first 200.
//!
//! Nothing in here waits for the UI thread to wake, with one deliberate exception:
//! [`Handle::reveal_when_ready`], which is answering a *forwarded launch* rather than
//! a key press. The difference is in the intent -- a key press is over by the time it
//! is handled, a forwarded launch is a request that stays valid until it is answered.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hub_core::{place, Rect};
use hub_win::MonitorCache;

use crate::hub::Request;
use crate::log::Log;

use super::Shown;

/// The hotkey thread's half: geometry, show, focus.
pub struct Handle {
    spec: hub_core::PanelSpec,
    monitors: Arc<Mutex<MonitorCache>>,
    hwnd: AtomicIsize,
    visible: AtomicBool,
    epoch: AtomicU64,
    /// What the last reveal took from the application underneath.
    ///
    /// Held here rather than in the request because the capture happens on this
    /// thread, between showing the window and focusing it, and the UI thread asks
    /// for it while it handles the show. Keeping it on `Handle` also means the
    /// only writer is the code that did the capture.
    selection: Mutex<Option<hub_win::selection::Capture>>,
    /// The window that had the focus when the panel was summoned.
    ///
    /// Not the same as `selection.target`: that one only exists when text was
    /// captured, and the keyboard has to go back to the application either way.
    /// Without this, dismissing the panel leaves the foreground on a parked window,
    /// so the user's next keystroke goes nowhere -- the panel is gone and nothing
    /// else has the keyboard.
    previous: AtomicIsize,
    /// The control that had the keyboard inside `previous`, if any.
    ///
    /// `park` clears the thread's focus to give the keyboard up, and that is the
    /// same focus a note's editor was using. Putting the window back in front does
    /// not bring the editor's focus back, so the child is remembered too.
    previous_focus: AtomicIsize,
    /// Whether a reveal should try to take the selection. Off for `--launcher-bench`:
    /// a hundred synthetic reveals would drive the clipboard a hundred times and
    /// measure the clipboard rather than the panel.
    capture: bool,
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
        capture: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            spec,
            monitors,
            hwnd: AtomicIsize::new(0),
            visible: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            selection: Mutex::new(None),
            previous: AtomicIsize::new(0),
            previous_focus: AtomicIsize::new(0),
            capture,
            inbox,
            ctx,
            log,
        })
    }

    /// The selection the last reveal took, for the UI thread and for a replace.
    pub fn selection(&self) -> Option<hub_win::selection::Capture> {
        self.selection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// The window that was in front when the panel was summoned, `0` if unknown.
    pub fn previous(&self) -> isize {
        self.previous.load(Ordering::SeqCst)
    }

    /// The control that was typing inside [`Handle::previous`], `0` if unknown.
    pub fn previous_focus(&self) -> isize {
        self.previous_focus.load(Ordering::SeqCst)
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

        if self.visible.swap(false, Ordering::SeqCst) {
            // Hiding needs the UI thread (see the module comment); the epoch rides
            // along so a show that happens first is not undone by this hide.
            let epoch = self.next_epoch();
            self.log.mark(epoch, "launcher_toggle", "action=hide");
            self.push(Request::LauncherHidden { epoch });
            return;
        }

        self.show(hwnd, "hotkey");
    }

    /// Show the panel if it is not already up: what a second launch means.
    ///
    /// Deliberately not `toggle`. A forwarded launch says "I want the thing in front
    /// of me", so a panel already on screen must stay on screen -- toggling would
    /// read as the second double-click having *closed* it. Runs on the watcher
    /// thread, which is a plain thread for the same reason the hotkey one is: the
    /// reveal must not queue behind a frame.
    pub fn reveal(&self) {
        let hwnd = self.hwnd.load(Ordering::SeqCst);
        if hwnd == 0 {
            // The shell was claimed but the panel has no window yet: the frame that
            // creates it has not run. Same line shape as the hotkey path, so a gate
            // reading the log does not need to know which of the two arrived first.
            self.log
                .mark(0, "launcher_toggle", "ready=false reason=no_window");
            return;
        }
        if self.visible.load(Ordering::SeqCst) {
            // Already up. Re-focusing is the honest answer: the user double-clicked
            // because they could not see it, which usually means another window is
            // over it rather than that it is absent.
            let focused = hub_win::window::focus(hwnd);
            self.log
                .mark(0, "launcher_toggle", &format!("action=refocus focused={focused}"));
            return;
        }
        self.show(hwnd, "second-launch");
    }

    /// How long a forwarded launch waits for the panel's window before giving up.
    ///
    /// Only reached on a machine so loaded that a 1×1 window takes three seconds; the
    /// measured wait at login is one or two frames.
    const REVEAL_WAIT: Duration = Duration::from_millis(3000);

    /// Show the panel as soon as it has a window, waiting if it does not yet.
    ///
    /// A forwarded launch is a **queued** request: the click that started the second
    /// process is a request for the panel, and arriving a frame early is not a reason
    /// to drop it. A key press is the opposite -- [`toggle`] answers `ready=false` and
    /// leaves it there, because the user pressed a key *now* and will press it again.
    ///
    /// The wait has to nudge the paint thread. The panel's viewport is created by a
    /// frame, and with nothing on screen the controller goes idle: sleeping here
    /// without asking for a frame would be waiting for something nobody is going to
    /// draw. Returns `None` if the window never appeared inside [`REVEAL_WAIT`].
    pub fn reveal_when_ready(&self) -> Option<Duration> {
        let at = Instant::now();
        loop {
            if self.hwnd.load(Ordering::SeqCst) != 0 {
                self.reveal();
                return Some(at.elapsed());
            }
            if at.elapsed() > Self::REVEAL_WAIT {
                return None;
            }
            self.ctx.request_repaint_of(egui::ViewportId::ROOT);
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn next_epoch(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn show(&self, hwnd: isize, reason: &str) {
        let at = Instant::now();
        let epoch = self.next_epoch();

        // One line per press, on the hotkey thread, before any work. The hide path
        // used to leave no trace at all, so "the key did nothing" and "the key hid
        // the panel" read the same in the log -- and a press that never becomes a
        // `WM_HOTKEY` leaves nothing behind either. `launcher_shown` follows this
        // line by the move's duration, which is where a slow reveal hides.
        self.log
            .mark(epoch, "launcher_toggle", &format!("action=show reason={reason}"));

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

        // Read before the move, not after: once the panel is on screen the answer
        // is allowed to be the panel, and then the window we owe the keyboard to
        // would be the one we are about to take it from.
        let previous = hub_win::window::foreground();
        if previous != 0 && previous != hwnd {
            self.previous.store(previous, Ordering::SeqCst);
            self.previous_focus
                .store(hub_win::window::focused_child(previous), Ordering::SeqCst);
        }

        hub_win::window::move_on_screen(
            hwnd,
            placement.x,
            placement.y,
            placement.width,
            placement.height,
            true,
        );
        let moved_us = at.elapsed().as_micros() as u64;

        // Between the move and the focus, and nowhere else. `SendInput` goes to the
        // foreground window: the app the user was reading still owns it here,
        // because `move_on_screen` shows the panel without activating it. Focus
        // first and the copy would ask the panel for its own (empty) selection.
        //
        // On the reveal path this costs what the copy costs -- 5-15 ms measured --
        // and `shown_ms` therefore includes it. That is the honest number: the
        // panel is not answering the keyboard until the capture returns, so a
        // budget that excluded it would be measuring a panel the user cannot type
        // into yet.
        let capture = if self.capture {
            let outcome = hub_win::selection::capture(hwnd);
            self.log.mark(epoch, "launcher_selection", &outcome.describe());
            match outcome {
                hub_win::selection::Outcome::Captured(capture) => Some(capture),
                hub_win::selection::Outcome::Miss { .. } => None,
            }
        } else {
            None
        };
        *self
            .selection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = capture;

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
