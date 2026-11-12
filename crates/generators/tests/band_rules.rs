//! Tier A, GEN-179 (TEST-127): the band rules of the start surface, switched off by default. The slope cap holds each band octave to `c x slope(e0) x wavelength / 2`; the 8 km hill octave has its own weight; with
//! both off every elevation and every key is the build before the rule. The switches are process-wide (as the turned lattices of GEN-140): no test here sets them (a test that did would race with the class fingerprint tests of the same binary); the rules are passed as values (`elevation_m_with`, `BakeInputs::id_with_rules`).

use planet_core::Vec3;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::classes::{capped_band_noise, BandRules, ClassSource, HILL_WAVELENGTHS_M, RIDGED_WAVELENGTHS_M};

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

// spec: GEN-179, TEST-127
#[test]
fn the_capped_noise_without_a_cap_or_weight_is_the_plain_sum() {
    let signals = [0.9, 0.4, 0.7, 0.2, 0.55];
    let amp = 1_400.0;
    let norm: f64 = (0..5).map(|o| libm::pow(0.5, f64::from(o))).sum();
    let plain: f64 = (0..5).map(|o| libm::pow(0.5, f64::from(o)) * signals[o as usize]).sum::<f64>() / norm;
    let got = capped_band_noise(&signals, &RIDGED_WAVELENGTHS_M, &[1.0; 5], amp, 0.3, 0.0);
    assert_eq!(got.to_bits(), plain.to_bits(), "c = 0 is the plain sum");
    let huge = capped_band_noise(&signals, &RIDGED_WAVELENGTHS_M, &[1.0; 5], amp, 0.3, 1e12);
    assert_eq!(huge.to_bits(), plain.to_bits(), "a cap that never binds is the plain sum");
}

// spec: GEN-179, TEST-127
#[test]
fn every_octave_is_held_to_its_cap_and_a_flat_base_has_no_band() {
    let amp = 780.0;
    let norm: f64 = (0..5).map(|o| libm::pow(0.5, f64::from(o))).sum();
    for (slope, c) in [(0.002, 1.0), (0.05, 0.5), (0.2, 2.0), (0.001, 4.0)] {
        for pattern in 0..32u32 {
            // the signals at their largest (1) or half, by the bits of the pattern
            let signals: Vec<f64> = (0..5).map(|o| if pattern >> o & 1 == 1 { 1.0 } else { 0.5 }).collect();
            let noise = capped_band_noise(&signals, &RIDGED_WAVELENGTHS_M, &[1.0; 5], amp, slope, c);
            // the height of the band is at most the sum of its octaves' heights, each at most min(its peak, its cap)
            let bound: f64 =
                (0..5).map(|o| (amp * libm::pow(0.5, f64::from(o)) / norm).min(c * slope * RIDGED_WAVELENGTHS_M[o as usize] / 2.0)).sum();
            assert!(
                amp * noise <= bound * (1.0 + 1e-12),
                "slope {slope}, c {c}, pattern {pattern}: {} m over the cap sum {bound} m",
                amp * noise
            );
        }
    }
    let flat = capped_band_noise(&[1.0; 3], &HILL_WAVELENGTHS_M, &[1.0; 3], 30.0, 0.0, 1.0);
    assert_eq!(flat, 0.0, "a base with no slope drains no band relief");
}

// spec: GEN-179, TEST-127
#[test]
fn a_larger_c_never_lowers_the_band_and_the_8_km_weight_scales_its_octave() {
    let signals = [0.8, 0.6, 0.9];
    let mut last = -1.0;
    for c in [0.0, 0.01, 0.1, 0.5, 1.0, 4.0, 100.0] {
        // c = 0 means no cap: skip it in the monotone chain, it is the largest
        if c == 0.0 {
            continue;
        }
        let n = capped_band_noise(&signals, &HILL_WAVELENGTHS_M, &[1.0; 3], 175.0, 0.01, c);
        assert!(n >= last, "c {c}: {n} under {last}");
        last = n;
    }
    let full = capped_band_noise(&signals, &HILL_WAVELENGTHS_M, &[1.0; 3], 175.0, 0.01, 0.0);
    let no8 = capped_band_noise(&signals, &HILL_WAVELENGTHS_M, &[0.0, 1.0, 1.0], 175.0, 0.01, 0.0);
    let norm = 1.75;
    assert!((full - no8 - 0.8 / norm).abs() < 1e-15, "weight 0 removes the first octave's term, the norm stays");
    let half = capped_band_noise(&signals, &HILL_WAVELENGTHS_M, &[0.5, 1.0, 1.0], 175.0, 0.01, 0.0);
    assert!((full - half - 0.4 / norm).abs() < 1e-15);
}

// spec: GEN-179, TEST-127
#[test]
fn with_the_rules_off_or_never_binding_the_elevation_is_the_same_bits() {
    let src = ClassSource::new(14);
    let dirs = directions();
    let sample = |rules: &BandRules| -> Vec<u64> {
        dirs.iter()
            .map(|&d| {
                let p = src.eval(d);
                src.elevation_m_with(d, &p, 3.0, rules).to_bits()
            })
            .collect()
    };
    let off = sample(&BandRules::OFF);
    // the default path is the rules-off path
    let default: Vec<u64> = dirs.iter().map(|&d| src.elevation_m(d, &src.eval(d), 3.0).to_bits()).collect();
    assert_eq!(default, off, "no rule on: the build before GEN-179");
    // a cap that never binds: the noise is the plain sum, bit for bit
    assert_eq!(sample(&BandRules { slope_cap_c: 1e12, hill8_weight: 1.0 }), off, "a cap that never binds gives the build before the rule");
}

