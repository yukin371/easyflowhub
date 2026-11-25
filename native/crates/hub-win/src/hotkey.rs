//! `RegisterHotKey` on a dedicated thread.
//!
//! Three decisions here matter for latency, and the first two were paid for in the
//! Tauri build:
//!
//! 1. **The hotkey owns its own message loop.** A hotkey registered with a null
//!    window handle is delivered to the registering *thread's* queue. Running it on
//!    the UI thread means every reveal competes with painting, and any blocking
//!    work in the callback (the old build captured the selection inline and could
//!    block on a clipboard owned by a screenshot tool) freezes the window. Here the
//!    thread runs the caller's closure — which only reads the cursor, hits the
//!    cached monitor layout and posts a message — then returns to `GetMessageW`.
//! 2. **Debounce happens before the closure runs.** QQ/WeChat install low-level
//!    keyboard hooks for the same combinations, so one physical press can arrive as
//!    two or three messages; without this the panel opens and closes immediately.
//! 3. **`MOD_NOREPEAT`.** Holding the key for 300 ms must not toggle a dozen times.
//!
//! ## Several combinations, one loop
//!
//! Batch 1b.3 needs four global keys. A thread per key would give four message
//! queues and four debounce clocks for one user gesture, so [`HotkeySet`] registers
//! them on one thread and tells them apart by `wParam`. The debounce is per slot on
//! purpose: pressing `Ctrl+Alt+N` and then `Ctrl+Alt+D` inside 250 ms is two
//! decisions, not an echo. [`Hotkey`] stays as the one-binding wrapper the spike
//! uses, because its measurement is "one panel, one key" and it should keep failing
//! loudly when that key cannot be registered -- which for a set is only a log line.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_QUIT,
};

/// Arbitrary but stable base id; `RegisterHotKey` only needs it unique per thread.
const HOTKEY_ID: i32 = 0xE1;

const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const MOD_NOREPEAT: u32 = 0x4000;

pub type PressHandler = Arc<dyn Fn() + Send + Sync>;

/// One combination and what to run when it is pressed.
pub struct Binding {
    pub combo: String,
    pub on_press: PressHandler,
}

impl Binding {
    pub fn new(combo: impl Into<String>, on_press: PressHandler) -> Self {
        Self {
            combo: combo.into(),
            on_press,
        }
    }
}

/// Several global combinations served by one thread.
pub struct HotkeySet {
    thread_id: u32,
    combos: Vec<String>,
    failures: Vec<String>,
}

impl HotkeySet {
    /// Register every binding that can be registered and start pumping messages.
    ///
    /// Returns `Err` only when the thread could not start at all. A combination that
    /// another program already owns lands in [`HotkeySet::failures`] instead: the
    /// remaining keys still work, and refusing to open a note because `Ctrl+Alt+M`
    /// is taken would be a worse outcome than a missing shortcut.
    pub fn spawn(bindings: Vec<Binding>, debounce: Duration) -> Result<Self, String> {
        if bindings.is_empty() {
            return Err("no bindings to register".to_string());
        }
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<(u32, Vec<String>, Vec<String>)>();

        thread::Builder::new()
            .name("hub-hotkey".to_string())
            .spawn(move || {
                let mut slots: Vec<(i32, PressHandler)> = Vec::new();
                let mut combos: Vec<String> = Vec::new();
                let mut failures: Vec<String> = Vec::new();

                for binding in bindings {
                    let (modifiers, vk) = match parse_combo(&binding.combo) {
                        Ok(parsed) => parsed,
                        Err(error) => {
                            failures.push(error);
                            continue;
                        }
                    };
                    let id = HOTKEY_ID + slots.len() as i32;
                    let registered = unsafe {
                        RegisterHotKey(std::ptr::null_mut(), id, modifiers | MOD_NOREPEAT, vk)
                    };
                    if registered == 0 {
                        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                        failures.push(format!(
                            "{}: RegisterHotKey failed (win32 error {code}); another program already owns it",
                            binding.combo
                        ));
                        continue;
                    }
                    slots.push((id, binding.on_press));
                    combos.push(binding.combo);
                }

                let thread_id = unsafe { GetCurrentThreadId() };
                let _ = ready_tx.send((thread_id, combos, failures));

                let mut last_press: Vec<Option<Instant>> = vec![None; slots.len()];
                let mut msg = MSG::default();
                loop {
                    let got = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
                    if got <= 0 || msg.message == WM_QUIT {
                        break;
                    }
                    if msg.message != WM_HOTKEY {
                        continue;
                    }
                    let id = msg.wParam as i32;
                    let Some(index) = slots.iter().position(|(slot, _)| *slot == id) else {
                        continue;
                    };

                    let now = Instant::now();
                    if let Some(previous) = last_press[index] {
                        if now.duration_since(previous) < debounce {
                            continue;
                        }
                    }
                    last_press[index] = Some(now);
                    (slots[index].1)();
                }

                for (id, _) in &slots {
                    unsafe {
                        UnregisterHotKey(std::ptr::null_mut(), *id);
                    }
                }
            })
            .map_err(|error| format!("failed to spawn hotkey thread: {error}"))?;

        match ready_rx.recv() {
            Ok((thread_id, combos, failures)) => Ok(Self {
                thread_id,
                combos,
                failures,
            }),
            Err(_) => Err("hotkey thread died before registering".to_string()),
        }
    }

    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    /// The combinations that are actually registered, in binding order.
    pub fn combos(&self) -> &[String] {
        &self.combos
    }

    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// Post `WM_HOTKEY` for slot `index`, as the OS would.
    pub fn post_press(&self, index: usize) -> bool {
        if index >= self.combos.len() {
            return false;
        }
        let posted = unsafe {
            PostThreadMessageW(self.thread_id, WM_HOTKEY, (HOTKEY_ID + index as i32) as usize, 0)
        };
        posted != 0
    }

