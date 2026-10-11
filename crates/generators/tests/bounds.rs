//! LOD-008: `HeightField::height_bounds` is sound (no height inside a cap escapes its bounds) and useful (small caps get
//! small ranges, so node volumes hug the terrain near the camera).

use planet_core::hash::SplitMix64;
use planet_core::Vec3;
use planet_generators::height::{HeightField, Reference};
use planet_generators::stress::Steep;

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    loop {
        let v = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        if v.length() > 0.1 && v.length() <= 1.0 {
            return v.normalized();
        }
    }
}

/// A direction at angle exactly `theta` (or less) from `centre`.
fn in_cap(rng: &mut SplitMix64, centre: Vec3, theta: f64) -> Vec3 {
    let t = centre.cross(random_dir(rng)).normalized();
    let angle = if rng.range(0.0, 1.0) < 0.5 { theta } else { theta * rng.range(0.0, 1.0) };
    (centre * libm::cos(angle) + t * libm::sin(angle)).normalized()
}

fn sound(field: &dyn HeightField, seed: u64) -> (f64, f64) {
    let mut rng = SplitMix64(seed);
    let (mut worst_margin, mut width_small) = (f64::MAX, 0.0f64);
    for case in 0..600 {
        // Caps from 0.1 mm-ish to 5 degrees, log-spaced.
        let theta = 1e-8 * libm::pow(5e5, f64::from(case) / 600.0) * (1.0 + rng.range(0.0, 1.0));
        let centre = random_dir(&mut rng);
        let (lo, hi) = field.height_bounds(centre, theta);
        assert!(lo <= hi, "inverted bounds {lo} > {hi}");
        let mut points = vec![centre];
        points.extend((0..150).map(|_| in_cap(&mut rng, centre, theta)));
        for p in points {
            let h = field.height(p);
            assert!(h >= lo && h <= hi, "height {h} escapes [{lo}, {hi}] for a cap of {theta} rad at {centre:?} (point {p:?})");
            worst_margin = worst_margin.min((h - lo).min(hi - h));
        }
        if theta < 2e-5 {
            width_small = width_small.max(hi - lo);
        }
    }
    (worst_margin, width_small)
}

// spec: GEN-011
#[test]
fn reference_bounds_hold_for_every_point_of_random_caps_and_are_tight_for_small_caps() {
    let (margin, width_small) = sound(&Reference(1), 11);
    eprintln!(
        "reference: smallest distance of a height to its bound {margin:.6} m, widest bound of a cap under 2e-5 rad {width_small:.1} m"
    );
    assert!(width_small < 40.0, "bounds of a 130 m cap are {width_small} m wide: useless for node volumes");
}

// spec: GEN-011
// spec: TEST-015
#[test]
fn steep_bounds_hold_for_every_point_of_random_caps_and_are_tight_for_small_caps() {
    let (margin, width_small) = sound(&Steep(1), 13);
    eprintln!("steep: smallest distance of a height to its bound {margin:.6} m, widest bound of a cap under 2e-5 rad {width_small:.1} m");
    // A 2e-5 rad cap is about 130 m: the relief of this field over that span is hundreds of metres, but not kilometres.
    assert!(width_small < 2000.0, "bounds of a 130 m cap are {width_small} m wide: useless for node volumes");
}

// spec: GEN-011
#[test]
fn the_default_bounds_are_the_global_range() {
    struct Flat;
    impl HeightField for Flat {
        fn height(&self, _: Vec3) -> f64 {
            0.0
        }
        fn seed(&self) -> u64 {
            0
        }
        fn implementation_id(&self) -> &'static str {
            "flat"
        }
        fn version(&self) -> u32 {
            1
        }
        fn max_abs_height_m(&self) -> f64 {
            10.0
        }
    }
    assert_eq!(Flat.height_bounds(Vec3::new(1.0, 0.0, 0.0), 0.5), (-10.0, 10.0));
}
