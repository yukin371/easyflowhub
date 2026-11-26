//! Where the caret is, and how it gets there.
//!
//! ## Why this is a separate file
//!
//! `lib.rs` was at the size cap and this is a seam it already had: every call here
//! is one `EM_*SEL` message and none of them touch the window, the theme or the
//! callbacks. Moving them out is what made room for [`Editor::set_caret`] without
//! splitting a function that belongs together.
//!
//! ## Units
//!
//! RichEdit counts selection positions in **character positions**, which are not
//! bytes, not `char`s and not UTF-16 code units: a `CRLF` pair is *one* position
//! while an emoji is two. [`position_of`] converts a byte offset in the control's
//! own text into that number, and [`Editor::caret`] answers in the same units.
//!
//! Measured, because the difference is invisible until it is not: handing
//! `EM_SETSEL` the UTF-16 offset 8 of `买菜清单\r\n鸡蛋\r\n# 工作 重要` puts the
//! caret at the *start of the tag line* -- 8 counts the line breaks twice, and the
//! real position 8 is one character further right. The first keystroke after
//! reopening a note landed on the wrong side of the `#`.
//!
//! ## Focus
//!
//! `EM_SETSEL` and `EM_SCROLLCARET` are silently ignored while the control does not
//! have the focus, which is why the note focuses the island *before* placing the
//! caret (Q19, measured: without it the caret sat at 0 and only the first screenful
//! ever scrolled).

use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, SendMessageW};

use crate::{Editor, EM_GETSEL, EM_SETSEL};

impl Editor {
    /// Put the caret at `position` (a character position, see the module docs),
    /// with nothing selected.
    ///
    /// Clamped to the document: a position past the end would make RichEdit place
    /// the caret at the end anyway, but reporting a number the control never used
    /// back through [`Editor::caret`] is the kind of quiet mismatch that costs an
    /// afternoon later.
    pub fn set_caret(&self, position: usize) {
        let end = self.unit_count();
        let at = position.min(end) as isize;
        unsafe {
            SendMessageW(self.hwnd, EM_SETSEL, at as usize, at);
        }
    }

    pub fn caret_to_end(&self) {
        unsafe {
            let end = GetWindowTextLengthW(self.hwnd) as usize;
            SendMessageW(self.hwnd, EM_SETSEL, end, end as isize);
        }
    }

    /// Select the whole document. `lParam = -1` means "end of text" to RichEdit.
    pub fn select_all(&self) {
        unsafe {
            SendMessageW(self.hwnd, EM_SETSEL, 0, -1);
        }
    }

    pub fn caret(&self) -> (usize, usize) {
        unsafe {
            let mut start: u32 = 0;
            let mut end: u32 = 0;
            // EM_GETSEL packs both positions into the return value, but passing
            // pointers is unambiguous and works on every RichEdit version.
            SendMessageW(
                self.hwnd,
                EM_GETSEL,
                &mut start as *mut u32 as usize,
                &mut end as *mut u32 as isize,
            );
            (start as usize, end as usize)
        }
    }

    /// Length of the document in the units the selection messages count.
    fn unit_count(&self) -> usize {
        unsafe { GetWindowTextLengthW(self.hwnd) as usize }
    }
}

/// The character position of byte offset `byte` in `text`, as `EM_SETSEL` counts it.
///
/// `text` must be the control's own text ([`Editor::text`]), not the string that was
/// pushed into it: RichEdit keeps line breaks as `CRLF`, and a pair is one position.
/// Counting UTF-16 units instead overshoots by one per line break, which on a note
/// with a tag line puts the caret exactly where the tag line begins.
///
/// The byte offset has to be on a character boundary; every caller gets it from a
/// search over the same string, so this does not defend against a split code point.
pub fn position_of(text: &str, byte: usize) -> usize {
    let mut positions = 0;
    let mut rest = text[..byte].chars().peekable();
    while let Some(ch) = rest.next() {
        positions += ch.len_utf16();
        if ch == '\r' && rest.peek() == Some(&'\n') {
            // One line break, one position: the LF that belongs to this CR adds none.
            rest.next();
        }
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::position_of;

    #[test]
    fn a_line_break_counts_once() {
        let text = "买菜清单\r\n鸡蛋\r\n# 工作 重要";
        // End of the body, which is the offset the note parks its caret at.
        assert_eq!(position_of(text, "买菜清单\r\n鸡蛋".len()), 7);
        // The whole document: 4 + 1 + 2 + 1 + 7.
        assert_eq!(position_of(text, text.len()), 15);
        // ... which is two fewer than UTF-16 units, one per line break.
        assert_eq!(text.encode_utf16().count(), 17);
    }

    #[test]
    fn a_bare_lf_is_one_position_and_so_is_each_cr_of_a_double_break() {
        assert_eq!(position_of("a\nb", 3), 3);
        let wrapped = "a\r\n\r\nb";
        assert_eq!(position_of(wrapped, 5), 3);
        assert_eq!(position_of(wrapped, wrapped.len()), 4);
    }

    #[test]
    fn a_surrogate_pair_is_two_positions() {
        let text = "😀\r\nx";
        assert_eq!(position_of(text, 4), 2);
        assert_eq!(position_of(text, text.len()), 4);
    }
}
