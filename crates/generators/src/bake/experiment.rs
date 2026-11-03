//! Experiment switches for the staircase diagnosis (not part of any layer: a diagnosis branch only, never merged). Process-wide, set by the example before a run.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

static JITTER_AMP: AtomicU64 = AtomicU64::new(0x3FE0_0000_0000_0000); // 0.5
static MFD_AREA: AtomicBool = AtomicBool::new(false);
static SMOOTH_FLOW: AtomicU32 = AtomicU32::new(0);
static SMOOTH_ISO: AtomicU32 = AtomicU32::new(0);
static C1_FACE_READ: AtomicBool = AtomicBool::new(false);

pub fn jitter_amp() -> f64 {
    f64::from_bits(JITTER_AMP.load(Ordering::Relaxed))
}
pub fn set_jitter_amp(v: f64) {
    JITTER_AMP.store(v.to_bits(), Ordering::Relaxed);
}
pub fn mfd_area() -> bool {
    MFD_AREA.load(Ordering::Relaxed)
}
pub fn set_mfd_area(v: bool) {
    MFD_AREA.store(v, Ordering::Relaxed);
}
/// Passes of 1-2-1 smoothing of the change along the carve's flow tree.
pub fn smooth_flow() -> u32 {
    SMOOTH_FLOW.load(Ordering::Relaxed)
}
pub fn set_smooth_flow(v: u32) {
    SMOOTH_FLOW.store(v, Ordering::Relaxed);
}
/// Passes of 3 x 3 binomial smoothing of the change.
pub fn smooth_iso() -> u32 {
    SMOOTH_ISO.load(Ordering::Relaxed)
}
pub fn set_smooth_iso(v: u32) {
    SMOOTH_ISO.store(v, Ordering::Relaxed);
}

/// Reads the pass-1 face change with Catmull-Rom (C1) instead of bilinear.
pub fn c1_face_read() -> bool {
    C1_FACE_READ.load(Ordering::Relaxed)
}
pub fn set_c1_face_read(v: bool) {
    C1_FACE_READ.store(v, Ordering::Relaxed);
}
