//! Footprint snapshot used by `--probe` and by the latency report.

use hub_win::ProcessProbe;

#[derive(Debug, Clone, Copy)]
pub struct ProbeJson {
    pub private_mb: f64,
    pub working_set_mb: f64,
    pub handles: u32,
    pub cpu_ms: f64,
    pub idle_cpu_ms: f64,
}

impl ProbeJson {
    pub fn capture(idle_cpu_ms: f64) -> Self {
        let probe = ProcessProbe::capture();
        Self {
            private_mb: probe.private_mb(),
            working_set_mb: probe.working_set_mb(),
            handles: probe.handles,
            cpu_ms: hub_win::probe::cpu_time_ms(),
            idle_cpu_ms,
        }
    }
}

