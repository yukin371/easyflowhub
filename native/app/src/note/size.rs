//! C5: remember the size the user dragged this window to.
//!
//! One size for every note, not one per window: `settings.quick_note.width/height`
//! is a **global** pair, so resizing one note changes what the *next* note opens
//! at. The per-window variant was ruled out on purpose (parity C5) -- a note is a
//! scratch pad, and "each one remembers its own shape" is a document-manager
//! feature that would need a row per window in the settings table.
//!
//! ## Why not a `WM_SIZE` handler
//!
//! The shell has no window procedure of its own; winit owns it. The size is
//! therefore *observed*, once per frame, from `GetClientRect`. That is also the
//! only measurement that survives being dragged between monitors: the physical
//! size changes with `pixels_per_point` while the logical one does not, and the
//! question C5 asks is about the logical size.
//!
//! ## Why a quiet period
//!
//! Dragging an edge produces a `WM_SIZE` per pixel. Writing the settings file on
//! each would be hundreds of renames for one gesture, and the file is the thing
//! the *next* process reads -- so what matters is where the drag ended, not the
//! path it took. The window has to sit at one size for [`QUIET`] before that size
//! is written, and the frame that measures it asks for one more pass in case the
//! drag ends on a frame that never arrives (no further input means no repaint).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hub_core::{SettingsSource, SpikeSettings};

use crate::log::Log;

/// How long the window has to stay at one size before it is written.
///
/// 400 ms is the same order as the drag's own settle time: shorter and a mouse
/// that pauses mid-gesture writes a size the user is about to leave.
pub const QUIET: Duration = Duration::from_millis(400);

/// Slack for "this is the same size", logical pixels.
///
/// The client size is a physical measurement divided by `pixels_per_point`, and
/// the window was created from a logical spec rounded twice (logical → physical
/// → back). A one-pixel difference is arithmetic, not a drag; without this every
/// launch on a 125% monitor would write a settings file that says `399x299`.
/// `observe` never *stores* a value within the slack of what is already there.
const SLACK: u32 = 2;

fn same(a: (u32, u32), b: (u32, u32)) -> bool {
    a.0.abs_diff(b.0) <= SLACK && a.1.abs_diff(b.1) <= SLACK
}

/// The remembered size, shared with the controller.
///
/// `Arc<Mutex<..>>` rather than a copy: the controller creates the next note from
/// this cell, and the note that resizes is a different object. Same shape as the
/// shortcut table, and for the same reason -- one live value, not one per window.
pub type SharedSize = Arc<Mutex<(u32, u32)>>;

pub fn shared(width: u32, height: u32) -> SharedSize {
    Arc::new(Mutex::new((width, height)))
}

pub struct Sizer {
    shared: SharedSize,
    path: Arc<PathBuf>,
    /// The size this window was created at, learned from the first measurement.
    ///
    /// Learned, not taken from `spec`: `place_note` clamps to the work area and
    /// the OS may round the physical size, so the spec and the client area can
    /// differ by a pixel. Treating that difference as a user resize would rewrite
    /// the settings file on every launch.
    base: Option<(u32, u32)>,
    /// The last size seen and when it was first seen at that value.
    pending: Option<((u32, u32), Instant)>,
    /// What the outside world already holds. `None` until `base` is known.
    written: Option<(u32, u32)>,
}

impl Sizer {
    pub fn new(shared: SharedSize, path: Arc<PathBuf>) -> Self {
        Self {
            shared,
            path,
            base: None,
            pending: None,
            written: None,
        }
    }

