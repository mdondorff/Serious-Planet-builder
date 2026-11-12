//! Tier A, GEN-175 (TEST-122): the per-world epsilon of the f32 rounds, in the frame of the sea level. Earth's span keeps the profile epsilon; a 60 km span stays f32 with a strict descent at both
//! ends; the choice is a function of the world's bounds only (the same in every window); above `epsilon_max` the rounds are f64; one round in f32 is f64 to within the f32 spacing.

use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::precision::{f32_spacing, WorldPrecision, EPSILON_MAX_M};
use planet_generators::bake::route::{jitter, RoundRules};
use planet_generators::bake::Grid;

const EPS: f64 = 1e-3;

// spec: GEN-175, TEST-122
#[test]
fn earth_keeps_the_profile_epsilon_and_f32() {
    let w = WorldPrecision::new(-11_000.0, 9_000.0, EPS, EPSILON_MAX_M);
    assert_eq!((w.radius, w.epsilon_w, w.f64_rounds), (11_000.0, EPS, false));
    let mut c = CarveParams { round_f32: false, ..CarveParams::default() };
    w.apply(&mut c);
    assert_eq!((c.epsilon, c.round_f32), (CarveParams::default().epsilon, true), "Earth's carve parameters are today's, with the f32 flag");
}

// spec: GEN-175, TEST-122
#[test]
fn a_60_km_span_stays_f32_with_a_strict_descent_at_both_ends() {
    let w = WorldPrecision::new(-10_000.0, 50_000.0, EPS, EPSILON_MAX_M);
    assert!(!w.f64_rounds);
    assert_eq!(w.radius, 50_000.0);
    assert!(w.epsilon_w >= f32_spacing(w.radius) && w.epsilon_w > EPS);
    // a chain of epsilon_w steps at the top of R's exponent bin (32,768 to 65,536 m), above and below the sea, stays strictly descending after the narrowing
    // the starts include a half-spacing offset: a height that lands on a rounding tie must still step down
    let half = 0.5 * f32_spacing(w.radius);
    for top in [65_000.0, 65_000.0 + half, 33_000.0 + half, -65_000.0 + half, -33_000.0] {
        let z: Vec<f64> = (0..64).map(|i| f64::from((top - w.epsilon_w * f64::from(i)) as f32)).collect();
        assert!(z.windows(2).all(|p| p[1] < p[0]), "a chain of epsilon_w steps from {top} m is strictly descending after the narrowing");
    }
}

// spec: GEN-175, TEST-122
#[test]
fn the_choice_is_a_function_of_the_bounds_only() {
    let a = WorldPrecision::new(-10_000.0, 50_000.0, EPS, EPSILON_MAX_M);
    for _ in 0..3 {
        assert_eq!(a, WorldPrecision::new(-10_000.0, 50_000.0, EPS, EPSILON_MAX_M), "every window reads the same epsilon_w");
    }
}

// spec: GEN-175, TEST-122
#[test]
fn above_epsilon_max_the_rounds_are_f64() {
    let w = WorldPrecision::new(-200_000.0, 200_000.0, EPS, EPSILON_MAX_M);
    assert!(w.f64_rounds);
    assert_eq!(w.epsilon_w, EPS);
    let mut c = CarveParams { round_f32: true, ..CarveParams::default() };
    w.apply(&mut c);
    assert!(!c.round_f32);
    assert!(w.mark_line().contains("rounds=f64"));
}

// spec: GEN-175, TEST-122
/// One round in f32 is the f64 round to within twice the f32 spacing at R: every absolute comparison reads the unrounded height of the sea frame.
#[test]
fn one_round_in_f32_is_the_f64_round_to_within_the_spacing() {
    const N: usize = 64;
    let g = Grid::new(N, N, 800.0);
    let c = N as f64 / 2.0;
    let mut z0 = vec![0.0; N * N];
    for j in 0..N {
        for i in 0..N {
            let r = (((i as f64 - c).powi(2) + (j as f64 - c).powi(2)).sqrt() / (c * 0.9)).min(1.0);
            let edge = (i.min(j).min(N - 1 - i).min(N - 1 - j) as f64 / 6.0).min(1.0);
            z0[j * N + i] =
                (40_000.0 * (1.0 - r).powf(1.3) + 200.0 * jitter(5, (j * N + i) as u64, 0)) * edge + 2.0 - 10_000.0 * (1.0 - edge);
        }
    }
    let w = WorldPrecision::new(-10_000.0, 50_000.0, EPS, EPSILON_MAX_M);
    let base = CarveParams { steps: 5, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let outlet = g.outlets(&z0, 0.0);
    let run = |p: &CarveParams| {
        let mut z = z0.clone();
        carve(&g, &mut z, &outlet, p, None, 2);
        z
    };
    let mut pw = base;
    w.apply(&mut pw);
    let (zf, z64) = (run(&pw), run(&CarveParams { round_f32: false, epsilon: w.epsilon_w, ..base }));
    let worst = zf.iter().zip(&z64).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
    assert!(
        worst <= 2.0 * f32_spacing(w.radius),
        "one round in f32 differs from f64 by {worst} m, more than 2 x the spacing {}",
        f32_spacing(w.radius)
    );
}

// spec: GEN-175, TEST-122
#[test]
fn the_spacing_and_the_choice_handle_every_edge() {
    assert_eq!((f32_spacing(0.0), f32_spacing(-0.0)), (0.0, 0.0));
    assert_eq!(f32_spacing(1.0), 2f64.powi(-23));
    assert_eq!(f32_spacing(16_384.0), 2f64.powi(-9), "at a power of two the spacing is the one above it");
    assert_eq!(f32_spacing(16_383.0), 2f64.powi(-10));
    assert_eq!(f32_spacing(-16_383.0), 2f64.powi(-10));
    for bad in [1e39, f64::INFINITY, f64::NAN] {
        assert!(f32_spacing(bad).is_infinite(), "{bad} does not fit an f32");
    }
    let w = WorldPrecision::new(-1e39, 1e39, EPS, EPSILON_MAX_M);
    assert!(w.f64_rounds, "a span that does not fit an f32 runs in f64");
}
