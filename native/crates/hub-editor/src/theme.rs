//! `CHARFORMATW` / `PARAFORMAT2` and the values they carry.
//!
//! Split from `lib.rs` because the window code and these two structs have nothing
//! to say to each other: one owns an HWND, the other is a byte layout that has to
//! match `richedit.h` exactly. Keeping them together made the module drift past the
//! 800-line cap, which the architecture guard reports as debt — the split is the
//! cheap way to pay it rather than an exception.

use windows_sys::Win32::Foundation::COLORREF;

/// Applies the format to the whole document rather than the selection.
pub(crate) const SCF_ALL: usize = 4;

const CFM_COLOR: u32 = 0x4000_0000;
const CFM_SIZE: u32 = 0x8000_0000;
const CFM_FACE: u32 = 0x2000_0000;
const CFM_CHARSET: u32 = 0x0800_0000;
const DEFAULT_CHARSET: u8 = 1;

/// `PARAFORMAT2::dwMask` bit for line spacing.
pub(crate) const PFM_LINESPACING: u32 = 0x0000_0100;
/// `PARAFORMAT2::bLineSpacingRule`: `dyLineSpacing` is a multiple of single
/// spacing expressed in 1/20ths, i.e. 20 means 1.0.
pub(crate) const LINE_SPACING_MULTIPLE: u8 = 5;

/// `PARAFORMAT2`, 188 bytes. Not exported by windows-sys (richedit.h).
///
/// The layout must match the C struct field for field, including the `[i32; 32]`
/// tab table: RichEdit validates `cbSize` and silently ignores the message when it
/// does not recognise the size, which is the same failure mode as `CHARFORMATW`.
#[repr(C)]
pub(crate) struct ParaFormat2 {
    pub cb_size: u32,
    pub mask: u32,
    pub numbering: u16,
    pub effects: u16,
    pub start_indent: i32,
    pub right_indent: i32,
    pub offset: i32,
    pub alignment: u16,
    pub tab_count: i16,
    pub tabs: [i32; 32],
    pub space_before: i32,
    pub space_after: i32,
    pub line_spacing: i32,
    pub style: i16,
    pub line_spacing_rule: u8,
    pub outline_level: u8,
    pub shading_weight: u16,
    pub shading_style: u16,
    pub numbering_start: u16,
    pub numbering_style: u16,
    pub numbering_tab: u16,
    pub border_space: u16,
    pub border_width: u16,
    pub borders: u16,
}

/// `CHARFORMATW`, 92 bytes. `cb_size` must equal `size_of::<CharFormat>()` or the
/// control drops the whole message without reporting an error.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct CharFormat {
    cb_size: u32,
    mask: u32,
    effects: u32,
    height: i32,
    offset: i32,
    text_color: COLORREF,
    charset: u8,
    pitch_and_family: u8,
    face_name: [u16; 32],
}

impl CharFormat {
    pub(crate) fn new(theme: &Theme) -> Self {
        let mut format = Self {
            cb_size: std::mem::size_of::<Self>() as u32,
            mask: CFM_COLOR | CFM_SIZE | CFM_FACE | CFM_CHARSET,
            effects: 0,
            // Twips: 1/20 point. Writing pixels here yields a 0.5pt or 200pt font.
            height: (theme.font_pt * 20.0).round() as i32,
            offset: 0,
            text_color: theme.foreground,
            charset: DEFAULT_CHARSET,
            pitch_and_family: 0,
            face_name: [0; 32],
        };
        for (slot, unit) in format.face_name.iter_mut().zip(theme.font_face.encode_utf16()) {
            *slot = unit;
        }
        format
    }

    pub(crate) fn face_name(&self) -> String {
        let end = self
            .face_name
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(self.face_name.len());
        String::from_utf16_lossy(&self.face_name[..end])
    }

    pub(crate) fn height_twips(&self) -> i32 {
        self.height
    }

    pub(crate) fn text_color(&self) -> COLORREF {
        self.text_color
    }

    /// Reusable buffer for `EM_GETCHARFORMAT`, which needs a `cbSize` before it
    /// will write anything.
    pub(crate) fn blank() -> Self {
        Self {
            cb_size: std::mem::size_of::<Self>() as u32,
            mask: 0,
            effects: 0,
            height: 0,
            offset: 0,
            text_color: 0,
            charset: DEFAULT_CHARSET,
            pitch_and_family: 0,
            face_name: [0; 32],
        }
    }
}

/// Colours arrive as `0x00BBGGRR`, the order Windows uses everywhere. The app
/// converts from egui's RGB so this crate never sees a UI type.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub background: COLORREF,
    pub foreground: COLORREF,
    /// Owned rather than `&'static str`: the face comes from the user's settings
    /// file, and leaking it to satisfy a lifetime would make "changing the font"
    /// a one-shot operation.
    pub font_face: String,
    pub font_pt: f32,
    /// Multiple of the natural line height. `1.0` leaves RichEdit's own spacing.
    pub line_spacing: f32,
    /// Inset between the island edge and the text, **physical** pixels.
    ///
    /// Without it a wrapped line starts at x = 0 and the caret's first column is
    /// clipped by the control edge: the caret looks like it vanished on a line
    /// break and reappears as soon as a character pushes it inward.
    pub padding: (i32, i32),
    /// Caret thickness, physical pixels. `0` leaves the system value alone.
    ///
    /// Windows has no per-window caret width — RichEdit builds its caret from
    /// `SPI_GETCARETWIDTH` and offers no message to override it — so a non-zero
    /// value is written **system-wide** and the previous one is restored on drop.
    /// The caret's *colour* has no API at all: it is an XOR inversion of whatever
    /// is underneath, which is why "make it blue" needs a self-drawn caret rather
    /// than a setting.
    pub caret_width: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn para_format2_matches_the_richedit_size() {
        // RichEdit rejects a `PARAFORMAT2` whose `cbSize` is not 188 without
        // returning an error; a padding mistake in the struct would show up as
        // "line spacing has no effect", not as a failed build.
        assert_eq!(std::mem::size_of::<ParaFormat2>(), 188);
        assert_eq!(std::mem::size_of::<CharFormat>(), 92);
    }

    #[test]
    fn line_spacing_is_expressed_in_twentieths() {
        // 1.35 -> 27; rounding matters because 1.35 * 20 is 27.000000000000004.
        assert_eq!((1.35f32 * 20.0).round() as i32, 27);
        assert_eq!((1.0f32 * 20.0).round() as i32, 20);
    }

    #[test]
    fn the_point_size_is_converted_to_twips_not_pixels() {
        let theme = Theme {
            background: 0,
            foreground: 0x00EE_EEEE,
            font_face: "Microsoft YaHei UI".to_string(),
            font_pt: 11.5,
            line_spacing: 1.0,
            padding: (0, 0),
            caret_width: 0,
        };
        let format = CharFormat::new(&theme);
        assert_eq!(format.height_twips(), 230); // 11.5 * 20
        assert_eq!(format.text_color(), 0x00EE_EEEE);
        assert_eq!(format.face_name(), "Microsoft YaHei UI");
    }
}
