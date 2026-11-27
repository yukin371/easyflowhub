//! Footprint snapshot used by `--probe` and by the latency report.

use std::sync::atomic::{AtomicU64, Ordering};

use hub_win::ProcessProbe;

/// Glyph-atlas size as of the last frame, packed `width << 32 | height`.
///
/// The probe thread cannot touch the egui `Context` (it is owned by the event
/// loop), so the render thread deposits the number here every frame and the
/// snapshot reads it. C1 needs it: "the font got cheaper" and "the atlas got
/// bigger" would otherwise be one indistinguishable delta.
static ATLAS: AtomicU64 = AtomicU64::new(0);

pub fn record_atlas(ctx: &egui::Context) {
    let [width, height] = ctx.fonts(|fonts| fonts.font_image_size());
    ATLAS.store(((width as u64) << 32) | height as u64, Ordering::Relaxed);
}

pub fn atlas_size() -> (u32, u32) {
    let packed = ATLAS.load(Ordering::Relaxed);
    ((packed >> 32) as u32, (packed & 0xffff_ffff) as u32)
}

#[derive(Debug, Clone, Copy)]
pub struct ProbeJson {
    /// Commit charge (`PagefileUsage`): what the process asks the system to back.
    pub private_mb: f64,
    /// Glyph atlas in texels, and what it would cost as `R8`.
    pub atlas_w: u32,
    pub atlas_h: u32,
    /// Private working set: resident and not shared.
    pub private_ws_mb: f64,
    /// Everything resident, including mapped file pages this process touched.
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
            atlas_w: atlas_size().0,
            atlas_h: atlas_size().1,
            private_ws_mb: probe.private_ws_mb(),
            working_set_mb: probe.working_set_mb(),
            handles: probe.handles,
            cpu_ms: hub_win::probe::cpu_time_ms(),
            idle_cpu_ms,
        }
    }
}

