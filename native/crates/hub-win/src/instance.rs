//! One shell per session: a named mutex, plus the event a second launch forwards
//! through.
//!
//! Without this, a login entry that starts the shell and a user who also clicks the
//! icon produce two processes, two tray icons, and a round of
//! `RegisterHotKey failed (1409)` -- the second process owns none of the keys it
//! promised, and nothing about the screen says which of the two icons is alive. The
//! Tauri build had a single-instance plugin for exactly this; the native shell never
//! got one, and the gap is why the login entry must not point here yet.
//!
//! Three decisions worth naming:
//!
//! * **`Local\`, not `Global\`.** Windows sessions are independent desktops: a
//!   console user and an RDP user each legitimately want their own shell, and
//!   `Global\` would make the second one refuse to start with no explanation the
//!   user could act on. The mutex is per session because the thing it protects
//!   (a tray icon, a set of global chords, a parked window) is per session.
//!
//! * **An event, not the mutex itself, carries the request.** A mutex cannot carry
//!   a payload, and waiting on it would block until the owner *exits* -- which is
//!   the opposite of what a second launch means. An auto-reset event is a doorbell:
//!   the second process sets it and leaves, the first one wakes and answers.
//!
//! * **The event is created in [`claim`], not in the watcher.** A signal that lands
//!   between "this process owns the shell" and "a thread is waiting for signals"
//!   would be dropped, and that window is precisely the login race -- the autostart
//!   entry is claimed while the desktop is still settling, so a double-click can
//!   arrive before any watcher exists. Creating the event with the mutex means the
//!   doorbell is already wired; the wait comes later and still sees it, because the
//!   auto-reset event stays signalled until somebody waits.
//!
//! * **Two doorbells, not one.** A second launch can mean "show me the panel" or
//!   "give me a note", and an event carries no payload: waking the single watcher
//!   with both requests would make `--note` show the panel. [`SHELL_SIGNAL`] stays
//!   the reveal ring; [`NOTE_SIGNAL`] is the note ring, watched by its own thread.
//!   Which note to open still cannot travel through an event, so that one name is
//!   left in a small file beside the settings (`app::pending`) -- see `--note-id`.

use std::sync::Arc;
use std::thread::{self, JoinHandle};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    INFINITE,
};

/// `WaitForSingleObject`'s failure return. This `windows-sys` feature set does not
/// export it, and writing `INFINITE` where a failure code belongs reads as a
/// duration -- which is how the check would end up silently never firing.
const WAIT_FAILED: u32 = 0xFFFF_FFFF;

/// The shell. Named after the product rather than the executable: a debug build and
/// a release build are the same shell as far as the user's desktop is concerned, and
/// letting both run is the double-tray bug with extra steps.
pub const SHELL_MUTEX: &str = r"Local\EasyFlowHub.Native.Shell";

/// The doorbell rung by a second launch that asks for the panel.
pub const SHELL_SIGNAL: &str = r"Local\EasyFlowHub.Native.Shell.reveal";

/// The doorbell rung by a second launch that asks for a note (`--note`).
///
/// Separate from [`SHELL_SIGNAL`] because the answer differs: the reveal watcher
/// shows the panel, this one spawns a window. One event cannot mean both, and an
/// event cannot say *which* note -- that is what the pending file is for.
pub const NOTE_SIGNAL: &str = r"Local\EasyFlowHub.Native.Shell.note";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Held for the life of the process. Dropping it releases the shell.
pub struct Guard {
    mutex: HANDLE,
    signal: HANDLE,
    note: HANDLE,
}

// SAFETY: both fields are kernel handles, which are process-wide and not tied to the
// thread that created them. The guard is moved into the controller and read by the
// watcher thread; neither touches the handle values concurrently (the watcher only
// waits on them), so the usual `*mut c_void` !Send objection does not apply.
unsafe impl Send for Guard {}
unsafe impl Sync for Guard {}

impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            if !self.mutex.is_null() {
                CloseHandle(self.mutex);
            }
            if !self.signal.is_null() {
                CloseHandle(self.signal);
            }
            if !self.note.is_null() {
                CloseHandle(self.note);
            }
        }
    }
}

/// What happened when this process asked to be the shell.
pub enum Claim {
    /// This process owns it; keep the guard alive or the next launch will claim it.
    First(Guard),
    /// Another process owns it. Signal and leave.
    Taken,
}

impl Guard {
    /// False when the mutex could not be created at all.
    ///
    /// A null handle means handle exhaustion or a broken object namespace -- not
    /// "another instance". The process runs anyway, because refusing to start is a
    /// worse failure than two shells, but the caller logs the difference instead of
    /// pretending the exclusivity is in force.
    pub fn owned(&self) -> bool {
        !self.mutex.is_null()
    }

    /// Wait for forwarded launches on a thread of its own.
    ///
    /// Takes the `Arc` rather than a raw handle so the handles cannot be closed
    /// while this thread is blocked on one: the guard is what keeps them open.
    pub fn watch<F>(self: Arc<Self>, on_signal: F) -> Option<JoinHandle<()>>
    where
        F: FnMut() + Send + 'static,
    {
        self.watch_ring(|guard| guard.signal, on_signal)
    }

