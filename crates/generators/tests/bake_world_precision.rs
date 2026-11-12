//! Tier A, GEN-175 (TEST-122): the world datum and the per-world epsilon. Earth's span keeps the profile epsilon; a 60 km span stays f32 with a strict descent at both ends;
//! the choice is a function of the world's bounds only (the same in every window); above `epsilon_max` the rounds are f64.

use planet_generators::bake::carve::{narrow_f32_about, CarveParams};
use planet_generators::bake::precision::{f32_spacing, WorldPrecision, EPSILON_MAX_M};

const EPS: f64 = 1e-3;

// spec: GEN-175, TEST-122
#[test]
fn earth_keeps_the_profile_epsilon_and_f32() {
    let w = WorldPrecision::new(-11_000.0, 9_000.0, EPS, EPSILON_MAX_M);
    assert_eq!(w.epsilon_w, EPS);
    assert!(!w.f64_rounds);
    assert_eq!(w.datum, -1_000.0);
    let mut c = CarveParams::default();
    w.apply(&mut c);
    assert_eq!((c.epsilon, c.datum, c.round_f32), (EPS, -1_000.0, true));
}

// spec: GEN-175, TEST-122
#[test]
fn a_60_km_span_stays_f32_with_an_epsilon_above_twice_the_spacing() {
    let w = WorldPrecision::new(-10_000.0, 50_000.0, EPS, EPSILON_MAX_M);
    assert!(!w.f64_rounds);
    assert_eq!(w.radius, 30_000.0);
    assert!(w.epsilon_w >= f32_spacing(30_000.0) && w.epsilon_w > EPS);
    // a strict descent at both ends, at the top of R's exponent bin (16,384 to 32,768 m from the datum): a ramp that falls by epsilon_w a cell from the top of the world and from its bottom, narrowed about the datum, stays strictly monotone
    for top in [w.datum + 32_700.0, w.datum - 32_700.0 + 1_000.0] {
        let mut z: Vec<f64> = (0..64).map(|i| top - w.epsilon_w * i as f64).collect();
        narrow_f32_about(&mut z, w.datum);
        assert!(z.windows(2).all(|p| p[1] < p[0]), "a chain of epsilon_w steps near {top} m is strictly descending after the narrowing");
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
    assert_eq!((w.epsilon_w, w.datum), (EPS, 0.0));
    let mut c = CarveParams::default();
    w.apply(&mut c);
    assert!(!c.round_f32);
    assert!(w.mark_line().contains("rounds=f64"));
}

// spec: GEN-175, TEST-122
#[test]
fn the_datum_narrowing_is_gen_166_at_datum_zero_and_idempotent() {
    let mut a = vec![0.1, 800.000_03, -0.0, 1.0 / 3.0];
    let mut b = a.clone();
    narrow_f32_about(&mut a, 0.0);
    planet_generators::bake::carve::narrow_f32(&mut b);
    assert_eq!(a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), b.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    let mut z = vec![30_000.123_456, 29_999.9, -4.2];
    narrow_f32_about(&mut z, 20_000.0);
    let again = z.clone();
    narrow_f32_about(&mut z, 20_000.0);
    assert_eq!(z, again);
}

// spec: GEN-175, TEST-122
/// The datum frame changes the rounding only: every absolute comparison (sea level, the floor, the uplift taper, the outlets) reads the unshifted height. One round of a 40 km cone world under the datum is within a few
/// f32 spacings of the f64 round; a constant left unshifted would move it by metres.
#[test]
fn the_datum_frame_moves_the_heights_by_rounding_only() {
    use planet_generators::bake::carve::carve;
    use planet_generators::bake::route::{jitter, RoundRules};
    use planet_generators::bake::Grid;
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
    assert!(worst <= 4.0 * f32_spacing(w.radius), "one round under the datum differs from f64 by {worst} m, more than rounding explains");
}
