//! A scripted descent draws no level pop (TEST-072). Tier A, no GPU: the drawn height of fixed probes over a synthetic height pyramid, through the morph model
//! of `LodParams` (LOD-008), judged by `popcheck`; the same descent without morphing is the planted failure the detector must catch.

use planet_frame::popcheck::{judge, Sample, Verdict};
use planet_frame::LodParams;

const PROBES: usize = 32;
/// The descent: 3000 km down to 2 m, 2 % a frame.
const START_M: f64 = 3_000_000.0;
const END_M: f64 = 2.0;
const RATIO: f64 = 0.98;

fn lod() -> LodParams {
    LodParams {
        cells: 16,
        viewport_h_px: 360.0,
        tau_px: 8.0,
        max_level: 16,
        min_level: 0,
        max_height_m: 9_000.0,
        ..LodParams::earth_1080p()
    }
}

/// The level a node is drawn at over a vertex at `d` metres: the first level that does not split there (LOD-008, the node of `level` is drawn from
/// its own split distance up to its parent's).
fn level_at(lod: &LodParams, d: f64) -> u8 {
    (0..=lod.max_level).find(|&l| d >= lod.split_distance(l)).unwrap_or(lod.max_level)
}

/// The height the pyramid gives probe `p` at `level`: each level adds detail of half its sample spacing (rugged ground: a slope of 0.5 at the scale of the level).
fn pyramid(lod: &LodParams, p: usize, level: u8) -> f64 {
    (1..=level).map(|k| 0.5 * lod.sample_spacing(k) * libm::sin(1.7 * f64::from(k) + 0.37 * p as f64)).sum()
}

/// The height a node of `level` draws over a vertex at `d`: its own grid at the start of its morph interval, its parent's at the end.
fn drawn(lod: &LodParams, p: usize, level: u8, d: f64, morph: bool) -> f64 {
    let own = pyramid(lod, p, level);
    if level == 0 || !morph {
        return own;
    }
    let (start, end) = lod.morph_interval(level);
    let m = ((d - start) / (end - start)).clamp(0.0, 1.0);
    (1.0 - m) * own + m * pyramid(lod, p, level - 1)
}

fn descent(morph: bool) -> (Vec<Vec<Option<Sample>>>, f64) {
    let lod = lod();
    let focal_px = lod.viewport_h_px / (2.0 * libm::tan(lod.fov_y_rad / 2.0));
    let mut trace = Vec::new();
    let mut alt = START_M;
    while alt > END_M {
        let row = (0..PROBES)
            .map(|p| {
                let d = libm::hypot(alt, 20.0 * p as f64);
                let level = level_at(&lod, d);
                Some(Sample { level, height_m: drawn(&lod, p, level, d, morph), distance_m: d })
            })
            .collect();
        trace.push(row);
        alt *= RATIO;
    }
    (trace, focal_px)
}

// spec: TEST-072
#[test]
fn a_descent_through_the_morph_draws_no_level_pop() {
    let (trace, focal_px) = descent(true);
    let v = judge(&trace, focal_px);
    assert!(matches!(v, Verdict::Pass { .. }), "{}", v.line());
}

// spec: TEST-072
#[test]
fn the_same_descent_without_morphing_fails_naming_a_frame_probe_and_levels() {
    let (trace, focal_px) = descent(false);
    let v = judge(&trace, focal_px);
    match v {
        Verdict::Fail { changes, p99_px, worst } => {
            assert!(
                changes >= 10 && worst.level_before != worst.level_after && p99_px > planet_frame::popcheck::P99_LIMIT_PX,
                "{}",
                v.line()
            );
        }
        other => panic!("a hard switch must fail: {}", other.line()),
    }
}
