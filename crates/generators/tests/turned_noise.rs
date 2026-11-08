//! Tier A, GEN-140: the turned lattices of the bands and creases. A rotation of the lattice per octave, off by default; the bounds of the turned noise hold, and the lattice planes of the
//! untouched noise (flat across every plane, the same planes at every octave) are gone from the turned band.

use planet_core::hash::SplitMix64;
use planet_core::Vec3;
use planet_generators::classes::ridged_octaves_at;
use planet_generators::global::hill_octaves_at;
use planet_generators::height::{layer_noise_bounds_turned, layer_noise_turned, turn, turned_noise, turned_noise_word};

const SEED: u64 = 11;
const LAYER: u64 = 0x1234;

fn direction(rng: &mut SplitMix64) -> Vec3 {
    loop {
        let v = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0);
        if v.length() > 0.2 && v.length() < 1.0 {
            return v.normalized();
        }
    }
}

// spec: GEN-140
#[test]
fn the_switch_is_off_by_default_and_leaves_an_identity_word_alone() {
    assert!(!turned_noise());
    assert_eq!(turned_noise_word(0xdead_beef), 0xdead_beef);
}

// spec: GEN-140
#[test]
fn the_turn_is_a_rotation_that_differs_for_every_octave() {
    let mut rng = SplitMix64(1);
    let p = direction(&mut rng) * 37.0;
    for o in 0..12 {
        let q = turn(o, p);
        assert!((q.length() - p.length()).abs() < 1e-12 * p.length(), "octave {o}");
        // Deterministic.
        assert_eq!(turn(o, p), q);
        for o2 in 0..o {
            assert!((turn(o2, p) - q).length() > 1e-3, "octaves {o2} and {o} turn alike");
        }
    }
}

// spec: GEN-140
#[test]
fn the_bounds_of_the_turned_noise_hold_over_caps() {
    let mut rng = SplitMix64(2);
    for octave in [0u32, 3, 7, 10] {
        for freq in [4.0, 64.0, 1024.0] {
            for theta in [1e-4, 3e-3, 0.05] {
                let c = direction(&mut rng);
                let (lo, hi) = layer_noise_bounds_turned(SEED, LAYER, octave, c, theta, freq);
                for _ in 0..200 {
                    // A point in the cap: the centre moved by an angle under theta.
                    let d = (c + (direction(&mut rng) - c * direction(&mut rng).dot(c)) * (theta * rng.next_f64())).normalized();
                    let n = layer_noise_turned(SEED, LAYER, octave, d * freq);
                    assert!(n >= lo - 1e-9 && n <= hi + 1e-9, "octave {octave} freq {freq} theta {theta}: {n} not in [{lo}, {hi}]");
                }
            }
        }
    }
}

/// The mean absolute slope along x of `f` across the planes x = k/`freq` of the world axes, over random points on those planes (finite difference of 1e-7 of a cell).
fn mean_slope_across_planes(f: &dyn Fn(Vec3) -> f64, scale: f64, rng: &mut SplitMix64) -> f64 {
    let h = 1e-7 / scale;
    let mut sum = 0.0;
    let n = 400;
    for _ in 0..n {
        let d = direction(rng);
        // Move the point onto the nearest plane x = k / scale; the unit direction stays near unit length (the band reads the direction, not exactly a unit vector, here).
        let x = libm::round(d.x * scale) / scale;
        let p = Vec3::new(x, d.y, d.z);
        sum += ((f(Vec3::new(x + h, p.y, p.z)) - f(Vec3::new(x - h, p.y, p.z))) / (2.0 * h)).abs();
    }
    sum / f64::from(n)
}

// spec: GEN-140
#[test]
fn the_lattice_planes_of_the_untouched_noise_are_flat_and_those_of_the_turned_noise_are_not() {
    let mut rng = SplitMix64(3);
    // Every octave of the untouched value noise has zero slope across its planes x = k / freq (the planes of a coarse octave are planes of every finer one); the sum of the octaves has too.
    let freq = 398.0;
    let sum = |turned: bool| {
        move |p: Vec3| -> f64 {
            (0..5u32)
                .map(|o| {
                    let f = freq * f64::from(1u32 << o);
                    let q = p * f;
                    if turned {
                        layer_noise_turned(SEED, LAYER, o, q)
                    } else {
                        planet_generators::height::layer_noise_untouched(SEED, LAYER, o, q)
                    }
                })
                .sum()
        }
    };
    let plain = mean_slope_across_planes(&sum(false), freq, &mut rng);
    let turned = mean_slope_across_planes(&sum(true), freq, &mut rng);
    assert!(plain < 0.05, "the untouched noise is flat across its planes: {plain}");
    assert!(turned > 100.0 * plain && turned > 1.0, "the turned noise has a slope across the world planes: {turned} against {plain}");
}

// spec: GEN-140
#[test]
fn the_turned_bands_keep_their_range_and_differ_from_the_untouched_ones() {
    let mut rng = SplitMix64(4);
    let (mut hill_diff, mut ridged_diff) = (0.0f64, 0.0f64);
    for _ in 0..300 {
        let d = direction(&mut rng);
        let (h0, h1) = (hill_octaves_at(SEED, d, false), hill_octaves_at(SEED, d, true));
        let (r0, r1) = (ridged_octaves_at(SEED, d, false), ridged_octaves_at(SEED, d, true));
        for v in [h0, h1, r0, r1] {
            assert!((0.0..=1.0).contains(&v), "{v}");
        }
        hill_diff = hill_diff.max((h0 - h1).abs());
        ridged_diff = ridged_diff.max((r0 - r1).abs());
    }
    assert!(hill_diff > 0.05 && ridged_diff > 0.05, "{hill_diff} {ridged_diff}");
}
