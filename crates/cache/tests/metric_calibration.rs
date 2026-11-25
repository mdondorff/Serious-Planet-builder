//! TEST-155 and BUILD-069: the geometric metrics' calibration harness on synthetic valleys and on S0.
use std::path::Path;

use planet_cache::crc32c::crc32c;
use planet_cache::metric_calibration::{
    calibrate, carve, for_each_case, judge, registered, reported, Background, Case, Floor, GeometricMetric, Profile, Reading, Verdict,
    XdiagInZone, ANGLES_DEG, DEPTH_M, FLOORS, N, OFFSET_CELLS, PROFILES, SAW_HALF_WIDTH, SAW_M, ZONE_CELLS,
};
use planet_generators::bake::Grid;

/// R1h's S0 crop (TEST-155's Detail): 256 x 256 little-endian f32.
const S0: &[u8] = include_bytes!("support/s0-r1h-river-256.f32");
const S0_CRC32C: u32 = 0x91ca_d068;

fn s0() -> Background {
    Background::from_f32_le("s0", S0).expect("the S0 fixture")
}

fn backgrounds() -> Vec<Background> {
    vec![Background::flat(), Background::fbm(), s0()]
}

fn cases(bg: &[Background], keep: impl Fn(&Case) -> bool) -> Vec<Case> {
    let mut out = Vec::new();
    for_each_case(bg, |c| {
        if keep(c) {
            out.push(c.clone());
        }
    });
    out
}

fn across(angle_deg: f64, x: f64, y: f64) -> f64 {
    let t = angle_deg * std::f64::consts::PI / 180.0;
    let mid = (N as f64 - 1.0) / 2.0;
    let (nx, ny) = (-libm::sin(t), libm::cos(t));
    ((x - mid) * nx + (y - mid) * ny - OFFSET_CELLS).abs()
}

// spec: TEST-155
#[test]
fn the_s0_fixture_is_the_recorded_crop() {
    assert_eq!(S0.len(), N * N * 4, "the S0 fixture is {N} x {N} f32");
    assert_eq!(crc32c(S0), S0_CRC32C, "the S0 fixture's CRC32C (its SHA-256 is in TEST-155's Detail)");
    let b = s0();
    let (lo, hi) = b.z.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| (l.min(v), h.max(v)));
    assert!(lo > 0.0, "the S0 crop is all land: min {lo}");
    assert!(hi > 600.0 && hi < 700.0, "the S0 crop's relief: max {hi}");
}

// spec: TEST-155
#[test]
fn the_valleys_follow_their_definition() {
    assert_eq!(carve(Profile::V, 0.0, 0.0), -DEPTH_M, "the floor at the centre lies DEPTH_M under the background");
    assert_eq!(carve(Profile::V, 0.0, ZONE_CELLS), 0.0, "the wall reaches the background inside the zone");
    assert!(carve(Profile::Round, 0.0, 1.0) < carve(Profile::V, 0.0, 1.0), "the round floor is lower than the V one cell off the floor");
    assert!(carve(Profile::V, 50.0, 0.0) < carve(Profile::V, -50.0, 0.0), "the floor falls downstream");

    let flat = [Background::flat()];
    let all = cases(&flat, |c| c.profile == Profile::V);
    let find = |f: Floor, a: f64| all.iter().find(|c| c.floor == f && c.angle_deg == a).expect("case");
    for &a in &ANGLES_DEG {
        let smooth = find(Floor::Smooth, a);
        // the sawtooth adds at most SAW_M, only near the axis
        let saw = find(Floor::Sawtooth, a);
        for k in 0..N * N {
            let d = saw.surface[k] - smooth.surface[k];
            let off = across(a, (k % N) as f64, (k / N) as f64);
            assert!((0.0..SAW_M).contains(&d), "sawtooth at {a} deg, cell {k}: {d} m");
            assert!(d == 0.0 || off <= SAW_HALF_WIDTH + 1e-9, "sawtooth at {a} deg, cell {k}, {off} cells off the axis: {d} m");
        }
        // the zone and the channel path
        let zone = smooth.zone.iter().filter(|&&z| z).count();
        assert!(zone > 2 * ZONE_CELLS as usize * 200, "the zone at {a} deg: {zone} cells");
        let p = &smooth.channel;
        assert!(p.len() > 150, "the channel at {a} deg: {} cells", p.len());
        assert!(p.iter().all(|&k| smooth.zone[k] && smooth.inside[k]), "the channel at {a} deg stays in the box's zone");
        let off: Vec<(usize, f64)> =
            p.iter().enumerate().map(|(i, &k)| (i, across(a, (k % N) as f64, (k / N) as f64))).filter(|x| x.1 > 1.0).collect();
        assert!(
            off.is_empty(),
            "the smooth channel at {a} deg lies on the floor; off it (step, cells): {off:?}, path start {:?}",
            p.iter().take(6).map(|&k| (k % N, k / N)).collect::<Vec<_>>()
        );
        // a staircase differs from the smooth floor only where it is a defect
        for f in [Floor::Stair4, Floor::Stair8] {
            let st = find(f, a);
            let diff = (0..N * N).filter(|&k| (st.surface[k] - smooth.surface[k]).abs() > 1e-6).count();
            assert!(diff > 0, "{} at {a} deg differs from smooth", f.name());
        }
    }
    assert!(Floor::Stair4.is_defect_at(45.0) && !Floor::Stair4.is_defect_at(0.0) && Floor::Stair4.is_defect_at(135.0));
    assert!(Floor::Stair8.is_defect_at(22.5) && !Floor::Stair8.is_defect_at(45.0) && !Floor::Stair8.is_defect_at(135.0));
    assert!(Floor::Sawtooth.is_defect_at(0.0) && !Floor::Smooth.is_defect_at(22.5));
}

