//! The messages the shell claims before RichEdit sees them.
//!
//! Split out of `lib.rs` when that file hit the 800-line hard cap, but the boundary
//! is real: `lib.rs` manages the control (create, theme, sizes, text), while this
//! module is the **message path** — what the island swallows so that the shell can
//! behave like the shipped note instead of like a text box.
//!
//! Three messages are claimed, each for a different reason:
//!
//! | Message | Default behaviour | Why the shell takes it |
//! |---|---|---|
//! | `WM_KEYDOWN` `VK_ESCAPE` | beeps, does nothing | Esc closes the note (Q12) |
//! | `WM_KEYDOWN` `VK_TAB` | moves focus out of the island | a note is a scratch pad; Tab must insert a tab |
//! | `WM_KEYDOWN` other | RichEdit's own editing | the shortcut table decides (Ctrl+D/Z/T/Shift+H) |
//! | `WM_MOUSEWHEEL` +Ctrl | **zooms the text** | `Ctrl+wheel` is window opacity (Q28) |
//! | `WM_LBUTTONDOWN` | places the caret | a click in the body is "outside the tools panel" (Q7) |
//! | `WM_CHAR` / paste / cut / undo | edits the document | the shell is told "this may have changed", then the message continues to `DefSubclassProc` |
//!
//! Anything not claimed goes to `DefSubclassProc`, which is the point: reimplementing
//! editing would trade the platform's caret, selection and IME behaviour for nothing.

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_DELETE, VK_ESCAPE, VK_TAB};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetCursorPos, LoadCursorW, SendMessageW, SetCursor, IDC_SIZENS, IDC_SIZENESW,
    IDC_SIZENWSE, IDC_SIZEWE, WM_CHAR, WM_CLEAR, WM_CUT, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEWHEEL,
    WM_PASTE, WM_SETCURSOR, WM_UNDO,
};

use crate::keys;

/// A left press in the island, in the control's own client pixels.
///
/// `width`/`height` are the control's size at message time, measured here rather
/// than remembered: the island is resized every frame the window is, and a stale
/// size would put the resize band a few pixels away from the visible edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// What the pointer should look like over a given edge, so the grip is visible
/// before it is used.
///
/// Two diagonals, not four: a bottom-left grip is dragged along the NE/SW axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Horizontal,
    Vertical,
    NorthEastSouthWest,
    NorthWestSouthEast,
}

/// `WM_IME_CHAR`: an IME commit arrives as this, not as `WM_CHAR`, so watching only
/// `WM_CHAR` would leave Chinese input unsaved until the next scan.
///
/// Spelled out because the constant lives in `UI::Input::Ime` in the bindings, and
/// pulling that feature in for one number is not worth it. The value is from
/// `winuser.h` and does not change.
const WM_IME_CHAR: u32 = 0x0286;

/// `LOWORD(wParam)` bit of `WM_MOUSEWHEEL` meaning "Ctrl was held".
///
/// windows-sys exports `MK_*` under `Win32_System_SystemServices`, and pulling that
/// feature in for one bit is not worth it. The value is `MK_CONTROL` from
/// `winuser.h` and does not change.
const MK_CONTROL: u32 = 0x0008;

/// `HTCLIENT`: the pointer is over the client area, not a border or a button.
const HTCLIENT: u32 = 0x0001;

