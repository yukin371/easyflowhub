//! Timing and pixel evidence for the text island.
//!
//! Two different questions are answered here, and keeping them in one module is
//! deliberate: both are "what did the control actually do", and both are cheap to
//! run on the parked window (no need to put anything on screen).
//!
//! * [`append_char`] splits one keystroke into `insert` (the control processes
//!   `WM_CHAR`) and `repaint` (an explicit `InvalidateRect` + `UpdateWindow`).
//! * [`probe_pixels`] renders the client area into a memory DC via
//!   `WM_PRINTCLIENT` and samples it, so "the dark theme applied" is a pixel
//!   count instead of an opinion.

use std::ptr;

use windows_sys::Win32::Foundation::{HWND, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmFlush;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel,
    ReleaseDC, SelectObject, UpdateWindow,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, SendMessageW, WM_CHAR};

use hub_core::PixelRect;

use crate::{now_us, print_client};

/// Cost of one character, split so a regression can be attributed.
///
/// `insert_us` is the natural path (`WM_CHAR`): RichEdit paints synchronously inside
/// that call, which is why the *natural* paint phase measures 0 - see the note on
/// [`append_char`]. `repaint_us` is a **forced full repaint** of the client area,
/// which is the conservative upper bound for "the character had to be drawn".
#[derive(Clone, Copy, Debug, Default)]
pub struct CharTiming {
    /// `WM_CHAR` handling: insertion, local reflow, and whatever it paints inline.
    pub insert_us: u64,
    /// `InvalidateRect` + `UpdateWindow`: an explicit full repaint.
    pub repaint_us: u64,
}

impl CharTiming {
    /// The gated number: natural handling plus a full repaint.
    ///
    /// Deliberately an upper bound. The inline paint inside `insert_us` and the
    /// forced repaint overlap, so this over-states the real cost - which is the
    /// right direction for a gate that must not claim success it cannot prove.
    ///
    /// Saturating, not `+`: QPC deltas are `u64`, and a wrapping add in a release
    /// build would report a slow keystroke as a fast one.
    pub fn gate_us(&self) -> u64 {
        self.insert_us.saturating_add(self.repaint_us)
    }
}

/// Insert one character through the same message the keyboard path sends.
///
/// `SendMessage` (not `PostMessage`) on purpose: it returns when the control has
/// processed the character, which is the boundary we want to time. The queue hop a
/// real keystroke adds is well under a millisecond and is declared out of scope in
/// the spike doc.
///
/// Measured fact: `UpdateWindow` right after `WM_CHAR` costs 0 us. RichEdit paints
/// inside the message, so there is no dirty region left to flush. That makes a
/// "insert vs paint" split meaningless on the natural path, and it is why the gated
/// number adds a *forced* repaint instead of pretending the flush measured a paint.
pub fn append_char(hwnd: HWND, ch: char) -> CharTiming {
    use windows_sys::Win32::Graphics::Gdi::InvalidateRect;

    let unit = ch as u32 as u16;

    let started = now_us();
    unsafe {
        SendMessageW(hwnd, WM_CHAR, unit as WPARAM, 1);
    }
    let inserted = now_us();

    unsafe {
        InvalidateRect(hwnd, ptr::null(), 0);
        UpdateWindow(hwnd);
    }
    let repainted = now_us();

    CharTiming {
        insert_us: inserted.saturating_sub(started),
        repaint_us: repainted.saturating_sub(inserted),
    }
}

/// Wait for the next composed frame and report how long that took.
///
/// Reported, never gated: the compositor runs at 60 Hz, so this value is
/// `paint + (0..16.7ms)` no matter how fast the control is. Treating it as a gate
/// would demand "faster than one refresh", which is not a property of the editor.
pub fn compose_us() -> u64 {
    let started = now_us();
    unsafe {
        DwmFlush();
    }
    now_us().saturating_sub(started)
}

#[derive(Clone, Debug, Default)]
pub struct PixelProbe {
    pub samples: usize,
    pub dominant: u32,
    pub dominant_share: f64,
    pub distinct: usize,
    /// Samples that match the configured text colour exactly.
    ///
    /// This is what turns "the text colour applied" into a check instead of an
    /// impression: an empty document must have none, a document with glyphs must
    /// have some. Antialiased edges do not count, but at 16.5 pt the interior of a
    /// CJK stroke is several pixels wide, so the exact colour does appear.
    pub fg_hits: usize,
}

impl PixelProbe {
    /// Fraction of sampled pixels that are neither the dominant colour nor close to
    /// it. Text glyphs are the only thing that should produce them.
    pub fn ink_share(&self) -> f64 {
        if self.samples == 0 {
            return 0.0;
        }
        1.0 - self.dominant_share
    }
}

/// Render the client area into an offscreen bitmap and sample it.
///
/// Step > 1 because `GetPixel` is a GDI call per sample; a 660x1290 island at step
/// 6 is ~23k samples, enough to see text and fast enough to run in the bench.
///
/// `foreground` is the `0x00BBGGRR` text colour the control was told to use, so the
/// probe measures "did the configured colour reach the surface" rather than "is the
/// surface not empty".
pub fn probe_pixels(hwnd: isize, rect: PixelRect, foreground: u32) -> PixelProbe {
    let hwnd = hwnd as HWND;
    let width = rect.width.max(1);
    let height = rect.height.max(1);

    unsafe {
        let screen = GetDC(ptr::null_mut());
        if screen.is_null() {
            return PixelProbe::default();
        }
        let memory = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        let previous = SelectObject(memory, bitmap as _);

        print_client(hwnd, memory as isize);

        let mut samples = Vec::new();
        let step = 6;
        let mut y = 0;
        while y < height {
            let mut x = 0;
            while x < width {
                let pixel = GetPixel(memory, x, y);
                if pixel != u32::MAX {
                    samples.push(pixel);
                }
                x += step;
            }
            y += step;
        }

        SelectObject(memory, previous);
        DeleteObject(bitmap as _);
        DeleteDC(memory);
        ReleaseDC(ptr::null_mut(), screen);

        classify(&samples, foreground)
    }
}