    /// Wait for the `--note` ring instead of the reveal one.
    ///
    /// A separate thread rather than a second branch in the reveal watcher: the two
    /// answers touch different things (a parked panel versus a new viewport), and one
    /// thread per ring keeps a slow spawn from delaying a panel reveal behind it.
    pub fn watch_note<F>(self: Arc<Self>, on_signal: F) -> Option<JoinHandle<()>>
    where
        F: FnMut() + Send + 'static,
    {
        self.watch_ring(|guard| guard.note, on_signal)
    }

    /// `pick` is a function pointer so the handle is read *inside* the thread. A
    /// raw `HANDLE` captured into the closure would make it non-`Send` even though
    /// the guard it comes from is not.
    fn watch_ring<F>(self: Arc<Self>, pick: fn(&Guard) -> HANDLE, mut on_signal: F) -> Option<JoinHandle<()>>
    where
        F: FnMut() + Send + 'static,
    {
        if pick(&self).is_null() {
            return None;
        }
        Some(thread::spawn(move || {
            // Owned by the thread for as long as it runs: the wait below is on a
            // handle this guard keeps alive.
            let guard = self;
            loop {
                // Auto-reset, so two launches that arrive together collapse into one
                // answer -- which is correct, because the panel is either up or not,
                // and a note request that arrives twice is still one window.
                let waited = unsafe { WaitForSingleObject(pick(&guard), INFINITE) };
                if waited == WAIT_FAILED {
                    return;
                }
                on_signal();
            }
        }))
    }
}

/// Ask to be the shell.
pub fn claim() -> Claim {
    claim_named(SHELL_MUTEX, SHELL_SIGNAL, NOTE_SIGNAL)
}

/// The names are parameters so the tests can use their own: two test threads in one
/// process would otherwise collide with each other and with the product's own mutex.
pub fn claim_named(mutex_name: &str, signal_name: &str, note_name: &str) -> Claim {
    let mutex_wide = wide(mutex_name);
    let signal_wide = wide(signal_name);
    let note_wide = wide(note_name);

    // Created first, and deliberately: see the module comment on the login race.
    let signal = unsafe { CreateEventW(std::ptr::null(), 0, 0, signal_wide.as_ptr()) };
    let note = unsafe { CreateEventW(std::ptr::null(), 0, 0, note_wide.as_ptr()) };
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, mutex_wide.as_ptr()) };
    // Read before anything else can call a Win32 function: `GetLastError` is
    // per-thread and every marshal overwrites it.
    let already = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

    if already {
        // We lost the race. Closing all three handles here rather than leaking them
        // is also what keeps the events from making the mutex look alive to `signal`.
        unsafe {
            if !mutex.is_null() {
                CloseHandle(mutex);
            }
            if !signal.is_null() {
                CloseHandle(signal);
            }
            if !note.is_null() {
                CloseHandle(note);
            }
        }
        return Claim::Taken;
    }

    Claim::First(Guard { mutex, signal, note })
}

/// Ring the doorbell of whichever process owns the shell.
///
/// Returns an error instead of creating anything when nobody is listening: the caller
/// is the second launch, and its only job is to hand the request over.
pub fn signal() -> Result<(), String> {
    signal_named(SHELL_SIGNAL)
}

/// Ring the note doorbell: "give me a window", not "show me the panel".
pub fn signal_note() -> Result<(), String> {
    signal_note_named(NOTE_SIGNAL)
}

pub fn signal_note_named(note_name: &str) -> Result<(), String> {
    signal_named(note_name)
}

