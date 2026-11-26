//! The RichEdit text island: a native child control that occupies one rectangle
//! of the egui panel and owns all text input.
//!
//! Why a child window instead of drawing text ourselves: IME composition, undo,
//! selection, caret movement and accessibility are the system's job, and every
//! attempt to reimplement them for CJK input is where "fast" editors become slow
//! ones. The shell keeps drawing its own chrome; this crate only owns the hole.
//!
//! Design notes that cost real debugging time and should not be rediscovered:
//!
//! * `Msftedit.dll` must be loaded before `RICHEDIT50W` can be created. Without it
//!   `CreateWindowExW` fails with "window class not found" and it looks like a
//!   permissions problem.
//! * `EM_SETCHARFORMAT` silently ignores a `CHARFORMATW` whose `cbSize` is wrong:
//!   no error, no text colour, and the only symptom is "my colour did not apply".
//! * `yHeight` is twips (1/20 pt), not pixels — and points are already device
//!   independent, so multiplying them by `pixels_per_point` renders 16.5 pt on a
//!   150% screen. The old shell did exactly that; the body text looked oversized
//!   and every line wrapped earlier than the same text does in Notepad.
//! * `EM_SETRECT` is what puts an inset between the text and the island edge. Without
//!   it the caret sits at x = 0 on a wrapped line and its first pixel column is
//!   clipped by the control edge, which is what "the caret disappears after a line
//!   break" looked like.
//! * A zero-sized child window receives no paint messages, so the size is clamped
//!   to at least 1x1 in `hub-core::island`.

pub mod measure;
pub mod keys;
mod caret;
mod dispatch;
mod theme;

use dispatch::Callbacks;

pub use keys::KeyPress;
pub use theme::Theme;
// `position_of` is the bridge between a byte offset in the document and the
// number `EM_SETSEL` wants; a caller that computed an offset needs it.
pub use caret::position_of;

use std::ptr;
use std::sync::OnceLock;

use crate::theme::{CharFormat, ParaFormat2, LINE_SPACING_MULTIPLE, PFM_LINESPACING, SCF_ALL};

use hub_core::PixelRect;
use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::UpdateWindow;
use windows_sys::Win32::System::LibraryLoader::LoadLibraryW;
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
use windows_sys::Win32::UI::Input::Ime::{ImmGetContext, ImmReleaseContext};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetClassNameW, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, SendMessageW, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
    SystemParametersInfoW, GWL_STYLE, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOWNA, WM_KEYDOWN,
    WM_PRINTCLIENT, WS_CHILD, WS_CLIPCHILDREN, WS_VISIBLE,
};

// --- RichEdit messages (not exported by windows-sys; values from richedit.h) ---
pub(crate) const EM_GETSEL: u32 = 0x00B0;
pub(crate) const EM_SETSEL: u32 = 0x00B1;
const EM_SETRECT: u32 = 0x00B3;
const EM_REPLACESEL: u32 = 0x00C2;
const EM_SCROLLCARET: u32 = 0x00B7;
const EM_LINESCROLL: u32 = 0x00B6;
const EM_GETLINECOUNT: u32 = 0x00BA;
const EM_CANUNDO: u32 = 0x00C6;
const EM_UNDO: u32 = 0x00C7;
const EM_EMPTYUNDOBUFFER: u32 = 0x00CD;
const EM_GETFIRSTVISIBLELINE: u32 = 0x00CE;
const EM_GETLIMITTEXT: u32 = 0x00D5;
const EM_GETCHARFORMAT: u32 = 0x0400 + 58;
const EM_SETCHARFORMAT: u32 = 0x0400 + 68;
const EM_SETPARAFORMAT: u32 = 0x0400 + 71;
const EM_SETBKGNDCOLOR: u32 = 0x0400 + 67;
const EM_GETSCROLLPOS: u32 = 0x0400 + 221;
const EM_SETUNDOLIMIT: u32 = 0x0400 + 82;
const EM_REDO: u32 = 0x0400 + 84;
const EM_CANREDO: u32 = 0x0400 + 85;
const EM_AUTOURLDETECT: u32 = 0x0400 + 91;
const EM_SETLANGOPTIONS: u32 = 0x0400 + 120;
const EM_SETEDITSTYLE: u32 = 0x0400 + 204;
const EM_EXLIMITTEXT: u32 = 0x0400 + 53;
/// Forces the next typed text into a new undo unit. Without it RichEdit merges a
/// whole contiguous typing run into one unit, so "how many undo steps" is a
/// property of the run, not of the edits.
const EM_STOPGROUPTYPING: u32 = 0x0400 + 148;

