//! Minimal timing probes for the gate: process footprint and session type.
//!
//! Kept alongside the window layer because both answer "is this environment the
//! one we design for": a remote session must degrade to `lite`, and a leak check
//! needs Private Bytes and handle count from inside the process being measured.

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION};

/// One snapshot of this process.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessProbe {
    pub private_bytes: u64,
    pub working_set_bytes: u64,
    pub handles: u32,
    pub threads: u32,
}

impl ProcessProbe {
    pub fn capture() -> Self {
        let process: HANDLE = unsafe { GetCurrentProcess() };

        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let memory_ok =
            unsafe { GetProcessMemoryInfo(process, &mut counters, counters.cb) } != 0;

        let mut handles = 0u32;
        let handles_ok = unsafe { GetProcessHandleCount(process, &mut handles) } != 0;

        Self {
            private_bytes: if memory_ok {
                counters.PagefileUsage as u64
            } else {
                0
            },
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
