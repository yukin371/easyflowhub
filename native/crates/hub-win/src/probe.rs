//! Minimal timing probes for the gate: process footprint and session type.
//!
//! Kept alongside the window layer because both answer "is this environment the
//! one we design for": a remote session must degrade to `lite`, and a leak check
//! needs Private Bytes and handle count from inside the process being measured.

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION};

/// One snapshot of this process.
///
/// Three memory numbers, because "how much does it use" has three answers and
/// they move independently once a file is mapped:
///
/// * [`Self::commit_bytes`] -- private commit. What the process asks the system
///   to guarantee; mapped file pages are **not** part of it.
/// * [`Self::private_ws_bytes`] -- resident *and* private. Excludes file-backed
///   pages other processes could share.
/// * [`Self::working_set_bytes`] -- everything resident, including the mapped
///   font pages this process touched.
///
/// The C1 gate needs all three: after `hub_win::font::map`, the font's 19.7 MB
/// leaves commit/private while (some of) it stays resident. A single number would
/// make that look like a measurement error.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessProbe {
    /// `PagefileUsage`, kept as `private_bytes` so S1/S2 numbers stay comparable.
    pub private_bytes: u64,
    pub commit_bytes: u64,
    pub private_ws_bytes: u64,
    pub working_set_bytes: u64,
    pub handles: u32,
    pub threads: u32,
}

impl ProcessProbe {
    pub fn capture() -> Self {
        let process: HANDLE = unsafe { GetCurrentProcess() };

        // EX2 is the only counters struct that carries `PrivateWorkingSetSize`.
        // It is accepted by `GetProcessMemoryInfo` on Win10+; if a future build
        // rejects it, the plain struct still answers the two numbers S1 recorded.
        let mut counters = PROCESS_MEMORY_COUNTERS_EX2::default();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
        // `GetProcessMemoryInfo` is declared against the plain struct; EX2 is a
        // strict prefix-compatible extension of it, which is exactly how the API
        // documents it ("cb" tells the API how much of the struct to fill).
        let mut memory_ok = unsafe {
            GetProcessMemoryInfo(
                process,
                &mut counters as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
                counters.cb,
            )
        } != 0;
        let mut private_ws = counters.PrivateWorkingSetSize as u64;

        if !memory_ok {
            let mut plain = PROCESS_MEMORY_COUNTERS::default();
            plain.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            memory_ok = unsafe { GetProcessMemoryInfo(process, &mut plain, plain.cb) } != 0;
            counters.WorkingSetSize = plain.WorkingSetSize;
            counters.PagefileUsage = plain.PagefileUsage;
            counters.PrivateUsage = plain.PagefileUsage;
            private_ws = 0;
        }

        let mut handles = 0u32;
        let handles_ok = unsafe { GetProcessHandleCount(process, &mut handles) } != 0;

        Self {
            private_bytes: if memory_ok {
                counters.PagefileUsage as u64
            } else {
                0
            },
            commit_bytes: if memory_ok {
                counters.PrivateUsage as u64
            } else {
                0
            },
            private_ws_bytes: private_ws,
            working_set_bytes: if memory_ok {
                counters.WorkingSetSize as u64
            } else {
                0
            },
            handles: if handles_ok { handles } else { 0 },
            threads: 0,
        }
    }

    pub fn private_mb(&self) -> f64 {
        self.private_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn working_set_mb(&self) -> f64 {
        self.working_set_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn private_ws_mb(&self) -> f64 {
        self.private_ws_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn commit_mb(&self) -> f64 {
        self.commit_bytes as f64 / (1024.0 * 1024.0)
    }
}

/// Total CPU time consumed by this process, in milliseconds.
///
/// Used to prove the idle claim: once warm, a hidden panel must not repaint, so
/// CPU time between two probes ten seconds apart should not move. That is the
/// difference between "prewarmed" and "polling".
pub fn cpu_time_ms() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let process = unsafe { GetCurrentProcess() };
    let ok = unsafe {
        GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user)
    };
    if ok == 0 {
        return 0.0;
    }
    let to_ms = |ft: FILETIME| {
        ((ft.dwHighDateTime as u64) << 32 | ft.dwLowDateTime as u64) as f64 / 10_000.0
    };
    to_ms(kernel) + to_ms(user)
}

/// True when running over RDP or another remote session.
///
/// §5.1/`lite`: remote sessions get no shadow, no fade and a narrower glyph set,
/// because every extra composite frame is paid on the wire.
pub fn is_remote_session() -> bool {
    unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 }
}
