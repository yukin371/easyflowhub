//! The text clipboard: read it, write it, and put back what a capture took.
//!
//! Two rules shape this module, and both come from the failure modes rather than
//! from taste:
//!
//! * **Every open is bounded.** Another program can own the clipboard while we
//!   ask for it — a screenshot tool bound to the same chord is the realistic case,
//!   and it is exactly what happens on the launcher's own hotkey. An unbounded
//!   retry loop is a hung panel, not a slow capture.
//! * **A snapshot is only worth what it can restore.** We copy text and nothing
//!   else, so a clipboard that held an image is reported as [`Prior::Other`]
//!   instead of being "restored" to something we never had.
//!
//! The Tauri build used the `clipboard_win` crate; this is the same four calls it
//! made underneath (`OpenClipboard` / `GetClipboardData` / `SetClipboardData` /
//! `GetClipboardSequenceNumber`), written out so the retry deadline and the
//! ownership rules are visible here rather than in a dependency.

use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

/// `CF_UNICODETEXT`, spelled out: the `windows-sys` feature that exports it is
/// `Win32_System_Ole`, which is a large surface to pull in for one constant.
const CF_UNICODETEXT: u32 = 13;

/// How long to keep asking for a clipboard another program is holding.
///
/// Short on purpose. The caller has a budget of its own (the launcher's capture
/// must finish before the panel takes focus), and this is the slice of it spent
/// waiting on the OS.
const OPEN_BUDGET: Duration = Duration::from_millis(60);

/// What the clipboard held before somebody copied over it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prior {
    /// Nothing at all: no format was available.
    Empty,
    /// Text, which is the only thing this module can put back.
    Text(String),
    /// Something else — an image, files, a private format. Readable by name, not
    /// by us, and therefore not restorable.
    Other,
}

impl Prior {
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Text(_) => "text",
            Self::Other => "other",
        }
    }
}

/// The clipboard's change counter.
///
/// This is what makes "did the application copy anything?" answerable without
/// guessing: a copy bumps the number even when the text is identical, and an
/// application with no selection bumps nothing. Comparing the *content* would
/// confuse "copied the same words again" with "did not copy".
pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Take a snapshot honest enough to be restored from.
pub fn snapshot() -> Prior {
    if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } != 0 {
        return match text() {
            Some(value) => Prior::Text(value),
            // Present but unreadable: treat it as something we did not copy.
            None => Prior::Other,
        };
    }
    if has_any_format() {
        Prior::Other
    } else {
        Prior::Empty
    }
}

/// Put `prior` back. `false` means the clipboard is not what it was.
///
/// [`Prior::Other`] is the case worth being loud about: the capture already
/// replaced it with text, and no amount of writing gets an image back. Saying so
/// lets the caller log it instead of pretending the round trip was clean.
pub fn restore(prior: &Prior) -> bool {
    match prior {
        Prior::Text(value) => set_text(value),
        Prior::Empty => clear(),
        Prior::Other => false,
    }
}

/// Read the clipboard as text, or `None` when it holds none.
pub fn text() -> Option<String> {
    let _open = Open::acquire()?;
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if handle.is_null() {
        return None;
    }
    let size = unsafe { GlobalSize(handle) };
    if size < 2 {
        return None;
    }
    let raw = unsafe { GlobalLock(handle) } as *const u16;
    if raw.is_null() {
        return None;
    }
    // Bounded by the allocation, not by the first NUL: a malformed owner can hand
    // back a block whose terminator is missing, and reading past it is a crash.
    let units = size / 2;
    let mut value = String::new();
    for index in 0..units {
        let unit = unsafe { *raw.add(index) };
        if unit == 0 {
            break;
        }
        value.push(char::from_u32(unit as u32).unwrap_or('\u{fffd}'));
    }
    unsafe { GlobalUnlock(handle) };
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Replace the clipboard with `value`.
pub fn set_text(value: &str) -> bool {
    let Some(_open) = Open::acquire() else {
        return false;
    };
    let wide = encode(value);
    unsafe {
        if EmptyClipboard() == 0 {
            return false;
        }
        let bytes = wide.len() * 2;
        let block = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if block.is_null() {
            return false;
        }
        let target = GlobalLock(block) as *mut u16;
        if target.is_null() {
            GlobalFree(block);
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
        GlobalUnlock(block);
        // Ownership moves to the clipboard on success; freeing it afterwards would
        // hand the OS a dangling block.
        if SetClipboardData(CF_UNICODETEXT, block).is_null() {
            GlobalFree(block);
            return false;
        }
    }
    true
}

/// Leave the clipboard empty — the restore for [`Prior::Empty`].
pub fn clear() -> bool {
    let Some(_open) = Open::acquire() else {
        return false;
    };
    unsafe { EmptyClipboard() != 0 }
}

fn has_any_format() -> bool {
    let Some(_open) = Open::acquire() else {
        return false;
    };
    unsafe { EnumClipboardFormats(0) != 0 }
}

/// UTF-16 with the terminator `CF_UNICODETEXT` requires.
fn encode(value: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = value.encode_utf16().collect();
    wide.push(0);
    wide
}

/// An open clipboard, closed on drop.
///
/// `OpenClipboard` fails while another process owns it, so the attempt is retried
/// against [`OPEN_BUDGET`] rather than once: the realistic contender is a
/// screenshot tool that fired on the same keystroke and holds the clipboard for a
/// few milliseconds.
struct Open;

impl Open {
    fn acquire() -> Option<Self> {
        let at = Instant::now();
        loop {
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return Some(Self);
            }
            if at.elapsed() >= OPEN_BUDGET {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_string_carries_its_terminator() {
        assert_eq!(encode("ab"), vec![0x61, 0x62, 0]);
    }

    #[test]
    fn an_empty_string_is_just_the_terminator() {
        // Not an error: `set_text("")` is how a caller empties the clipboard, and
        // the block must still be a legal `CF_UNICODETEXT` value.
        assert_eq!(encode(""), vec![0]);
    }

    #[test]
    fn the_sequence_number_is_a_counter_not_a_clock() {
        // Whatever it is, two reads in a row must not go backwards: callers use
        // `!=` on it to answer "did anything copy?", and a value that could repeat
        // inside one capture would answer "no" for a copy that did happen.
        let first = sequence();
        let second = sequence();
        assert!(second >= first, "{first} then {second}");
    }

    #[test]
    fn restore_of_an_other_format_reports_failure_rather_than_guessing() {
        // No OS call: this is the branch that decides *not* to call one. The
        // alternative -- clearing -- would turn "your image is gone" into "your
        // clipboard is empty", which is the same loss reported less honestly.
        assert!(!restore(&Prior::Other));
    }

    #[test]
    fn prior_describes_itself_for_the_log() {
        assert_eq!(Prior::Text("x".into()).describe(), "text");
        assert_eq!(Prior::Empty.describe(), "empty");
        assert_eq!(Prior::Other.describe(), "other");
    }
}