/// A control that reads the floor class: 1 on a defect case, 0 otherwise.
struct Oracle;
/// A constant reading.
struct Constant;
/// A reading of the angle.
struct Angle;

impl GeometricMetric for Oracle {
    fn name(&self) -> &str {
        "oracle"
    }
    fn band(&self) -> Option<(f64, f64)> {
        Some((0.0, 0.5))
    }
    fn measure(&self, _: &Grid, c: &Case) -> f64 {
        f64::from(u8::from(c.is_defect()))
    }
}

impl GeometricMetric for Constant {
    fn name(&self) -> &str {
        "constant"
    }
    fn band(&self) -> Option<(f64, f64)> {
        Some((0.0, 0.5))
    }
    fn measure(&self, _: &Grid, _: &Case) -> f64 {
        0.0
    }
}

impl GeometricMetric for Angle {
    fn name(&self) -> &str {
        "angle"
    }
    fn band(&self) -> Option<(f64, f64)> {
        Some((0.0, 0.3))
    }
    fn measure(&self, _: &Grid, c: &Case) -> f64 {
        c.angle_deg / 90.0 + if c.is_defect() { 10.0 } else { 0.0 }
    }
}

// spec: TEST-155
#[test]
fn the_judge_passes_a_control_and_fails_a_constant_and_an_angle_reading() {
    let cal = calibrate(&[&Oracle, &Constant, &Angle], &[Background::flat()]);
    assert_eq!(cal[0].verdict, Verdict::Pass, "{}", cal[0].table());
    assert!(matches!(cal[1].verdict, Verdict::MissesItsDefect(_)), "{}", cal[1].table());
    assert!(matches!(cal[2].verdict, Verdict::NotAngleInvariant(_)), "{}", cal[2].table());
    for c in &cal {
        assert_eq!(c.readings.len(), PROFILES.len() * FLOORS.len() * ANGLES_DEG.len());
    }
    // a NaN reading is outside every band
    let r = |floor, value| Reading { background: "none".into(), profile: Profile::V, floor, angle_deg: 22.5, value };
    assert!(matches!(judge(Some((0.0, 1.0)), &[r(Floor::Smooth, f64::NAN)]), Verdict::NotAngleInvariant(_)));
    assert_eq!(judge(Some((0.0, 1.0)), &[r(Floor::Smooth, 0.5), r(Floor::Stair4, f64::NAN)]), Verdict::Pass);
    assert_eq!(judge(None, &[r(Floor::Smooth, 9.0)]), Verdict::ReportOnly);
}

// spec: TEST-155
#[test]
fn a_calibration_gives_the_same_bits_twice() {
    let bg = [Background::fbm()];
    let a = calibrate(&[&XdiagInZone], &bg);
    let b = calibrate(&[&XdiagInZone], &bg);
    let bits = |c: &[planet_cache::metric_calibration::Calibration]| c[0].readings.iter().map(|r| r.value.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a), bits(&b));
    assert!(Background::fbm().z.iter().zip(&Background::fbm().z).all(|(x, y)| x.to_bits() == y.to_bits()));
}

// spec: BUILD-069, TEST-155
#[test]
fn registered_metrics_pass_and_reported_ones_write_their_tables() {
    let reg = registered();
    let rep = reported();
    let metrics: Vec<&dyn GeometricMetric> = reg.iter().chain(rep.iter()).map(|m| m.as_ref()).collect();
    let cal = calibrate(&metrics, &backgrounds());
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/review/metric-calibration");
    for c in &cal {
        assert_eq!(c.readings.len(), 3 * PROFILES.len() * FLOORS.len() * ANGLES_DEG.len(), "{}", c.name);
        let p = c.write(&dir).expect("write the table");
        println!("{}: {} ({})", c.name, c.verdict.describe(), p.display());
    }
    for c in &cal[..reg.len()] {
        assert!(c.verdict.passed(), "BUILD-069: registered metric {} fails its calibration\n{}", c.name, c.table());
    }
}
