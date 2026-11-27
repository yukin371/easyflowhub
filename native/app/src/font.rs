//! Where the CJK font comes from.
//!
//! ## The one decision
//!
//! The font is mapped, not read. `hub_ui::theme::install_owned` hands epaint a
//! `Vec<u8>`; epaint stores it as a `Cow::Owned` and then clones it again for
//! `ab_glyph::FontVec`, so `msyh.ttc` (19.7 MB) is charged to the process at
//! least once, twice at startup. `install_mapped` instead takes a `&'static [u8]`
//! pointing at a read-only file mapping: `FontRef`, no copies, pages shared with
//! every other process using the same font and evictable when nothing needs them.
//!
//! Reading the file is kept as a **fallback and as a baseline**, not as a
//! second product path:
//!
//! * fallback -- a mapping can fail (a locked or replaced font file), and a note
//!   that renders tofu because of that would be a worse bug than 19 MB;
//! * baseline -- `--font-heap` forces the old path so the C1 gate can compare the
//!   same face, the same glyphs and the same window, differing only in how the
//!   bytes are held. Without that control the gate would only prove "we changed
//!   something", which is what the 130 MB number did not survive either.
//!
//! ## What is logged, and why in one line
//!
//! `fonts cjk=<path> index=<n> mode=mapped|heap bytes=<n>` -- the path says which
//! candidate won, `mode` says whether the win32 call actually ran, and `bytes`
//! says how much the process is *not* paying for. When C1 is re-measured, that
//! single line is the difference between "the font was skipped" and "the font was
//! loaded the expensive way", which otherwise look identical from outside.

use hub_ui::theme::{install_mapped, install_owned, CJK_CANDIDATES};

use crate::log::Log;

/// How the font's bytes are held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Read-only file mapping, `ab_glyph::FontRef`, no copy.
    Mapped,
    /// Owned `Vec<u8>` in the heap, `ab_glyph::FontVec`.
    Heap,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Mapped => "mapped",
            Mode::Heap => "heap",
        }
    }
}

/// What a successful install did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub path: String,
    pub index: u32,
    pub bytes: u64,
    pub mode: Mode,
    /// Why a mapping was skipped, when it was. Empty for a mapped install.
    pub note: String,
}

/// Install the first candidate that loads.
///
/// `heap` forces the owned path (`--font-heap`), which exists so the gate has a
/// control run rather than a remembered number.
pub fn install(ctx: &egui::Context, heap: bool) -> Option<Installed> {
    let mut notes: Vec<String> = Vec::new();

    for path in CJK_CANDIDATES {
        // The TTC's first face is Microsoft YaHei, which is the face the Tauri
        // build rendered with (`font_face: "Microsoft YaHei UI"` in the spike).
        let index = 0;

        if !heap {
            match hub_win::font::map(std::path::Path::new(path)) {
                Ok(bytes) => {
                    install_mapped(ctx, bytes, index);
                    return Some(Installed {
                        path: path.to_string(),
                        index,
                        bytes: bytes.len() as u64,
                        mode: Mode::Mapped,
                        note: String::new(),
                    });
                }
                Err(error) => notes.push(format!("{} {error}", file_name(path))),
            }
        }

        match std::fs::read(path) {
            Ok(bytes) => {
                let size = bytes.len() as u64;
                install_owned(ctx, bytes, index);
                return Some(Installed {
                    path: path.to_string(),
                    index,
                    bytes: size,
                    mode: Mode::Heap,
                    note: if heap {
                        "forced by --font-heap".to_string()
                    } else {
                        format!("fallback: {}", notes.join("; "))
                    },
                });
            }
            Err(error) => notes.push(format!("{} read={}", file_name(path), error.raw_os_error().unwrap_or(0))),
        }
    }

    None
}

/// Install the font and write the one line that explains which path ran.
///
/// Both entry points (the note controller and the spike panel) call this, so the
/// message a gate greps for is produced in exactly one place -- the previous
/// version had two copies that had already drifted in wording.
pub fn install_and_log(ctx: &egui::Context, log: &Log, skip: bool, heap: bool) {
    if skip {
        log.mark(0, "fonts", "skipped (--no-font)");
        return;
    }
    match install(ctx, heap) {
        Some(font) => log.mark(
            0,
            "fonts",
            &format!(
                "cjk={} index={} mode={} bytes={}{}",
                font.path,
                font.index,
                font.mode.as_str(),
                font.bytes,
                if font.note.is_empty() {
                    String::new()
                } else {
                    format!(" note=\"{}\"", font.note)
                }
            ),
        ),
        None => log.mark(0, "fonts", "no CJK font found; text renders as tofu"),
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_print_the_token_the_gate_greps() {
        assert_eq!(Mode::Mapped.as_str(), "mapped");
        assert_eq!(Mode::Heap.as_str(), "heap");
    }

    #[test]
    fn a_path_without_a_directory_is_its_own_name() {
        assert_eq!(file_name("msyh.ttc"), "msyh.ttc");
        assert_eq!(file_name(r"C:\Windows\Fonts\msyh.ttc"), "msyh.ttc");
    }

    #[test]
    fn every_candidate_is_a_ttf_or_ttc() {
        // A candidate that is not a font file would map fine and then panic inside
        // epaint's parser, where the reason is no longer visible.
        for path in CJK_CANDIDATES {
            assert!(
                path.ends_with(".ttc") || path.ends_with(".ttf"),
                "{path} is not a font file"
            );
        }
    }
}