pub fn signal_named(signal_name: &str) -> Result<(), String> {
    let name = wide(signal_name);
    let handle = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
    if handle.is_null() {
        let code = unsafe { GetLastError() };
        return Err(format!("OpenEventW failed (win32 error {code})"));
    }
    let set = unsafe { SetEvent(handle) };
    let code = unsafe { GetLastError() };
    unsafe { CloseHandle(handle) };
    if set == 0 {
        return Err(format!("SetEvent failed (win32 error {code})"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::time::{Duration, Instant};

    use super::*;

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A namespace nothing else on this machine uses. The tests own a real mutex in
    /// the real object namespace -- a mock here would prove the mock.
    fn names() -> (String, String, String) {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let tag = format!("efh-test-{}-{n}", std::process::id());
        (format!(r"Local\{tag}.mutex"), format!(r"Local\{tag}.signal"), format!(r"Local\{tag}.note"))
    }

    fn wait_until(flag: &AtomicBool, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if flag.load(Ordering::SeqCst) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[test]
    fn the_second_claim_is_taken() {
        let (mutex, signal, note) = names();
        let first = claim_named(&mutex, &signal, &note);
        assert!(matches!(first, Claim::First(_)));
        assert!(matches!(claim_named(&mutex, &signal, &note), Claim::Taken));
    }

    #[test]
    fn a_claim_reports_that_it_owns_the_mutex() {
        let (mutex, signal, note) = names();
        match claim_named(&mutex, &signal, &note) {
            Claim::First(guard) => assert!(guard.owned()),
            Claim::Taken => panic!("a name nothing else uses cannot be taken"),
        }
    }

    #[test]
    fn releasing_the_guard_lets_the_next_claim_win() {
        let (mutex, signal, note) = names();
        // Explicitness matters: the second claim in `the_second_claim_is_taken` is
        // taken *while the first guard is alive*, which is the bug being fixed. Here
        // the point is the opposite -- the shell is free once the owner is gone.
        let first = claim_named(&mutex, &signal, &note);
        assert!(matches!(first, Claim::First(_)));
        drop(first);
        assert!(matches!(claim_named(&mutex, &signal, &note), Claim::First(_)));
    }

    #[test]
    fn signalling_a_shell_that_does_not_exist_is_an_error() {
        let (_, signal, _) = names();
        let error = signal_named(&signal).expect_err("nothing owns this name");
        assert!(error.contains("OpenEventW"), "unexpected error: {error}");
    }

    #[test]
    fn signalling_a_note_ring_that_does_not_exist_is_an_error() {
        let (_, _, note) = names();
        let error = signal_note_named(&note).expect_err("nothing owns this name");
        assert!(error.contains("OpenEventW"), "unexpected error: {error}");
    }

    #[test]
    fn a_note_forward_reaches_the_note_watcher() {
        let (mutex, signal, note) = names();
        let Claim::First(guard) = claim_named(&mutex, &signal, &note) else {
            panic!("first claim");
        };
        let woken = Arc::new(AtomicBool::new(false));
        let flag = woken.clone();
        Arc::new(guard)
            .watch_note(move || flag.store(true, Ordering::SeqCst))
            .expect("the event exists");

        signal_note_named(&note).expect("the shell is up");
        assert!(wait_until(&woken, Duration::from_secs(2)), "the note ring rang nobody");
    }

    #[test]
    fn a_note_ring_does_not_wake_the_reveal_watcher() {
        // The reason there are two events: `--note` used to ring the same doorbell
        // as a plain second launch, so asking for a note showed the panel instead.
        let (mutex, signal, note) = names();
        let Claim::First(guard) = claim_named(&mutex, &signal, &note) else {
            panic!("first claim");
        };
        let revealed = Arc::new(AtomicBool::new(false));
        let flag = revealed.clone();
        let guard = Arc::new(guard);
        guard
            .clone()
            .watch(move || flag.store(true, Ordering::SeqCst))
            .expect("the event exists");

        signal_note_named(&note).expect("the shell is up");
        // The ring is a kernel wake-up, not a race with the thread's start: 200 ms is
        // an order of magnitude past the measured latency of the other watcher test.
        std::thread::sleep(Duration::from_millis(200));
        assert!(!revealed.load(Ordering::SeqCst), "a note ring reached the reveal watcher");
    }

    #[test]
    fn a_forward_reaches_the_watcher() {
        let (mutex, signal, note) = names();
        let Claim::First(guard) = claim_named(&mutex, &signal, &note) else {
            panic!("first claim");
        };
        let woken = Arc::new(AtomicBool::new(false));
        let flag = woken.clone();
        Arc::new(guard)
            .watch(move || flag.store(true, Ordering::SeqCst))
            .expect("the event exists");

        signal_named(&signal).expect("the shell is up");
        assert!(wait_until(&woken, Duration::from_secs(2)), "the doorbell rang nobody");
    }

    #[test]
    fn a_forward_sent_before_the_watcher_starts_is_not_lost() {
        // The login race: the shell is claimed, the desktop is still settling, a
        // second launch lands before any watcher thread exists. The auto-reset event
        // stays signalled, so the wait that comes later still answers immediately.
        let (mutex, signal, note) = names();
        let Claim::First(guard) = claim_named(&mutex, &signal, &note) else {
            panic!("first claim");
        };
        signal_named(&signal).expect("the shell is up");

        let woken = Arc::new(AtomicBool::new(false));
        let flag = woken.clone();
        Arc::new(guard)
            .watch(move || flag.store(true, Ordering::SeqCst))
            .expect("the event exists");
        assert!(wait_until(&woken, Duration::from_secs(2)), "the early ring was dropped");
    }

    #[test]
    fn two_forwards_collapse_into_one_reveal() {
        // Auto-reset is the reason: the panel is either up or not, so the second
        // ring while the first is still pending needs no second answer.
        let (mutex, signal, note) = names();
        let Claim::First(guard) = claim_named(&mutex, &signal, &note) else {
            panic!("first claim");
        };
        signal_named(&signal).expect("the shell is up");
        signal_named(&signal).expect("the shell is up");

        let rings = Arc::new(AtomicU32::new(0));
        let counter = rings.clone();
        Arc::new(guard)
            .watch(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            })
            .expect("the event exists");
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(rings.load(Ordering::SeqCst), 1);
    }
}