/// The callbacks the shell installs, owned by [`crate::Editor`].
pub(crate) struct Callbacks {
    pub(crate) on_escape: Option<Box<dyn Fn() + Send + Sync>>,
    /// Returns `true` when the shell consumed the key. A `false` falls through to
    /// `DefSubclassProc`, which is what keeps `Ctrl+Z` inside RichEdit's own undo
    /// instead of reimplementing it one level up.
    pub(crate) on_key: Option<Box<dyn Fn(keys::KeyPress) -> bool + Send + Sync>>,
    /// `Ctrl+wheel`, in wheel delta units (positive = away from the user). Returns
    /// `true` when the shell consumed the notch.
    ///
    /// The shell needs this on this side of the boundary because RichEdit owns the
    /// wheel: `Ctrl+wheel` is its text **zoom** gesture, so unless the message is
    /// intercepted here, scrolling the opacity would also resize the body text.
    pub(crate) on_wheel: Option<Box<dyn Fn(i32) -> bool + Send + Sync>>,
    /// "The text may have changed." Not a claim: the shell compares the document
    /// against what it last wrote, so this only decides *when* it looks.
    pub(crate) on_edit: Option<Box<dyn Fn() + Send + Sync>>,
    /// "The body was clicked." Not a claim either — the caret still has to move.
    ///
    /// This exists because the click is invisible one level up: the island is a
    /// child window covering the body rectangle, so egui's parent window never
    /// receives the press. The shell needs it to dismiss the tools panel (Q7).
    pub(crate) on_click: Option<Box<dyn Fn() + Send + Sync>>,
    /// "A press landed here; is it yours?" Returns `true` when the shell claims it,
    /// in which case the press never reaches RichEdit.
    ///
    /// The one claimer is the resize band on the note's own edge (Q3). Claiming is
    /// the point: without it the control would place the caret and start a text
    /// selection at the same moment the window starts to grow, which is both
    /// visible and impossible to undo -- the selection moves with the pointer.
    pub(crate) on_press: Option<Box<dyn Fn(Press) -> bool + Send + Sync>>,
    /// "What cursor belongs here?" `Some` sets it and claims the message; `None`
    /// leaves the control's own I-beam alone.
    ///
    /// The band a press can start a resize in is the band the pointer has to
    /// advertise, or the grip exists only for people who already know about it.
    pub(crate) on_hover: Option<Box<dyn Fn(Press) -> Option<Shape> + Send + Sync>>,
}

/// Handle one message. `Some` means "claimed: do not pass it on".
///
/// # Safety
///
/// `callbacks` must point at the box installed with `SetWindowSubclass` and stay
/// alive for the call, which is what `Editor` guarantees for the life of the
/// control.
pub(crate) unsafe fn claim(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    callbacks: &Callbacks,
) -> Option<LRESULT> {
    if message == WM_KEYDOWN {
        match wparam as u16 {
            // Backspace and Delete are edits, not shortcuts. Reported and then
            // handed to `DefSubclassProc`: the control does the deleting.
            VK_BACK | VK_DELETE => {
                if let Some(callback) = callbacks.on_edit.as_ref() {
                    callback();
                }
                return None;
            }
            // Esc: RichEdit does nothing with it but would beep; the shell needs
            // to see it so the panel closes instead of the editor swallowing it.
            VK_ESCAPE => {
                if let Some(callback) = callbacks.on_escape.as_ref() {
                    callback();
                }
                return Some(0);
            }
            // Tab: insert a tab instead of moving focus out of the panel.
            VK_TAB => {
                unsafe { SendMessageW(hwnd, WM_CHAR, 0x09, 1) };
                return Some(0);
            }
            // Everything else goes through the shell's shortcut table. Esc stays
            // above it because closing is not a table entry: it must work even if
            // the table is empty or the callback was never installed.
            other => {
                if let Some(callback) = callbacks.on_key.as_ref() {
                    let press = keys::modifiers(keys::original_key(hwnd, other, lparam));
                    if callback(press) {
                        return Some(0);
                    }
                }
            }
        }
        return None;
    }

    // The editing messages. None of them is claimed -- the control must still
    // insert, paste, cut and undo -- but each is a reason to look at the document
    // before the next scan would.
    //
    // `WM_UNDO` is here for the app's own undo path, which sends `EM_UNDO` instead;
    // the shortcut table's `Ctrl+Z` changes the text without any message at all, so
    // that path raises the flag itself.
    if matches!(
        message,
        WM_CHAR | WM_IME_CHAR | WM_PASTE | WM_CUT | WM_CLEAR | WM_UNDO
    ) {
        if let Some(callback) = callbacks.on_edit.as_ref() {
            callback();
        }
        return None;
    }

    // The pointer over the band: advertise the grip. `WM_SETCURSOR` carries the
    // hit-test code in `LOWORD(lParam)`; anything but `HTCLIENT` is a frame this
    // window does not have.
    if message == WM_SETCURSOR && (lparam as u32 & 0xFFFF) == HTCLIENT {
        if let Some(callback) = callbacks.on_hover.as_ref() {
            if let Some(shape) = callback(press_under(hwnd)) {
                unsafe { SetCursor(cursor_for(shape)) };
                return Some(1);
            }
        }
        return None;
    }

    // A press in the body. The shell gets first refusal (the resize band on the
    // note's edge, Q3); anything it does not claim is reported for Q7 and handed
    // on, because claiming would cost caret placement, selection and drag-select --
    // everything a text control does with a mouse.
    if message == WM_LBUTTONDOWN {
        if let Some(callback) = callbacks.on_press.as_ref() {
            let (mut width, mut height) = (0, 0);
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
                width = rect.right - rect.left;
                height = rect.bottom - rect.top;
            }
            // LOWORD/HIGHORD are 16-bit and signed: a press outside the control
            // (still possible while another window holds capture) arrives negative.
            let x = (lparam as u32 & 0xFFFF) as u16 as i16 as i32;
            let y = ((lparam as u32 >> 16) & 0xFFFF) as u16 as i16 as i32;
            if callback(Press {
                x,
                y,
                width,
                height,
            }) {
                return Some(0);
            }
        }
        if let Some(callback) = callbacks.on_click.as_ref() {
            callback();
        }
        return None;
    }

    // `Ctrl+wheel` is RichEdit's zoom gesture, and the shell wants it for window
    // opacity. Claimed before `DefSubclassProc` so one wheel notch cannot do both:
    // the shipped note adjusts *only* the opacity, and a body that also grows a
    // point per notch is a different product.
    if message == WM_MOUSEWHEEL {
        let keys_down = (wparam as usize & 0xFFFF) as u32;
        if keys_down & MK_CONTROL != 0 {
            if let Some(callback) = callbacks.on_wheel.as_ref() {
                // HIWORD is the signed delta, in multiples of WHEEL_DELTA (120).
                let delta = ((wparam as usize >> 16) & 0xFFFF) as u16 as i16 as i32;
                if callback(delta) {
                    return Some(0);
                }
            }
        }
    }

    None
}

