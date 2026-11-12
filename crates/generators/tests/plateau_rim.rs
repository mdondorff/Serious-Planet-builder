//! Tier A, GEN-183 (TEST-132): the plateau rim rule of the start surface, off by default. The uplift share is `smoothstep(-W, W, x)` of the signed distance inside the plateau field's 0.5 contour, so the
//! uplift never falls toward the interior; with the rule off every `e0` and every key is the build before. The switch is process-wide: no test here sets it; the rule is passed as a value (`eval_rules`).

use planet_core::Vec3;
use planet_generators::classes::{ClassSource, CoastTilt, PlateauRim};

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

// spec: GEN-183, TEST-132
#[test]
fn the_share_is_half_on_the_contour_monotone_and_saturates() {
    for w in [25.0, 100.0, 400.0] {
        let rim = PlateauRim { w_km: w };
        assert!(rim.is_on());
        assert!((rim.share(0.0) - 0.5).abs() < 1e-12, "half on the contour");
        assert_eq!(rim.share(-w), 0.0);
        assert_eq!(rim.share(-2.0 * w), 0.0);
        assert_eq!(rim.share(w), 1.0);
        assert_eq!(rim.share(2.0 * w), 1.0);
        let mut last = -1.0;
        for k in -80..=80 {
            let s = rim.share(f64::from(k) * w / 40.0);
            assert!(s >= last && (0.0..=1.0).contains(&s), "W {w}, step {k}: {s} after {last}");
            last = s;
        }
    }
    assert_eq!(PlateauRim::OFF.share(50.0), 0.0, "off adds nothing");
    assert!(!PlateauRim::OFF.is_on());
}

// spec: GEN-183, TEST-132
#[test]
fn with_the_rule_off_the_point_is_the_same_bits() {
    let src = ClassSource::new(14);
    for d in directions() {
        let (a, b) = (src.eval(d), src.eval_rules(d, &CoastTilt::OFF, &PlateauRim::OFF));
        assert_eq!(a.e0.to_bits(), b.e0.to_bits());
        assert_eq!(a.weights.map(f64::to_bits), b.weights.map(f64::to_bits));
    }
}

// spec: GEN-183, TEST-132
#[test]
fn the_identity_word_is_unchanged_while_off_and_moves_with_the_width() {
    let id = 0x1234_5678_9abc_def0u64;
    assert_eq!(PlateauRim::OFF.fold(id), id);
    let (a, b) = (PlateauRim { w_km: 50.0 }.fold(id), PlateauRim { w_km: 60.0 }.fold(id));
    assert!(a != id && b != id && a != b);
}

// spec: GEN-183, TEST-132
#[test]
#[should_panic(expected = "plateau rim width")]
fn a_negative_width_is_refused() {
    planet_generators::classes::set_plateau_rim(-1.0);
}
