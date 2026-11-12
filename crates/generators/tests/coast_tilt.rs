//! Tier A, GEN-180 (TEST-128): the coast tilt of the start surface, off by default. The added height is `weight x t x D x s(d / D)` with a ramp that saturates at 1, so it never exceeds `t x D`, is 0 at the
//! coast and over the sea, and moves no class; with the tilt off every `e0` and every key is the build before. The switch is process-wide (as the band rules of GEN-179): no test here sets it; the tilt is passed
//! as a value (`eval_with`).

use planet_core::Vec3;
use planet_generators::classes::{ClassSource, CoastTilt, PLAIN, ROLLING};

/// A spiral of directions over the sphere (golden angle).
fn directions() -> Vec<Vec3> {
    let n = 600;
    (0..n)
        .map(|i| {
            let z = 1.0 - 2.0 * (f64::from(i) + 0.5) / f64::from(n);
            let r = libm::sqrt(1.0 - z * z);
            let a = f64::from(i) * 2.399_963_229_728_653;
            Vec3::new(r * libm::cos(a), r * libm::sin(a), z)
        })
        .collect()
}

// spec: GEN-180, TEST-128
#[test]
fn the_added_height_is_bounded_zero_at_the_coast_monotone_and_saturates() {
    for (t, d) in [(2.0, 100.0), (4.0, 200.0), (8.0, 400.0)] {
        let tilt = CoastTilt { t_m_per_km: t, d_km: d };
        assert_eq!(tilt.height_m(0.0, 1.0), 0.0, "no height at the coast");
        let mut last = 0.0;
        for k in 0..=80 {
            let dist = f64::from(k) * d / 40.0;
            let h = tilt.height_m(dist, 1.0);
            assert!(h >= last - 1e-12 && h <= t * d + 1e-9, "t {t}, D {d}, dist {dist}: {h} m (limit {} m)", t * d);
            last = h;
        }
        assert!((tilt.height_m(2.0 * d, 1.0) - t * d).abs() < 1e-9, "beyond D the tilt is t x D");
        assert!((tilt.height_m(d, 1.0) - t * d).abs() < 1e-9, "at D the tilt is t x D");
        // the slope at the coast is 2 t
        let eps = 1e-4;
        assert!((tilt.height_m(eps, 1.0) / eps - 2.0 * t).abs() < 1e-2 * t, "the slope at the coast is 2 t metres per km");
        // the weight scales the height and 0 gives none
        assert_eq!(tilt.height_m(d / 2.0, 0.0), 0.0);
        assert!((tilt.height_m(d / 2.0, 0.5) - 0.5 * tilt.height_m(d / 2.0, 1.0)).abs() < 1e-12);
    }
    assert_eq!(CoastTilt::OFF.height_m(300.0, 1.0), 0.0, "off adds nothing");
    assert!(!CoastTilt::OFF.is_on());
}

// spec: GEN-180, TEST-128
#[test]
fn with_the_tilt_off_the_point_is_the_same_bits() {
    let src = ClassSource::new(14);
    for d in directions() {
        let (a, b) = (src.eval(d), src.eval_with(d, &CoastTilt::OFF));
        assert_eq!(a.e0.to_bits(), b.e0.to_bits());
        assert_eq!(a.weights.map(f64::to_bits), b.weights.map(f64::to_bits));
    }
}

// spec: GEN-180, TEST-128
#[test]
fn the_tilt_lifts_land_only_within_its_bound_and_moves_no_class() {
    let src = ClassSource::new(14);
    let tilt = CoastTilt { t_m_per_km: 4.0, d_km: 200.0 };
    let (mut lifted, mut weightless, mut sea) = (0, 0, 0);
    for d in directions() {
        let (off, on) = (src.eval(d), src.eval_with(d, &tilt));
        assert_eq!(off.weights.map(f64::to_bits), on.weights.map(f64::to_bits), "the class weights do not see the tilt");
        let up = on.e0 - off.e0;
        let w = off.weights[PLAIN] + off.weights[ROLLING];
        if off.land_depth <= 0.0 {
            assert_eq!(up, 0.0, "the sea is untouched");
            sea += 1;
            continue;
        }
        assert!(up >= -1e-9 && up <= w * tilt.t_m_per_km * tilt.d_km + 1e-9, "added {up} m with the plain weight {w}");
        if up > 1.0 {
            lifted += 1;
        }
        if w == 0.0 {
            assert_eq!(up, 0.0, "a point with no plain weight gets none");
            weightless += 1;
        }
    }
    assert!(lifted > 20 && sea > 50, "the fixture has lifted land ({lifted}) and sea ({sea}); weightless land {weightless}");
}

// spec: GEN-180, TEST-128
/// The ramp is not cut short by the shelf's coast lattice (170 km): with D = 400 km the lift of a plain deep inland reaches nearly t x D; and the tilt is faded in at the field's coast, so that the shore has no step.
#[test]
fn the_ramp_reaches_its_full_height_inland_and_leaves_no_step_at_the_shore() {
    let src = ClassSource::new(14);
    let tilt = CoastTilt { t_m_per_km: 4.0, d_km: 400.0 };
    let limit = tilt.t_m_per_km * tilt.d_km;
    let (mut best, mut shore, mut shore_worst) = (0.0f64, 0, 0.0f64);
    for k in 0..30_000 {
        let n = 30_000;
        let z = 1.0 - 2.0 * (f64::from(k) + 0.5) / f64::from(n);
        let r = libm::sqrt(1.0 - z * z);
        let a = f64::from(k) * 2.399_963_229_728_653;
        let d = Vec3::new(r * libm::cos(a), r * libm::sin(a), z);
        let (off, on) = (src.eval(d), src.eval_with(d, &tilt));
        let w = off.weights[PLAIN] + off.weights[ROLLING];
        let up = on.e0 - off.e0;
        if off.land_depth > 0.0 && w > 0.9 {
            best = best.max(up / (w * limit));
        }
        if off.land_depth > 0.0 && off.land_depth < 0.01 {
            shore += 1;
            shore_worst = shore_worst.max(up / limit);
        }
    }
    assert!(best > 0.9, "a plain far inland gains {best} of t x D (the shelf lattice's cap would stop it at 0.67)");
    assert!(shore >= 5 && shore_worst < 0.03, "{shore} points within 0.01 of the field's coast gain at most {shore_worst} of t x D");
}

// spec: GEN-180, TEST-128
#[test]
fn the_identity_word_moves_only_with_the_tilt_on() {
    let w = 0xfeed_f00d_u64;
    assert_eq!(CoastTilt::OFF.fold(w), w, "off: the word is unchanged");
    let (a, b, c) = (
        CoastTilt { t_m_per_km: 2.0, d_km: 100.0 },
        CoastTilt { t_m_per_km: 4.0, d_km: 100.0 },
        CoastTilt { t_m_per_km: 2.0, d_km: 200.0 },
    );
    assert!(a.fold(w) != w && b.fold(w) != a.fold(w) && c.fold(w) != a.fold(w), "each value gives its own word");
    assert!(!planet_generators::classes::coast_tilt().is_on(), "the process runs with the tilt off");
}

// spec: GEN-180, TEST-128
#[test]
#[should_panic(expected = "coast tilt t")]
fn a_negative_tilt_is_refused() {
    planet_generators::classes::set_coast_tilt(-1.0, 100.0);
}

// spec: GEN-180, TEST-128
#[test]
#[should_panic(expected = "coast tilt distance")]
fn a_zero_distance_is_refused() {
    planet_generators::classes::set_coast_tilt(2.0, 0.0);
}