/// Where the cursor is, in the control's own client pixels, plus its size.
///
/// `WM_SETCURSOR` does not carry a position -- it is about "wherever the pointer
/// is now" -- so it has to be asked for, and the answer has to be in the same
/// coordinate space the press hook uses.
fn press_under(hwnd: HWND) -> Press {
    let mut point = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut point);
        ScreenToClient(hwnd, &mut point);
    }
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let (mut width, mut height) = (0, 0);
    if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
        width = rect.right - rect.left;
        height = rect.bottom - rect.top;
    }
    Press {
        x: point.x,
        y: point.y,
        width,
        height,
    }
}

/// The system cursor for a shape. Ids, not resources: these are the `IDC_*`
/// numbers from `winuser.h`, and `LoadCursorW` resolves them for any process.
fn cursor_for(shape: Shape) -> windows_sys::Win32::UI::WindowsAndMessaging::HCURSOR {
    let id = match shape {
        Shape::Horizontal => IDC_SIZEWE,
        Shape::Vertical => IDC_SIZENS,
        Shape::NorthEastSouthWest => IDC_SIZENESW,
        Shape::NorthWestSouthEast => IDC_SIZENWSE,
    };
    unsafe { LoadCursorW(std::ptr::null_mut(), id) }
}

/// The callback installers.
///
/// They live here rather than in `lib.rs` because this module owns the message
/// path they feed, and because `lib.rs` hit the 800-line cap: moving them keeps
/// "what the shell can hook" next to "what the shell then sees".
impl crate::Editor {
    pub fn on_escape(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_escape = Some(callback);
        }
    }

    /// Install the window-level shortcut hook. The callback runs inside the
    /// subclass proc, so it must only raise a flag — window code is not re-entered
    /// from a message handler.
    pub fn on_key(&mut self, callback: Box<dyn Fn(keys::KeyPress) -> bool + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_key = Some(callback);
        }
    }

    /// Install the `Ctrl+wheel` hook. Same rule as [`crate::Editor::on_key`]: raise
    /// a flag, do not touch the window from inside the message handler.
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

    /// Install the "is this press yours?" hook (Q3: the note's resize band).
    ///
    /// The only callback that can *claim* a mouse press. It runs inside the
    /// subclass proc like the others, so it must raise a flag rather than touch the
    /// window — but unlike them it answers a question, and the answer decides
    /// whether RichEdit ever sees the message.
    pub fn on_press(&mut self, callback: Box<dyn Fn(Press) -> bool + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_press = Some(callback);
        }
    }

    /// Install the "what cursor belongs here?" hook (Q3's resize band).
    ///
    /// Called on `WM_SETCURSOR`, which means it runs on every mouse move over the
    /// control, so it must stay a comparison: `Some` sets that cursor and claims
    /// the message, `None` leaves RichEdit's own I-beam in place.
    pub fn on_hover(&mut self, callback: Box<dyn Fn(Press) -> Option<Shape> + Send + Sync>) {
        unsafe {
            (*self.callbacks).on_hover = Some(callback);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ctrl_wheel_is_claimed_and_the_delta_is_signed() {
        // No window is created here: the wheel branch never touches `hwnd` unless a
        // callback is installed, so a `null` handle is enough to pin the arithmetic.
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<i32>::new()));
        let sink = seen.clone();
        let callbacks = Callbacks {
            on_escape: None,
            on_key: None,
            on_wheel: Some(Box::new(move |delta| {
                sink.lock().unwrap().push(delta);
                true
            })),
            on_edit: None,
            on_click: None,
            on_press: None,
            on_hover: None,
        };

        let hwnd = std::ptr::null_mut();
        // HIWORD = 120 (one notch up) with MK_CONTROL in LOWORD.
        let up: usize = 120usize << 16 | MK_CONTROL as usize;
        let claimed = unsafe { claim(hwnd, WM_MOUSEWHEEL, up, 0, &callbacks) };
        assert_eq!(claimed, Some(0), "Ctrl+wheel must not reach RichEdit");
        // HIWORD = -120, as two's complement in the upper word.
        let down: usize = 0xFF88usize << 16 | MK_CONTROL as usize;
        let claimed = unsafe { claim(hwnd, WM_MOUSEWHEEL, down, 0, &callbacks) };
        assert_eq!(claimed, Some(0));

        assert_eq!(*seen.lock().unwrap(), vec![120, -120]);

        // Without Ctrl the wheel is a scroll, not an opacity change.
        let plain: usize = 120usize << 16;
        let claimed = unsafe { claim(hwnd, WM_MOUSEWHEEL, plain, 0, &callbacks) };
        assert_eq!(claimed, None);
        assert_eq!(seen.lock().unwrap().len(), 2, "the plain wheel was not reported");
    }

    #[test]
    fn a_body_click_is_reported_but_never_claimed() {
        // Q7 closes the tools panel on a click outside it, and the click has to be
        // seen here: the island is a child window, so the press reaches this proc
        // and never the egui parent.
        let count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let sink = count.clone();
        let callbacks = Callbacks {
            on_escape: None,
            on_key: None,
            on_wheel: None,
            on_edit: None,
            on_click: Some(Box::new(move || {
                sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })),
            on_press: None,
            on_hover: None,
        };
        let hwnd = std::ptr::null_mut();

        let claimed = unsafe { claim(hwnd, WM_LBUTTONDOWN, 0, 0, &callbacks) };
        assert_eq!(claimed, None, "a click must still reach RichEdit");
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Moving the mouse is not a click, and must not dismiss the panel.
        let claimed = unsafe { claim(hwnd, 0x0200, 0, 0, &callbacks) };
        assert_eq!(claimed, None);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn a_press_the_shell_claims_never_reaches_richedit() {
        // Q3: the press starts a window resize, so the control must not also place
        // the caret and begin a selection -- both would happen on the same press,
        // and the selection would move with the pointer while the window grows.
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Press>::new()));
        let sink = seen.clone();
        let clicks = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let clicks_sink = clicks.clone();
        let callbacks = Callbacks {
            on_escape: None,
            on_key: None,
            on_wheel: None,
            on_edit: None,
            on_click: Some(Box::new(move || {
                clicks_sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })),
            on_press: Some(Box::new(move |press| {
                sink.lock().unwrap().push(press);
                // Claim anything in the left four pixels, like the resize band.
                press.x < 4
            })),
            on_hover: None,
        };
        let hwnd = std::ptr::null_mut();
        // x = 2, y = 40 as the message packs them.
        let lparam = (40usize << 16) | 2;

        let claimed = unsafe { claim(hwnd, WM_LBUTTONDOWN, 0, lparam as isize, &callbacks) };
        assert_eq!(claimed, Some(0), "a press in the band must not reach the control");
        assert_eq!(
            *seen.lock().unwrap(),
            vec![Press {
                x: 2,
                y: 40,
                width: 0,
                height: 0
            }],
            "the shell has to see where the press was, and the size it was in"
        );
        assert_eq!(
            clicks.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "a claimed press is not a body click: it must not dismiss the tools panel"
        );

        // Away from the edge the press is reported, not claimed: the caret moves.
        let lparam = (40usize << 16) | 60;
        let claimed = unsafe { claim(hwnd, WM_LBUTTONDOWN, 0, lparam as isize, &callbacks) };
        assert_eq!(claimed, None);
        assert_eq!(clicks.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_negative_press_coordinate_survives_the_word_packing() {
        // A press can arrive with the pointer outside the control (capture), and
        // the message packs x and y as *signed* 16-bit halves. Read as unsigned,
        // -3 would become 65533 and the band test would claim the far right edge.
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Press>::new()));
        let sink = seen.clone();
        let callbacks = Callbacks {
            on_escape: None,
            on_key: None,
            on_wheel: None,
            on_edit: None,
            on_click: None,
            on_press: Some(Box::new(move |press| {
                sink.lock().unwrap().push(press);
                false
            })),
            on_hover: None,
        };
        let lparam = ((0xFFFE_usize) << 16) | 0xFFFD;
        unsafe { claim(std::ptr::null_mut(), WM_LBUTTONDOWN, 0, lparam as isize, &callbacks) };
        assert_eq!(
            *seen.lock().unwrap(),
            vec![Press {
                x: -3,
                y: -2,
                width: 0,
                height: 0
            }]
        );
    }

    #[test]
    fn a_hover_in_the_band_advertises_the_grip_cursor() {
        // The band a press can start a resize in has to *look* like one, or the
        // grip exists only for people who already know about it. The callback
        // answers with a shape; claiming the message is what stops RichEdit from
        // setting its I-beam over the answer.
        let asked = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let sink = asked.clone();
        let callbacks = Callbacks {
            on_escape: None,
            on_key: None,
            on_wheel: None,
            on_edit: None,
            on_click: None,
            on_press: None,
            // Position is not the subject here -- `press_under` needs a real
            // window to convert screen coordinates, and a null one has no client
            // area to convert into.
            on_hover: Some(Box::new(move |_| {
                sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Some(Shape::Horizontal)
            })),
        };
        let hwnd = std::ptr::null_mut();

        let claimed = unsafe { claim(hwnd, WM_SETCURSOR, 0, HTCLIENT as isize, &callbacks) };
        assert_eq!(claimed, Some(1), "a band hover must claim the message");
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Outside the client area there is nothing to advertise: `WM_SETCURSOR`
        // arrives with the frame's hit-test code and must fall through untouched.
        let claimed = unsafe { claim(hwnd, WM_SETCURSOR, 0, 2, &callbacks) };
        assert_eq!(claimed, None);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn an_edit_is_reported_but_never_claimed() {
        // Both halves matter. Reported, or the debounce never starts and the save
        // waits for the 2 s scan; not claimed, or the character is swallowed --
        // `Some(0)` here would turn the note into a control that accepts no input.
        let count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let sink = count.clone();
        let callbacks = Callbacks {
            on_escape: None,
            on_key: None,
            on_wheel: None,
            on_edit: Some(Box::new(move || {
                sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })),
            on_click: None,
            on_press: None,
            on_hover: None,
        };
        let hwnd = std::ptr::null_mut();

        for message in [WM_CHAR, WM_IME_CHAR, WM_PASTE, WM_CUT, WM_CLEAR, WM_UNDO] {
            let claimed = unsafe { claim(hwnd, message, 0, 0, &callbacks) };
            assert_eq!(claimed, None, "message {message:#06x} was claimed");
        }
        // Backspace and Delete arrive as keys, and are edits too.
        for key in [VK_BACK as usize, VK_DELETE as usize] {
            let claimed = unsafe { claim(hwnd, WM_KEYDOWN, key, 0, &callbacks) };
            assert_eq!(claimed, None, "key {key:#04x} was claimed");
        }
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 8);

        // A key that only moves the caret is not an edit: reporting it would restart
        // the debounce on every arrow press and never save while the user scrolls.
        let claimed = unsafe { claim(hwnd, WM_KEYDOWN, 0x25, 0, &callbacks) };
        assert_eq!(claimed, None);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 8);
    }
}
