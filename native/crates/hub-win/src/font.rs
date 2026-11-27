//! Read-only file mapping for the font file.
//!
//! ## Why the bytes must not be owned
//!
//! epaint keeps font data in a `Cow<'static, [u8]>` and builds the glyph backend
//! from it (`text/fonts.rs`, `ab_glyph_font_from_font_data`):
//!
//! * `Cow::Borrowed` -> `FontRef`, which reads the glyph tables straight out of
//!   the buffer. Nothing is copied.
//! * `Cow::Owned` -> `FontVec`, constructed from `bytes.clone()` -- a **second**
//!   full copy of the file, on top of the `Vec` that was just handed in.
//!
//! `msyh.ttc` is 19.7 MB, and the shell loads it at startup, so the owned path
//! charges roughly twice that to the process and keeps one copy for its lifetime.
//! Mapping the system file and handing epaint a `&'static [u8]` turns the same
//! bytes into file-backed pages: no copy is made, the pages are shared with every
//! other process that mapped the file, and ones no glyph touched are never read.
//!
//! ## Why the mapping is never released
//!
//! `FontData::from_static` requires `'static`, and the font outlives every window
//! (it lives on the egui `Context`). Unmapping while a `FontRef` still points into
//! the view would be a use-after-free, so the view is intentionally left for
//! process exit. The file and section handles *are* closed right after mapping:
//! a view keeps its own reference to the section, and leaving handles open would
//! show up in the handle-stability gate as a leak.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, GetFileSizeEx, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_DELETE,
    FILE_SHARE_READ, OPEN_EXISTING,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, PAGE_READONLY,
};

/// Which win32 call refused, and with what code.
///
/// Kept as a code rather than a message because the caller logs it: "open=2" is
/// greppable, and turns "no CJK text" into "the file was not there" without
/// anybody having to reproduce it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapError {
    pub stage: &'static str,
    pub code: u32,
}

impl MapError {
    fn last(stage: &'static str) -> Self {
        Self {
            stage,
            code: std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32,
        }
    }
}

impl std::fmt::Display for MapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}={}", self.stage, self.code)
    }
}

/// Map `path` read-only and return it for the rest of the process.
///
/// An empty file is rejected: `CreateFileMappingW` maps empty files by extending
/// them, and a zero-length font would parse as "corrupt" much later, in epaint's
/// panic path, where the reason is no longer visible.
pub fn map(path: &Path) -> Result<&'static [u8], MapError> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

    // `FILE_SHARE_DELETE` matters: Windows Update and font installers replace
    // these files while processes have them open, and without it the mapping
    // turns a routine font update into a "file in use" dialog.
    let file = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if file == INVALID_HANDLE_VALUE {
        return Err(MapError::last("open"));
    }

    let mut size: i64 = 0;
    if unsafe { GetFileSizeEx(file, &mut size) } == 0 {
        unsafe { CloseHandle(file) };
        return Err(MapError::last("size"));
    }
    if size <= 0 {
        unsafe { CloseHandle(file) };
        return Err(MapError {
            stage: "size",
            code: 0,
        });
    }

    let section = unsafe {
        CreateFileMappingW(
            file,
            std::ptr::null(),
            PAGE_READONLY,
            0,
            0,
            std::ptr::null(),
        )
    };
    // The section holds its own reference to the file, so the file handle is not
    // needed past this point -- and a mapped view keeps the section alive after
    // its handle is closed.
    unsafe { CloseHandle(file) };
    if section.is_null() {
        return Err(MapError::last("section"));
    }

    let view = unsafe { MapViewOfFile(section, FILE_MAP_READ, 0, 0, 0) };
    unsafe { CloseHandle(section) };
    if view.Value.is_null() {
        return Err(MapError::last("view"));
    }

    // SAFETY: the view covers `size` readable bytes and is never unmapped, so the
    // slice outlives every reader. Nothing else writes to a `PAGE_READONLY` view.
    Ok(unsafe { std::slice::from_raw_parts(view.Value as *const u8, size as usize) })
}

/// Drop a view created by [`map`]. Only tests call this: the product keeps its
/// font mapped for the process lifetime, and unmapping under a live `FontRef`
/// would be unsound.
#[doc(hidden)]
pub fn unmap(bytes: &'static [u8]) -> bool {
    let address = windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
        Value: bytes.as_ptr() as *mut std::ffi::c_void,
    };
    unsafe { UnmapViewOfFile(address) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_a_real_file_byte_for_byte() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let expected = std::fs::read(&path).expect("read the manifest");
        let mapped = map(&path).expect("map the manifest");
        assert_eq!(mapped, expected.as_slice());
        unmap(mapped);
    }

    #[test]
    fn reports_which_stage_failed() {
        let error = map(Path::new(r"C:\Windows\Fonts\definitely-not-a-font.ttf"))
            .expect_err("a missing file cannot be mapped");
        assert_eq!(error.stage, "open");
        assert_ne!(error.code, 0);
        assert!(error.to_string().starts_with("open="));
    }
}