    /// Feed one measurement. Returns how long to wait for the next look.
    ///
    /// `logical` is the client size in logical pixels, already divided by the
    /// window's `pixels_per_point`.
    pub fn observe(
        &mut self,
        logical: (u32, u32),
        now: Instant,
        log: &Log,
        id: u32,
    ) -> Option<Duration> {
        let Some(base) = self.base else {
            // First measurement: this *is* the size the window opened at. Nothing
            // to compare against, so nothing can be a change yet.
            self.base = Some(logical);
            self.written = Some(logical);
            return None;
        };

        if same(logical, self.written.unwrap_or(base)) {
            // Back to what is already stored -- either the drag was undone or the
            // settle timer had not expired. Either way a pending write is moot.
            self.pending = None;
            return None;
        }

        match self.pending {
            Some((last, since)) if same(last, logical) => {
                let waited = now.saturating_duration_since(since);
                // The size that gets written is the latest one, not the first in
                // the burst: the quiet period is evidence about the *end* of a
                // drag, and the end is the last measurement.
                self.pending = Some((logical, since));
                if waited >= QUIET {
                    self.pending = None;
                    self.persist(logical, base, log, id);
                    return None;
                }
                // Ask for the frame that performs the write when the quiet period
                // is up; without it a drag that ends in silence would sit
                // unwritten until the note's next autosave tick.
                Some(QUIET - waited)
            }
            _ => {
                self.pending = Some((logical, now));
                Some(QUIET)
            }
        }
    }

    /// Write the size into `settings.quick_note`, leaving every other key alone.
    ///
    /// Read-modify-write the **file**, not the in-memory snapshot: the workbench
    /// may have written a rebind to the same file since this process started, and
    /// saving our stale copy would drop it.
    fn persist(&mut self, logical: (u32, u32), base: (u32, u32), log: &Log, id: u32) {
        let path: &Path = &self.path;
        let existed = path.exists();
        let (mut settings, source) = SpikeSettings::load(path);

        // A file that exists but does not parse loads as defaults, and saving that
        // would replace the user's hand-edited settings with a default document --
        // one resize would then look like "all my settings were reset". Refusing
        // is the honest outcome: the resize still applies to this process.
        if source == SettingsSource::Default && existed {
            self.written = Some(logical);
            log.mark(
                0,
                "note_size",
                &format!(
                    "id={id} logical={}x{} saved=false reason=settings_unreadable path={}",
                    logical.0,
                    logical.1,
                    path.display()
                ),
            );
            return;
        }

        settings.quick_note = settings.quick_note.sized(logical.0, logical.1);
        let remembered = (settings.quick_note.width, settings.quick_note.height);
        match settings.save(path) {
            Ok(()) => {
                self.written = Some(remembered);
                self.store(remembered);
                log.mark(
                    0,
                    "note_size",
                    &format!(
                        "id={id} logical={}x{} from={}x{} saved=true schema={}",
                        remembered.0, remembered.1, base.0, base.1, settings.quick_note.schema
                    ),
                );
            }
            Err(error) => {
                // Not marked as written: the next quiet period at this size (or the
                // next start) retries. The shared cell still moves, because the
                // next window in *this* process can honour it even if the file
                // cannot.
                self.store(remembered);
                log.mark(
                    0,
                    "note_size",
                    &format!(
                        "id={id} logical={}x{} saved=false error=\"{error}\"",
                        remembered.0, remembered.1
                    ),
                );
            }
        }
    }

    fn store(&self, size: (u32, u32)) {
        if let Ok(mut cell) = self.shared.lock() {
            *cell = size;
        }
    }
}