const ECOOP_OR: u32 = 0x0002;
const SES_EXTENDBACKCOLOR: u32 = 0x0004;
const IMF_AUTOFONT: u32 = 0x0002;
const IMF_DUALFONT: u32 = 0x0080;

/// `SPI_*` actions for the system caret: read the current width, write a new one.
const SPI_GETCARETWIDTH: u32 = 0x2006;
const SPI_SETCARETWIDTH: u32 = 0x2007;

/// Read back from the control, so A5 can compare "what I set" with "what it kept"
/// instead of trusting the call.
#[derive(Clone, Debug)]
pub struct CharFormatEcho {
    pub face: String,
    pub height_twips: i32,
    pub color: COLORREF,
}

pub struct Editor {
    hwnd: HWND,
    parent: HWND,
    rect: PixelRect,
    callbacks: *mut Callbacks,
    /// The text inset last applied, kept so a resize can rebuild `EM_SETRECT`.
    padding: (i32, i32),
    /// System caret width before this editor changed it, restored on drop.
    caret_restore: Option<i32>,
    /// The text colour last applied. Remembered so [`Editor::pixel_probe`] can ask
    /// "did *this* colour reach the surface" instead of only "is the surface
    /// non-uniform". Atomic because `apply_theme` takes `&self`.
    foreground: std::sync::atomic::AtomicU32,
}

impl Editor {
    /// Create the island as a child of `parent`, clipped out of the parent's own
    /// painting by `WS_CLIPCHILDREN`.
    ///
    /// The parent arrives as a plain handle value so `app` never has to name a
    /// win32 type (the guard keeps platform types on this side of the boundary).
    pub fn create(parent: isize, rect: PixelRect, theme: &Theme) -> Result<Self, String> {
        let parent = parent as HWND;
        unsafe {
            // Registers the RICHEDIT50W window class as a side effect.
            LoadLibraryW(wide("Msftedit.dll").as_ptr());

            ensure_clip_children(parent);

            // No `WS_VSCROLL`: the system scrollbar is drawn with the light visual
            // style even when the control is themed dark, so it lands in a dark
            // note as a 17 px white gutter. Scrolling is kept (wheel plus
            // `EM_SCROLLCARET`); the visible position indicator is a self-drawn
            // bar in a later batch, because drawing it under the island is
            // impossible - a child window always paints over its parent.
            let style = WS_CHILD
                | WS_VISIBLE
                | ES_MULTILINE
                | ES_AUTOVSCROLL
                | ES_WANTRETURN;

            let hwnd = CreateWindowExW(
                0,
                wide("RICHEDIT50W").as_ptr(),
                wide("").as_ptr(),
                style,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                parent,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            );

            if hwnd.is_null() {
                return Err(format!(
                    "CreateWindowExW(RICHEDIT50W) failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let callbacks = Box::into_raw(Box::new(Callbacks {
                on_escape: None,
                on_key: None,
                on_wheel: None,
                on_edit: None,
                on_click: None,
            }));

            // ~800 lines of safety: keep the callback box reachable from the
            // subclass proc and free it in Drop.
            let installed = SetWindowSubclass(hwnd, Some(subclass_proc), 1, callbacks as usize);
            if installed == 0 {
                DestroyWindow(hwnd);
                drop(Box::from_raw(callbacks));
                return Err("SetWindowSubclass failed".to_string());
            }

            let editor = Self {
                hwnd,
                parent,
                rect,
                callbacks,
                padding: theme.padding,
                caret_restore: None,
                foreground: std::sync::atomic::AtomicU32::new(theme.foreground),
            };

            // 0x7FFF_FFF0: the 10k-character document has to fit with room to spare,
            // and the default limit differs between RichEdit versions.
            SendMessageW(editor.hwnd, EM_EXLIMITTEXT, 0, 0x7FFF_FFF0);
            SendMessageW(editor.hwnd, EM_SETUNDOLIMIT, 500, 0);
            // Do not let the control restyle user text as links/email addresses.
            SendMessageW(editor.hwnd, EM_AUTOURLDETECT, 0, 0);
            SendMessageW(
                editor.hwnd,
                EM_SETLANGOPTIONS,
                (IMF_AUTOFONT | IMF_DUALFONT) as WPARAM,
                0,
            );
            SendMessageW(
                editor.hwnd,
                EM_SETEDITSTYLE,
                ECOOP_OR as WPARAM,
                SES_EXTENDBACKCOLOR as LPARAM,
            );

            editor.apply_theme(theme);
            editor.apply_margins();
            Ok(editor)
        }
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd as isize
    }