// spec: GEN-179, TEST-127
#[test]
fn a_cap_lowers_the_band_where_the_slope_is_small_and_the_weight_lowers_the_hill_octave() {
    let src = ClassSource::new(14);
    let dirs = directions();
    let relief = |rules: &BandRules| -> f64 {
        dirs.iter()
            .map(|&d| {
                let p = src.eval(d);
                (src.elevation_m_with(d, &p, 3.0, rules) - p.e0).abs()
            })
            .sum()
    };
    let off = relief(&BandRules::OFF);
    let capped = relief(&BandRules { slope_cap_c: 0.5, hill8_weight: 1.0 });
    assert!(capped < off, "the cap removes relief: {capped} against {off}");
    let no8 = relief(&BandRules { slope_cap_c: 0.0, hill8_weight: 0.0 });
    assert!(no8 < off, "the 8 km octave's weight removes relief: {no8} against {off}");
    let tight = relief(&BandRules { slope_cap_c: 0.05, hill8_weight: 1.0 });
    assert!(tight < capped, "a smaller c removes more: {tight} against {capped}");
    assert!(dirs.iter().all(|&d| src.e0_slope(d).is_finite() && src.e0_slope(d) >= 0.0), "the slope is finite and not negative");
}

// spec: GEN-179, TEST-127
#[test]
fn the_identity_word_moves_only_with_a_rule_on() {
    let w = 0xdead_beef_u64;
    assert_eq!(BandRules::OFF.fold(w), w, "both rules off: the word is unchanged");
    let a = BandRules { slope_cap_c: 0.5, hill8_weight: 1.0 };
    let b = BandRules { slope_cap_c: 1.0, hill8_weight: 1.0 };
    let c = BandRules { slope_cap_c: 0.0, hill8_weight: 0.25 };
    assert!(!BandRules::OFF.is_on() && a.is_on() && c.is_on());
    assert!(
        a.fold(w) != w && b.fold(w) != a.fold(w) && c.fold(w) != w && c.fold(w) != a.fold(w),
        "each rule and each value gives its own word"
    );
    // the inputs identity: unchanged with both rules off, moved by each rule and value
    let id = |r: &BandRules| BakeInputs::id_with_rules(1, 1, 1.0, 1, r);
    assert_eq!(id(&BandRules::OFF), BakeInputs::id_with_rules(1, 1, 1.0, 1, &BandRules::OFF));
    assert!(
        id(&a) != id(&BandRules::OFF) && id(&b) != id(&a) && id(&c) != id(&BandRules::OFF) && id(&c) != id(&a),
        "the inputs identity moves with each rule and value"
    );
    // the process's switches are off: the identity is the one before the rule
    assert!(!planet_generators::classes::band_rules_on());
    assert_eq!(planet_generators::classes::band_rules_word(w), w);
}

// spec: GEN-179, TEST-127
#[test]
#[should_panic(expected = "slope cap c")]
fn a_negative_cap_is_refused() {
    planet_generators::classes::set_band_slope_cap(-1.0);
}

// spec: GEN-179, TEST-127
#[test]
#[should_panic(expected = "8 km hill octave")]
fn a_weight_over_one_is_refused() {
    planet_generators::classes::set_hill8_weight(1.5);
}

// spec: GEN-179, TEST-127
/// The slope estimate has no seam: two directions 0.6 m apart give the same slope to within the estimate's curvature error, on a ring across the old axis change (|x| = 0.9) and over the sphere.
#[test]
fn the_slope_estimate_has_no_seam() {
    let src = ClassSource::new(14);
    let mut checked = 0;
    let mut pairs: Vec<(Vec3, Vec3)> = Vec::new();
    for k in 0..400 {
        let a = f64::from(k) * 0.0157 + 0.3;
        let (y, z) = (libm::cos(a), libm::sin(a));
        let r = libm::sqrt(1.0 - 0.81);
        pairs.push((Vec3::new(0.9 - 1e-7, r * y, r * z).normalized(), Vec3::new(0.9 + 1e-7, r * y, r * z).normalized()));
    }
    for d in directions() {
        pairs.push((d, Vec3::new(d.x + 1e-7, d.y, d.z).normalized()));
    }
    for (p, q) in pairs {
        let (s0, s1) = (src.e0_slope(p), src.e0_slope(q));
        if s0.max(s1) < 1e-4 {
            continue;
        }
        checked += 1;
        assert!((s0 - s1).abs() <= 0.02 * s0.max(s1) + 1e-6, "slope {s0} against {s1} across two directions 0.6 m apart");
    }
    assert!(checked >= 60, "the fixture finds slopes: {checked}");
}

// spec: GEN-179, TEST-127
/// A flat base (slope exactly 0) has no band whatever `c` is (stated in GEN-179); a base with a slope under a cap that never binds has the plain band.
#[test]
fn a_cap_that_never_binds_keeps_the_band_wherever_the_base_has_a_slope() {
    let src = ClassSource::new(14);
    let never = BandRules { slope_cap_c: 1e12, hill8_weight: 1.0 };
    let (mut with_slope, mut checked) = (0, 0);
    for &d in &directions() {
        let p = src.eval(d);
        if src.e0_slope(d) > 0.0 {
            with_slope += 1;
            assert_eq!(src.elevation_m_with(d, &p, 3.0, &never).to_bits(), src.elevation_m_with(d, &p, 3.0, &BandRules::OFF).to_bits());
        }
        checked += 1;
    }
    assert!(with_slope > 100 && checked == 600, "{with_slope} of {checked} directions have a slope");
}
