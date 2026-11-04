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

// SPIKE-99 (the relief driver k; spike branch only, never merged).
static K_ONE: AtomicBool = AtomicBool::new(false);
static UPLIFT: std::sync::Mutex<Option<std::sync::Arc<Vec<f64>>>> = std::sync::Mutex::new(None);

/// The base reads k = 1 everywhere (the hill band at its k = 1 amplitude) instead of the class-blended k.
pub fn k_one() -> bool {
    K_ONE.load(Ordering::Relaxed)
}
pub fn set_k_one(v: bool) {
    K_ONE.store(v, Ordering::Relaxed);
}
/// A per-cell uplift of a carve, in metres over the whole run (spread evenly over its steps), added to the carve's own constant uplift. A carve uses it only when its grid has
/// exactly this many cells; the example sets it before pass 1 and clears it before pass 2.
pub fn uplift() -> Option<std::sync::Arc<Vec<f64>>> {
    UPLIFT.lock().expect("uplift lock").clone()
}
pub fn set_uplift(v: Option<Vec<f64>>) {
    *UPLIFT.lock().expect("uplift lock") = v.map(std::sync::Arc::new);
}

// SPIKE-101 (high-mountain relief; spike branch only, never merged). f64 values as bits.
static HM_BAND_SCALE: AtomicU64 = AtomicU64::new(0x3FF0_0000_0000_0000); // 1.0
static HM_RIDGED: AtomicBool = AtomicBool::new(false);
static HM_MID_AMP: AtomicU64 = AtomicU64::new(0); // 0.0 m
static HM_PARTS: AtomicU32 = AtomicU32::new(0);

/// The hill band's amplitude times this where the class is mountains or high mountains (blended by their weight).
pub fn hm_band_scale() -> f64 {
    f64::from_bits(HM_BAND_SCALE.load(Ordering::Relaxed))
}
pub fn set_hm_band_scale(v: f64) {
    HM_BAND_SCALE.store(v.to_bits(), Ordering::Relaxed);
}
/// The band's noise is a ridged multifractal (16 km to 1 km) where the class is mountains or high mountains, blended by their weight.
pub fn hm_ridged() -> bool {
    HM_RIDGED.load(Ordering::Relaxed)
}
pub fn set_hm_ridged(v: bool) {
    HM_RIDGED.store(v, Ordering::Relaxed);
}
/// Metres of mid-scale (80 km down to 20 km) ridges added to the base inside the belt core (times the core).
pub fn hm_mid_amp() -> f64 {
    f64::from_bits(HM_MID_AMP.load(Ordering::Relaxed))
}
pub fn set_hm_mid_amp(v: f64) {
    HM_MID_AMP.store(v.to_bits(), Ordering::Relaxed);
}
/// Decomposition: 0 the full base, 1 the base without the band, 2 the band alone (over land, plus 1,000 m).
pub fn hm_parts() -> u32 {
    HM_PARTS.load(Ordering::Relaxed)
}
pub fn set_hm_parts(v: u32) {
    HM_PARTS.store(v, Ordering::Relaxed);
}

static HM_CENTRE: AtomicU64 = AtomicU64::new(0); // 0.0: no centring
/// The band's noise minus this (its mean, about 0.45) where the class is mountains or high mountains, so a larger band does not lift the median.
pub fn hm_centre() -> f64 {
    f64::from_bits(HM_CENTRE.load(Ordering::Relaxed))
}
pub fn set_hm_centre(v: f64) {
    HM_CENTRE.store(v.to_bits(), Ordering::Relaxed);
}

// SPIKE-102 (archetypes; spike branch only, never merged).
static KEEP_PITS: AtomicBool = AtomicBool::new(false);
static KEPT_PIT_CELLS: AtomicU64 = AtomicU64::new(0);
/// The carve routes over the flooded copy but does not raise a cell that lies below its receiver (a depression): the pit keeps its height plus the uplift, no fill.
pub fn keep_pits() -> bool {
    KEEP_PITS.load(Ordering::Relaxed)
}
pub fn set_keep_pits(v: bool) {
    KEEP_PITS.store(v, Ordering::Relaxed);
}
/// Cells left below their receiver in the last step of the last carve with `keep_pits` (a count, for the log).
pub fn kept_pit_cells() -> u64 {
    KEPT_PIT_CELLS.load(Ordering::Relaxed)
}
pub fn set_kept_pit_cells(v: u64) {
    KEPT_PIT_CELLS.store(v, Ordering::Relaxed);
}