    /// Post `count` presses `gap` apart, each a separate `WM_HOTKEY`.
    ///
    /// Used to prove the debounce: a screenshot tool echoing the same combination
    /// produces exactly this pattern, and it must yield one action, not `count`.
    pub fn post_presses(&self, index: usize, count: usize, gap: Duration) {
        let thread_id = self.thread_id;
        let id = HOTKEY_ID + index as i32;
        thread::spawn(move || {
            for step in 0..count {
                if step > 0 {
                    thread::sleep(gap);
                }
                unsafe {
                    PostThreadMessageW(thread_id, WM_HOTKEY, id as usize, 0);
                }
            }
        });
    }

    /// Ask the message loop to exit (used on the way out of the process).
    pub fn shutdown(&self) {
        unsafe {
            PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
        }
    }
}

/// One registered global hotkey and the thread serving it.
pub struct Hotkey {
    set: HotkeySet,
}

impl Hotkey {
    /// Parse `combo`, register it and start pumping messages.
    ///
    /// `on_press` runs on the hotkey thread, immediately after the debounce check.
    /// Returns the `RegisterHotKey` failure when the combination is already owned by
    /// another process — Windows awards these first come, first served, so the
    /// caller is expected to surface it rather than retry silently.
    pub fn spawn(combo: &str, debounce: Duration, on_press: PressHandler) -> Result<Self, String> {
        let set = HotkeySet::spawn(vec![Binding::new(combo, on_press)], debounce)?;
        // The spike measures one panel revealed by one key; a combo that did not
        // register is not "degraded", it is a measurement that cannot run.
        if let Some(failure) = set.failures().first() {
            return Err(failure.clone());
        }
        Ok(Self { set })
    }

    pub fn thread_id(&self) -> u32 {
        self.set.thread_id()
    }

    pub fn post_test_press(&self) -> bool {
        self.set.post_press(0)
    }

    pub fn post_presses(self: &Arc<Self>, count: usize, gap: Duration) {
        self.set.post_presses(0, count, gap);
    }

    pub fn shutdown(&self) {
        self.set.shutdown();
    }
}

/// `"Ctrl+Shift+Space"` → `(MOD_CONTROL | MOD_SHIFT, VK_SPACE)`.
fn parse_combo(combo: &str) -> Result<(u32, u32), String> {
    let mut modifiers = 0u32;
    let mut key: Option<u32> = None;

    for part in combo.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "shift" => modifiers |= MOD_SHIFT,
            "alt" => modifiers |= MOD_ALT,
            "win" | "super" => modifiers |= MOD_WIN,
            other => {
                if key.is_some() {
                    return Err(format!("{combo}: more than one non-modifier key"));
                }
                key = Some(
                    virtual_key(other)
                        .ok_or_else(|| format!("{combo}: unsupported key '{other}'"))?,
                );
            }
        }
    }

    let key = key.ok_or_else(|| format!("{combo}: no non-modifier key"))?;
    Ok((modifiers, key))
}

fn virtual_key(name: &str) -> Option<u32> {
    let upper = name.to_ascii_uppercase();
    if upper.len() == 1 {
        let byte = upper.as_bytes()[0];
        if byte.is_ascii_alphanumeric() {
            return Some(byte as u32);
        }
    }
    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(number) = rest.parse::<u32>() {
            if (1..=24).contains(&number) {
                return Some(0x70 + number - 1);
            }
        }
    }
    match upper.as_str() {
        "SPACE" => Some(0x20),
        "ENTER" | "RETURN" => Some(0x0D),
        "TAB" => Some(0x09),
        "ESC" | "ESCAPE" => Some(0x1B),
        "BACKSPACE" => Some(0x08),
        "GRAVE" | "`" => Some(0xC0),
        "SEMICOLON" => Some(0xBA),
        "QUOTE" => Some(0xDE),
        "COMMA" => Some(0xBC),
        "PERIOD" => Some(0xBE),
        "SLASH" => Some(0xBF),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_documented_combination() {
        assert_eq!(
            parse_combo("Ctrl+Shift+Space").unwrap(),
            (MOD_CONTROL | MOD_SHIFT, 0x20)
        );
    }

    #[test]
    fn parses_function_and_letter_keys() {
        assert_eq!(
            parse_combo("Ctrl+Alt+K").unwrap(),
            (MOD_CONTROL | MOD_ALT, b'K' as u32)
        );
        assert_eq!(
            parse_combo("Ctrl+Shift+F10").unwrap(),
            (MOD_CONTROL | MOD_SHIFT, 0x79)
        );
    }

    #[test]
    fn rejects_a_combination_without_a_key() {
        assert!(parse_combo("Ctrl+Shift").is_err());
    }

    #[test]
    fn rejects_two_non_modifier_keys() {
        assert!(parse_combo("Ctrl+A+B").is_err());
    }

    #[test]
    fn rejects_unknown_key_names() {
        assert!(parse_combo("Ctrl+Nope").is_err());
    }

    #[test]
    fn an_empty_set_is_an_error_rather_than_a_silent_no_op() {
        assert!(HotkeySet::spawn(Vec::new(), Duration::from_millis(1)).is_err());
    }

    #[test]
    fn post_press_rejects_slots_that_do_not_exist() {
        // No thread is started here: the guard is what is being pinned, and index
        // arithmetic must not post an id that belongs to no slot.
        let set = HotkeySet {
            thread_id: 0,
            combos: vec!["Ctrl+Alt+N".to_string()],
            failures: Vec::new(),
        };
        assert!(!set.post_press(1), "one combo means slot 0 is the only valid index");
    }
}
