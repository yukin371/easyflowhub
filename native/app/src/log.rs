//! Timing log.
//!
//! Format is relative time (`t=123.456ms` since process start) plus a stable
//! `seq` and a fixed event name, because the measurement wrapper greps for those
//! three things. No wall clock: correlating with the Windows event log is not part
//! of any acceptance item, and formatting `SystemTime` without pulling in `chrono`
//! would be the only reason to add a dependency.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

pub struct Log {
    path: PathBuf,
    start: Instant,
}

impl Log {
    pub fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let log = Self {
            path,
            start: Instant::now(),
        };
        let _ = std::fs::remove_file(&log.path);
        log
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// `seq=0` for startup lines, the reveal index otherwise.
    pub fn mark(&self, seq: u64, event: &str, detail: &str) {
        let elapsed = self.start.elapsed().as_secs_f64() * 1000.0;
        let line = if detail.is_empty() {
            format!("t={elapsed:09.3}ms seq={seq:04} {event}\n")
        } else {
            format!("t={elapsed:09.3}ms seq={seq:04} {event} {detail}\n")
        };

        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = file.write_all(line.as_bytes());
        }
        eprint!("{line}");
    }
}