impl super::NoteShell {
    /// Turn this frame's client measurement into a "remember me" decision.
    ///
    /// Lives here rather than in `note.rs` so the shell keeps one call and the
    /// policy -- quiet period, slack, clamping -- stays in one file with its tests.
    pub(super) fn track_size(
        &mut self,
        ctx: &egui::Context,
        client_w: i32,
        client_h: i32,
        ppp: f32,
    ) {
        if client_w <= 0 || client_h <= 0 || !ppp.is_finite() || ppp <= 0.0 {
            return;
        }
        let logical = (
            (client_w as f32 / ppp).round() as u32,
            (client_h as f32 / ppp).round() as u32,
        );
        if let Some(wait) = self
            .sizer
            .observe(logical, Instant::now(), &self.log, self.id)
        {
            ctx.request_repaint_after(wait);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizer(dir: &Path) -> Sizer {
        Sizer::new(shared(400, 300), Arc::new(dir.join("settings.json")))
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hub-note-size-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_first_measurement_is_the_size_the_window_opened_at() {
        // A clamp to the work area or an OS rounding must not read as a resize,
        // or every launch would rewrite the settings file.
        let dir = temp_dir("first");
        let log = Log::new(dir.join("note.log"));
        let mut sizer = sizer(&dir);
        let now = Instant::now();

        assert_eq!(sizer.observe((400, 300), now, &log, 0), None);
        assert_eq!(sizer.observe((399, 299), now, &log, 0), None);
        assert!(
            !dir.join("settings.json").exists(),
            "nothing to write yet: the window has not been resized"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_size_is_written_only_after_it_holds_still() {
        let dir = temp_dir("quiet");
        let log = Log::new(dir.join("note.log"));
        let mut sizer = sizer(&dir);
        let start = Instant::now();

        sizer.observe((400, 300), start, &log, 1);
        // Mid-drag: 500x300 arrives, then 620x340. Neither may reach the file.
        let wait = sizer.observe((500, 300), start, &log, 1);
        assert_eq!(wait, Some(QUIET));
        sizer.observe((620, 340), start + Duration::from_millis(100), &log, 1);
        assert!(!dir.join("settings.json").exists());

        // The same size a moment later is still pending, and the caller is told
        // how long is left rather than the whole period.
        let wait = sizer
            .observe((620, 340), start + Duration::from_millis(300), &log, 1)
            .expect("still pending");
        assert!(wait <= Duration::from_millis(200), "wait={wait:?}");

        sizer.observe((620, 340), start + Duration::from_millis(500), &log, 1);
        let (loaded, source) = SpikeSettings::load(&dir.join("settings.json"));
        assert_eq!(source, SettingsSource::File);
        assert_eq!((loaded.quick_note.width, loaded.quick_note.height), (620, 340));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_drag_that_returns_to_the_stored_size_writes_nothing() {
        let dir = temp_dir("undone");
        let log = Log::new(dir.join("note.log"));
        let mut sizer = sizer(&dir);
        let start = Instant::now();

        sizer.observe((400, 300), start, &log, 1);
        sizer.observe((700, 500), start, &log, 1);
        // Dragged back before the quiet period elapsed.
        assert_eq!(
            sizer.observe((400, 300), start + Duration::from_millis(200), &log, 1),
            None
        );
        sizer.observe((400, 300), start + Duration::from_millis(600), &log, 1);
        assert!(!dir.join("settings.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_remembered_size_is_clamped_and_other_keys_survive() {
        let dir = temp_dir("keeps");
        let path = dir.join("settings.json");
        let mut settings = SpikeSettings::default();
        settings.quick_note.width = 500;
        settings.quick_note.height = 400;
        settings
            .shortcuts
            .insert("switcher".to_string(), "Ctrl+Shift+K".to_string());
        settings.save(&path).expect("seed");

        let log = Log::new(dir.join("note.log"));
        let mut sizer = Sizer::new(shared(500, 400), Arc::new(path.clone()));
        let start = Instant::now();
        sizer.observe((500, 400), start, &log, 1);
        // 2x2 is below the legal floor; it must land on the floor, not on a default.
        sizer.observe((2, 2), start, &log, 1);
        sizer.observe((2, 2), start + Duration::from_millis(600), &log, 1);

        let (loaded, _) = SpikeSettings::load(&path);
        assert_eq!(
            (loaded.quick_note.width, loaded.quick_note.height),
            (hub_core::MIN_NOTE_WIDTH as u32, hub_core::MIN_NOTE_HEIGHT as u32)
        );
        assert_eq!(
            loaded.shortcuts.get("switcher").map(String::as_str),
            Some("Ctrl+Shift+K"),
            "a resize must not drop a rebind written since startup"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_settings_file_is_not_overwritten() {
        // The load falls back to defaults, so saving would replace a broken file
        // with a default one. The resize applies to this process; the file is left
        // for its owner to fix.
        let dir = temp_dir("broken");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{not json").unwrap();

        let log = Log::new(dir.join("note.log"));
        let mut sizer = Sizer::new(shared(400, 300), Arc::new(path.clone()));
        let start = Instant::now();
        sizer.observe((400, 300), start, &log, 1);
        sizer.observe((700, 500), start, &log, 1);
        sizer.observe((700, 500), start + Duration::from_millis(600), &log, 1);

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{not json");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