    pub fn rect(&self) -> PixelRect {
        self.rect
    }

    /// Move/resize only when the rectangle actually changed. Returns whether a
    /// `SetWindowPos` was issued so the bench can prove it is not per-frame.
    pub fn set_rect(&mut self, rect: PixelRect) -> bool {
        if self.rect.same_as(&rect) {
            return false;
        }
        unsafe {
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.rect = rect;
        // The formatting rectangle is relative to the client area, so it has to be
        // rebuilt after every resize or the inset lives on in a stale size.
        self.apply_margins();
        true
    }

    /// Push the text inset into the control.
    ///
    /// `EM_SETRECT` is the only way to get an inset: RichEdit ignored the
    /// `LEFTMARGIN`/`RIGHTMARGIN` of a plain edit control, and `EM_SETMARGINS`
    /// moves the scrollbar gutter rather than the text origin.
    fn apply_margins(&self) {
        let (pad_x, pad_y) = self.padding;
        let pad_x = pad_x.max(0).min(self.rect.width / 2);
        let pad_y = pad_y.max(0).min(self.rect.height / 2);
        let rect = RECT {
            left: pad_x,
            top: pad_y,
            right: (self.rect.width - pad_x).max(pad_x + 1),
            bottom: (self.rect.height - pad_y).max(pad_y + 1),
        };
        unsafe {
            SendMessageW(
                self.hwnd,
                EM_SETRECT,
                0,
                &rect as *const RECT as isize,
            );
        }
    }

    pub fn apply_theme(&self, theme: &Theme) {
        unsafe {
            SendMessageW(self.hwnd, EM_SETBKGNDCOLOR, 0, theme.background as isize);
            let mut format = CharFormat::new(theme);
            SendMessageW(
                self.hwnd,
                EM_SETCHARFORMAT,
                SCF_ALL,
                &mut format as *mut CharFormat as isize,
            );

            if (theme.line_spacing - 1.0).abs() > 0.01 {
                let mut paragraph = ParaFormat2 {
                    cb_size: std::mem::size_of::<ParaFormat2>() as u32,
                    mask: PFM_LINESPACING,
                    ..std::mem::zeroed()
                };
                paragraph.line_spacing = (theme.line_spacing * 20.0).round() as i32;
                paragraph.line_spacing_rule = LINE_SPACING_MULTIPLE;
                SendMessageW(
                    self.hwnd,
                    EM_SETPARAFORMAT,
                    SCF_ALL,
                    &mut paragraph as *mut ParaFormat2 as isize,
                );
            }
        }
        self.foreground.store(
            theme.foreground,
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    /// Widen the system caret and remember what to put back.
    ///
    /// Split from [`Editor::apply_theme`] because it is a system write, and a
    /// redraw-heavy caller should not be repeating it every frame.
    pub fn apply_caret_width(&mut self, width: i32) {
        if width <= 0 {
            return;
        }
        let mut previous: i32 = 0;
        unsafe {
            if SystemParametersInfoW(
                SPI_GETCARETWIDTH,
                0,
                &mut previous as *mut i32 as *mut std::ffi::c_void,
                0,
            ) == 0
            {
                return;
            }
            if previous != width {
                SystemParametersInfoW(
                    SPI_SETCARETWIDTH,
                    width as u32,
                    ptr::null_mut(),
                    0,
                );
            }
            // Remember even when the width already matched: someone else's value
            // is still the one to restore.
            if self.caret_restore.is_none() {
                self.caret_restore = Some(previous);
            }
            // An existing caret keeps its old bitmap until it is recreated, so the
            // change would not be visible until the next focus switch.
            self.recreate_caret(width);
        }
    }

    /// Rebuild this thread's caret at `width` without touching system settings.
    ///
    /// `CreateCaret` replaces the caret of its window; RichEdit calls
    /// `SetCaretPos`/`ShowCaret` on the same handle afterwards, so the new bitmap
    /// is what the user sees.
    fn recreate_caret(&self, width: i32) {
        let height = 0i32; // 0 = caret height follows the window font
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::CreateCaret(
                self.hwnd,
                ptr::null_mut(),
                width,
                height,
            );
            windows_sys::Win32::UI::WindowsAndMessaging::ShowCaret(self.hwnd);
        }
    }

    pub fn char_format(&self) -> CharFormatEcho {
        unsafe {
            let mut format = CharFormat::blank();
            SendMessageW(
                self.hwnd,
                EM_GETCHARFORMAT,
                SCF_ALL,
                &mut format as *mut CharFormat as isize,
            );
            CharFormatEcho {
                face: format.face_name(),
                height_twips: format.height_twips(),
                color: format.text_color(),
            }
        }
    }

    pub fn limit(&self) -> usize {
        unsafe { SendMessageW(self.hwnd, EM_GETLIMITTEXT, 0, 0) as usize }
    }

    pub fn class_name(&self) -> String {
        let mut buffer = [0u16; 64];
        unsafe {
            let len = GetClassNameW(self.hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
            String::from_utf16_lossy(&buffer[..len.max(0) as usize])
        }
    }

    pub fn set_text(&self, text: &str) {
        unsafe {
            SetWindowTextW(self.hwnd, wide(text).as_ptr());
        }
    }

    pub fn text(&self) -> String {
        unsafe {
            let length = GetWindowTextLengthW(self.hwnd);
            if length <= 0 {
                return String::new();
            }
            let mut buffer = vec![0u16; length as usize + 1];
            let copied = GetWindowTextW(self.hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
            String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
        }
    }

    pub fn char_count(&self) -> usize {
        self.text().chars().count()
    }

    // The selection lives in `caret.rs`: `set_caret`, `caret_to_end`, `select_all`
    // and `caret`.

    pub fn undo(&self, times: usize) -> bool {
        unsafe {
            for _ in 0..times {
                if SendMessageW(self.hwnd, EM_CANUNDO, 0, 0) == 0 {
                    return false;
                }
                SendMessageW(self.hwnd, EM_UNDO, 0, 0);
            }
            true
        }
    }

    pub fn redo(&self, times: usize) {
        unsafe {
            for _ in 0..times {
                SendMessageW(self.hwnd, EM_REDO, 0, 0);
            }
        }
    }

    /// Whether there is anything to undo / redo right now.
    ///
    /// The shell asks before acting so the log can say `can=false` instead of
    /// reporting a "redo that did nothing" as if it had run.
    pub fn can_undo(&self) -> bool {
        unsafe { SendMessageW(self.hwnd, EM_CANUNDO, 0, 0) != 0 }
    }

    pub fn can_redo(&self) -> bool {
        unsafe { SendMessageW(self.hwnd, EM_CANREDO, 0, 0) != 0 }
    }

    pub fn clear_undo(&self) {
        unsafe {
            SendMessageW(self.hwnd, EM_EMPTYUNDOBUFFER, 0, 0);
        }
    }

    /// Ask RichEdit to close the current typing group.
    ///
    /// **Measured: this has no effect on `RICHEDIT50W`.** Typing 5 + this + typing 5
    /// still undoes in 1 step, while a caret move in the same spot produces 2 (see
    /// `examples/undo_probe.rs`). Kept because the message is part of the documented
    /// API and the negative result should not have to be rediscovered - do not rely
    /// on it to create undo boundaries.
    pub fn stop_group_typing(&self) {
        unsafe {
            SendMessageW(self.hwnd, EM_STOPGROUPTYPING, 0, 0);
        }
    }

    /// Deliver a key press synchronously and return the control's `LRESULT`.
    ///
    /// `0` from a `WM_KEYDOWN` means the message was consumed - which is how the
    /// subclass interception of Esc and Tab is verified. This does *not* produce
    /// `WM_CHAR`: translation is the message loop's job, see [`Editor::key_press`].
    pub fn key_down(&self, virtual_key: u16) -> isize {
        unsafe { SendMessageW(self.hwnd, WM_KEYDOWN, virtual_key as WPARAM, 0) as isize }
    }

    /// Post a key press, translate it, and return the resulting text.
    ///
    /// Used to prove that keys the subclass does *not* intercept still reach the
    /// control (`DefSubclassProc` fallback). A key that got swallowed would leave
    /// the text unchanged.
    pub fn key_press(&self, virtual_key: u16) -> String {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
        };

        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                self.hwnd,
                WM_KEYDOWN,
                virtual_key as WPARAM,
                1,
            );

            // Bounded pump: only messages for this control, and only a few of them.
            let mut guard = 32;
            loop {
                let mut message: MSG = std::mem::zeroed();
                let found = PeekMessageW(&mut message, self.hwnd, 0, 0, PM_REMOVE);
                if found == 0 || guard == 0 {
                    break;
                }
                guard -= 1;
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        self.text()
    }

    /// Scroll so the caret is inside the client area. Without this the control sits
    /// at line 0 no matter where the caret is, which is what "caret follows" means.
    pub fn scroll_caret(&self) {
        unsafe {
            SendMessageW(self.hwnd, EM_SCROLLCARET, 0, 0);
        }
    }

    pub fn first_visible_line(&self) -> i32 {
        unsafe { SendMessageW(self.hwnd, EM_GETFIRSTVISIBLELINE, 0, 0) as i32 }
    }

    /// Display lines after wrapping. Proves the 10k document actually needs to
    /// scroll, so "first visible line stayed at 0" cannot be blamed on a document
    /// that fits on one screen.
    pub fn line_count(&self) -> i32 {
        unsafe { SendMessageW(self.hwnd, EM_GETLINECOUNT, 0, 0) as i32 }
    }

    /// Scroll by lines without involving the caret. Unlike `EM_SCROLLCARET` this
    /// does not depend on a caret having been created, so it works on the parked
    /// window the bench runs in.
    pub fn lines_scroll(&self, lines: i32) {
        unsafe {
            SendMessageW(self.hwnd, EM_LINESCROLL, 0, lines as isize);
        }
    }

    /// Current scroll offset in pixels, `(x, y)`.
    pub fn scroll_pos(&self) -> (i32, i32) {
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            SendMessageW(
                self.hwnd,
                EM_GETSCROLLPOS,
                0,
                &mut point as *mut POINT as isize,
            );
        }
        (point.x, point.y)
    }

    pub fn replace_selection(&self, text: &str) {
        unsafe {
            SendMessageW(self.hwnd, EM_REPLACESEL, 1, wide(text).as_ptr() as isize);
        }
    }

    /// Hide the control without destroying it.
    ///
    /// The switcher draws over exactly this rectangle, and a child window is
    /// always in front of its parent's painting: the only way to see the panel is
    /// to take the control out of the picture. Hidden rather than resized to zero
    /// because a zero-sized control loses its layout and the caret position.
    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd as HWND, SW_HIDE);
        }
    }

    /// Put the control back without taking the keyboard back.
    ///
    /// `SW_SHOWNA` rather than `SW_SHOW`: the caller decides who gets the focus,
    /// and a show that activated the control would undo the frame's own focus
    /// bookkeeping during a switch.
    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd as HWND, SW_SHOWNA);
        }
    }

    pub fn focus(&self) -> bool {
        unsafe { SetFocus(self.hwnd) != ptr::null_mut() }
    }

    /// Whether the control currently owns the keyboard focus.
    ///
    /// `SetFocus` returns the *previous* focus window, so a non-null result does not
    /// prove the call worked on a window in another thread. Reading it back is the
    /// only way to tell "focus moved" from "focus was refused".
    pub fn is_focused(&self) -> bool {
        unsafe { GetFocus() == self.hwnd }
    }

    /// Whether the control has an input-method context.
    ///
    /// This is the automatable half of B9: if `ImmGetContext` returns NULL, no IME
    /// can ever compose into this control and the whole island is Latin-only. The
    /// other half - real composition with three installed IMEs - needs a human and
    /// is recorded as 未测. `WM_IME_*` messages are never intercepted: the subclass
    /// proc only inspects `WM_KEYDOWN`.
    pub fn imm_context(&self) -> bool {
        unsafe {
            let context = ImmGetContext(self.hwnd);
            if context.is_null() {
                return false;
            }
            ImmReleaseContext(self.hwnd, context);
            true
        }
    }

    /// Repaint the whole client area through `WM_PRINTCLIENT` into a memory DC and
    /// report what the control actually painted.
    ///
    /// This is how A4 proves the dark theme applied without a screen grab: a
    /// screenshot needs the window on screen at a known position, while this works
    /// on the parked window and cannot be fooled by what is layered on top.
    pub fn pixel_probe(&self) -> measure::PixelProbe {
        measure::probe_pixels(
            self.hwnd as isize,
            self.rect,
            self.foreground.load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// Insert one character the way the message loop would, with timing.
    pub fn append_char(&self, ch: char) -> measure::CharTiming {
        measure::append_char(self.hwnd, ch)
    }

    pub fn on_escape(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_escape = Some(callback);
        }
    }

    /// Install the window-level shortcut hook. The callback runs inside the
    /// subclass proc, so it must only raise a flag — window code is not re-entered
    /// from a message handler.
    pub fn on_key(&mut self, callback: Box<dyn Fn(KeyPress) -> bool + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_key = Some(callback);
        }
    }

    /// Install the `Ctrl+wheel` hook. Same rule as [`Editor::on_key`]: raise a flag,
    /// do not touch the window from inside the message handler.
    pub fn on_wheel(&mut self, callback: Box<dyn Fn(i32) -> bool + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_wheel = Some(callback);
        }
    }

    /// Install the "the text may have changed" hook.
    ///
    /// Fires on the messages that can alter the document, never on the ones that
    /// only move the caret. The callback is a hint, not the source of truth: the
    /// shell compares the text against what it last wrote, so a message this list
    /// misses costs latency (the next scan finds it) rather than content.
    pub fn on_edit(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_edit = Some(callback);
        }
    }

    /// Install the "the body was clicked" hook (Q7: a click outside the tools
    /// panel closes it).
    ///
    /// Same rule as the others, plus one of its own: this is reported and not
    /// claimed, so the control still places the caret, selects a word on a double
    /// click and drag-selects.
    pub fn on_click(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_click = Some(callback);
        }
    }

    pub fn parent(&self) -> isize {
        self.parent as isize
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
            if !self.callbacks.is_null() {
                drop(Box::from_raw(self.callbacks));
            }
        }
        // Put the system caret width back. Doing this after the destroy matters:
        // the control hides the caret on WM_DESTROY and would otherwise recreate
        // one at the old width.
        if let Some(width) = self.caret_restore.take() {
            unsafe {
                SystemParametersInfoW(SPI_SETCARETWIDTH, width.max(1) as u32, ptr::null_mut(), 0);
            }
        }
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    refdata: usize,
) -> LRESULT {
    if refdata != 0 {
        let callbacks = unsafe { &*(refdata as *const Callbacks) };
        if let Some(claimed) = unsafe { dispatch::claim(hwnd, message, wparam, lparam, callbacks) } {
            return claimed;
        }
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

/// `WS_CLIPCHILDREN` on the parent is what stops the GL swap from painting over
/// the child control every frame.
fn ensure_clip_children(parent: HWND) {
    unsafe {
        let style = GetWindowLongPtrW(parent, GWL_STYLE) as u32;
        if style & WS_CLIPCHILDREN == 0 {
            SetWindowLongPtrW(parent, GWL_STYLE, (style | WS_CLIPCHILDREN) as isize);
        }
    }
}

/// Refresh without going through the message queue, used by the bench.
pub fn repaint_now(hwnd: HWND) {
    unsafe {
        UpdateWindow(hwnd);
    }
}

pub fn print_client(hwnd: HWND, hdc: isize) -> isize {
    const PRF_CLIENT: usize = 0x0000_0004;
    unsafe { SendMessageW(hwnd, WM_PRINTCLIENT, hdc as WPARAM, PRF_CLIENT as LPARAM) as isize }
}

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Monotonic microsecond clock. Percentiles of a wall clock would drift.
pub(crate) fn now_us() -> u64 {
    static FREQUENCY: OnceLock<i64> = OnceLock::new();
    let frequency = *FREQUENCY.get_or_init(|| {
        let mut value: i64 = 0;
        unsafe {
            QueryPerformanceFrequency(&mut value);
        }
        value.max(1)
    });

    let mut counter: i64 = 0;
    unsafe {
        QueryPerformanceCounter(&mut counter);
    }
    ((counter as i128 * 1_000_000) / frequency as i128) as u64
}

// ES_* styles that windows-sys does not export for the edit control family.
const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_WANTRETURN: u32 = 0x1000;