/// Histogram summary of sampled pixels. Pure, so the classification is testable
/// without a window.
pub fn classify(samples: &[u32], foreground: u32) -> PixelProbe {
    use std::collections::HashMap;

    if samples.is_empty() {
        return PixelProbe::default();
    }

    let mut counts: HashMap<u32, usize> = HashMap::new();
    for pixel in samples {
        *counts.entry(*pixel).or_insert(0) += 1;
    }

    let (dominant, count) = counts
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(pixel, count)| (*pixel, *count))
        .unwrap_or((0, 0));

    PixelProbe {
        samples: samples.len(),
        dominant,
        dominant_share: count as f64 / samples.len() as f64,
        distinct: counts.len(),
        fg_hits: samples.iter().filter(|pixel| **pixel == foreground).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_colour_wins_on_a_flat_surface() {
        let probe = classify(&[0x0010_1010; 90], 0x00FF_FFFF);
        assert_eq!(probe.samples, 90);
        assert_eq!(probe.dominant, 0x0010_1010);
        assert!((probe.dominant_share - 1.0).abs() < f64::EPSILON);
        assert_eq!(probe.distinct, 1);
        assert_eq!(probe.fg_hits, 0);
        assert!(probe.ink_share() < f64::EPSILON);
    }

    #[test]
    fn ink_share_counts_pixels_that_are_not_background() {
        // 8 background + 2 glyph pixels: the glyphs are the only "ink".
        let mut samples = vec![0x0010_1010; 8];
        samples.extend([0x00E0_E0E0, 0x00CC_CCCC]);
        let probe = classify(&samples, 0x00E0_E0E0);
        assert_eq!(probe.dominant, 0x0010_1010);
        assert!((probe.ink_share() - 0.2).abs() < 1e-9);
        assert_eq!(probe.distinct, 3);
        // Exactly one sample is the configured text colour: A4's "a pixel of the
        // text colour exists" is decided by this count, not by distinct > 1.
        assert_eq!(probe.fg_hits, 1);
    }

    #[test]
    fn foreground_hits_need_an_exact_match() {
        // Near-misses are antialiasing, not the text colour. Counting them would
        // make "the colour applied" pass on a screenshot of a blurred edge.
        let probe = classify(&[0x0010_1010, 0x00F7_EEE8, 0x00F6_EDE7], 0x00F7_EEE8);
        assert_eq!(probe.fg_hits, 1);
    }

    #[test]
    fn empty_surface_reports_zero_rather_than_dividing_by_zero() {
        let probe = classify(&[], 0x00FF_FFFF);
        assert_eq!(probe.samples, 0);
        assert_eq!(probe.fg_hits, 0);
        assert_eq!(probe.ink_share(), 0.0);
    }

    #[test]
    fn timing_total_is_the_sum_of_the_two_phases() {
        let timing = CharTiming {
            insert_us: 900,
            repaint_us: 2_400,
        };
        assert_eq!(timing.gate_us(), 3_300);
    }

    #[test]
    fn gated_total_saturates_instead_of_wrapping() {
        // Both halves come from QPC deltas, so a hostile clock must not turn a
        // slow frame into a fast one through integer overflow.
        let timing = CharTiming {
            insert_us: u64::MAX,
            repaint_us: 1,
        };
        assert_eq!(timing.gate_us(), u64::MAX);
    }
}

/// Post `count` characters and pump the thread queue until they are painted.
///
/// This is the throughput counterpart to [`append_char`]: `SendMessage` hides queue
/// starvation (it runs the control inline), while a real burst can reveal a message
/// loop that never gets to paint. Pumping here is safe because we are on the UI
/// thread that owns both windows, and the peek is filtered to `hwnd` so the shell's
/// own messages are left for winit.
pub fn burst_chars(hwnd: isize, text: &str, count: usize) -> u64 {
    let hwnd = hwnd as HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_CHAR,
    };

    let units: Vec<u16> = text.encode_utf16().collect();
    if units.is_empty() || count == 0 {
        return 0;
    }

    let started = now_us();
    for index in 0..count {
        unsafe {
            PostMessageW(hwnd, WM_CHAR, units[index % units.len()] as WPARAM, 1);
        }
    }

    // Bounded: a control that keeps queueing messages to itself must not hang the
    // bench. 4x the posted count is far above what RichEdit emits per character.
    let mut guard = count * 4 + 100;
    loop {
        let mut message: MSG = unsafe { std::mem::zeroed() };
        let found = unsafe { PeekMessageW(&mut message, hwnd, 0, 0, PM_REMOVE) };
        if found == 0 || guard == 0 {
            break;
        }
        guard -= 1;
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    unsafe {
        UpdateWindow(hwnd);
    }
    now_us().saturating_sub(started)
}
