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
            let r = (((i as f64 - c) * (i as f64 - c) + (j as f64 - c) * (j as f64 - c)).sqrt() / (c * 0.9)).min(1.0);
            let edge = (i.min(j).min(N - 1 - i).min(N - 1 - j) as f64 / 6.0).min(1.0);
            z0[j * N + i] =
                (40_000.0 * libm::pow(1.0 - r, 1.3) + 200.0 * jitter(5, (j * N + i) as u64, 0)) * edge + 2.0 - 10_000.0 * (1.0 - edge);
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
    assert_eq!(f32_spacing(1.0), libm::pow(2.0, -23.0));
    assert_eq!(f32_spacing(16_384.0), libm::pow(2.0, -9.0), "at a power of two the spacing is the one above it");
    assert_eq!(f32_spacing(16_383.0), libm::pow(2.0, -10.0));
    assert_eq!(f32_spacing(-16_383.0), libm::pow(2.0, -10.0));
    for bad in [1e39, f64::INFINITY, f64::NAN] {
        assert!(f32_spacing(bad).is_infinite(), "{bad} does not fit an f32");
    }
    let w = WorldPrecision::new(-1e39, 1e39, EPS, EPSILON_MAX_M);
    assert!(w.f64_rounds, "a span that does not fit an f32 runs in f64");
}

// spec: GEN-175, TEST-122
#[test]
fn apply_plan_sets_every_epsilon_that_acts_on_an_f32_surface() {
    use planet_generators::bake::build::PlanParams;
    let spill = std::path::PathBuf::from("unused.f64");
    // Earth: today's plan, with the world remembered
    let earth = WorldPrecision::new(-11_000.0, 9_000.0, EPS, EPSILON_MAX_M);
    let e = PlanParams::standard_for(5, 1, spill.clone(), &earth);
    // today's values, written out: the epsilon 1e-3 everywhere, the f32 rounds, the world remembered
    assert_eq!((e.epsilon, e.carve.epsilon, e.sediment.carve.epsilon, e.carve.round_f32), (1e-3, 1e-3, 1e-3, true));
    assert_eq!(e.precision, Some(earth));
    // 60 km: the carve, H1's fill and pass 2 (the sediment pass and K7) flood with epsilon_w; pass 2 never takes the f32 flag
    let w = WorldPrecision::new(-10_000.0, 50_000.0, EPS, EPSILON_MAX_M);
    let p = PlanParams::standard_for(5, 1, spill.clone(), &w);
    assert!(w.epsilon_w > EPS);
    assert_eq!((p.epsilon, p.carve.epsilon, p.sediment.carve.epsilon), (w.epsilon_w, w.epsilon_w, w.epsilon_w));
    assert!(p.carve.round_f32 && !p.sediment.carve.round_f32);
    let mut k7 = planet_generators::bake::k7::K7Params::standard(5, 0.0);
    w.apply_k7(&mut k7);
    assert_eq!(k7.pass2.carve.epsilon, w.epsilon_w, "K7 floods with the world's epsilon through apply_k7");
    // above epsilon_max: f64 rounds, the profile epsilon everywhere
    let big = WorldPrecision::new(-200_000.0, 200_000.0, EPS, EPSILON_MAX_M);
    let q = PlanParams::standard_for(5, 1, spill, &big);
    assert!(!q.carve.round_f32 && q.epsilon == EPS && q.carve.epsilon == EPS && q.sediment.carve.epsilon == EPS);
}

// spec: GEN-175, TEST-122
#[test]
fn a_height_outside_the_bounds_is_refused_naming_the_cell() {
    use planet_generators::bake::build::{build_resident, PlanParams};
    let w = WorldPrecision::new(-11_000.0, 9_000.0, EPS, EPSILON_MAX_M);
    let grid = Grid::new(4, 4, 800.0);
    let mut z = vec![100.0; 16];
    assert!(w.check_within_radius(&grid, &z, |c| c as u64 + 1000).is_ok());
    z[5] = -11_000.5;
    let e = w.check_within_radius(&grid, &z, |c| c as u64 + 1000).unwrap_err();
    assert!(e.contains("cell 5") && e.contains("global id 1005") && e.contains("-11000.5") && e.contains("R = 11000"), "{e}");
    z[5] = f64::NAN;
    assert!(w.check_within_radius(&grid, &z, |c| c as u64).is_err(), "a NaN is outside the bounds");
    let masked = Grid::new(4, 4, 800.0).with_active((0..16).map(|c| c != 5).collect());
    assert!(w.check_within_radius(&masked, &z, |c| c as u64).is_ok(), "an inactive cell is not checked");
    // through the plan: a 50 km summit in a world declared as Earth's span is refused after the rounds
    const N: usize = 32;
    let g = Grid::new(N, N, 800.0);
    let mut z0 = vec![0.0; N * N];
    for j in 4..N - 4 {
        for i in 4..N - 4 {
            z0[j * N + i] = 50_000.0 + jitter(3, (j * N + i) as u64, 0);
        }
    }
    let outlet = g.outlets(&z0, 0.0);
    let mut p = PlanParams::standard_for(5, 1, std::env::temp_dir().join(format!("planet-wire-{}.f64", std::process::id())), &w);
    p.rounds = 1;
    p.carve.steps = 5;
    let e = build_resident(&g, z0, &outlet, &p, &|_| (900.0, 12.0), &|_| {}).err().expect("the plan refuses the surface");
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        e.to_string().starts_with("the landmass:") && e.to_string().contains("GEN-175") && e.to_string().contains("global id"),
        "the input is checked before the rounds: {e}"
    );
}

// spec: GEN-175, TEST-122
/// H1's flood on the 60 km world: a crater floor at 39 km under a rim at 40 km fills to the rim and is graded (GEN-173) by steps below epsilon. H1 reads the f32 surface and floods it in f64: every
/// lifted cell stands strictly above its receiver with epsilon_w. A copy of that fill narrowed to f32 collapses (the grade steps by a fraction of epsilon): recorded here, so that nothing stores a narrowed fill (it is re-flooded in f64).
#[test]
fn h1s_fill_descends_strictly_on_the_60_km_world() {
    use planet_generators::bake::carve::narrow_f32;
    use planet_generators::bake::route::{route_rules_filled, NONE};
    const N: usize = 40;
    let g = Grid::new(N, N, 800.0);
    let mut z = vec![0.0; N * N];
    for j in 0..N {
        for i in 0..N {
            let ring = (i as f64 - 19.5).abs().max((j as f64 - 19.5).abs());
            z[j * N + i] = if ring < 6.0 {
                39_000.0
            } else if ring < 7.0 {
                40_000.0
            } else {
                (40_000.0 - 3_000.0 * (ring - 6.0)).max(1.0)
            };
        }
    }
    narrow_f32(&mut z);
    let outlet = g.outlets(&z, 0.0);
    let w = WorldPrecision::new(-10_000.0, 50_000.0, EPS, EPSILON_MAX_M);
    // (lifted cells, those not above their receiver, the same on the fill narrowed to f32)
    let ties = |eps: f64| {
        let (r, mut filled) = route_rules_filled(&g, &z, &outlet, eps, 5, 0.0, None, RoundRules::ROUND);
        let lifted: Vec<usize> = (0..N * N).filter(|&c| filled[c] > z[c] && r.receiver[c] != NONE).collect();
        let count = |f: &[f64]| lifted.iter().filter(|&&c| f[c] <= f[r.receiver[c] as usize]).count();
        let wide = count(&filled);
        narrow_f32(&mut filled);
        (lifted.len(), wide, count(&filled))
    };
    let (lifted, f64_ties, narrowed_ties) = ties(w.epsilon_w);
    assert!(lifted > 50, "the crater floor is lifted: {lifted} cells");
    assert_eq!(f64_ties, 0, "H1 floods in f64: every lifted cell stands above its receiver with epsilon_w");
    assert!(
        narrowed_ties > 0,
        "the grade steps by a fraction of epsilon: a fill narrowed to f32 collapses at epsilon_w (the fixture has teeth)"
    );
}

// spec: GEN-175, TEST-122
/// A round refuses a surface whose f32 spacing is not under its epsilon, wherever it runs (the regional bake, the plan): the chains would collapse near the summit.
#[test]
#[should_panic(expected = "GEN-175")]
fn a_round_with_an_epsilon_under_the_spacing_of_its_summit_is_refused() {
    use planet_generators::bake::carve::narrow_f32_checked;
    narrow_f32_checked(&mut [10.0, 40_000.0, -3.0], EPS);
}

// spec: GEN-175, TEST-122
#[test]
fn a_round_with_the_world_epsilon_narrows_the_summit() {
    use planet_generators::bake::carve::narrow_f32_checked;
    let w = WorldPrecision::new(-10_000.0, 50_000.0, EPS, EPSILON_MAX_M);
    let mut z = [10.0, 40_000.000_1, -3.0];
    narrow_f32_checked(&mut z, w.epsilon_w);
    assert_eq!(z[1], f64::from(40_000.000_1f64 as f32));
    narrow_f32_checked(&mut [16_000.0, -11_000.0], EPS);
}

// spec: GEN-175, TEST-122
#[test]
fn the_plan_has_no_constructor_without_the_bounds_of_its_world() {
    // `PlanParams::standard` is gone: only standard_for (compile-time: this test would not build with a call to `standard`), and Earth's bounds are written out by the callers
    let earth = WorldPrecision::new(-11_000.0, 9_000.0, EPS, EPSILON_MAX_M);
    let p = planet_generators::bake::build::PlanParams::standard_for(5, 1, std::path::PathBuf::from("unused.f64"), &earth);
    assert!(p.precision.is_some());
}

// spec: GEN-175, TEST-122
#[test]
#[should_panic(expected = "filled surface is never narrowed")]
fn a_filled_surface_is_never_narrowed_to_f32() {
    use planet_generators::bake::carve::{narrow_f32_surface, Surface};
    narrow_f32_surface(Surface::Filled, &mut [10.0, 20.0], EPS);
}

// spec: GEN-175, TEST-122
#[test]
fn a_round_surface_is_narrowed_through_the_surface_gate() {
    use planet_generators::bake::carve::{narrow_f32_surface, Surface};
    let mut z = [10.000_000_1, 20.5];
    narrow_f32_surface(Surface::Round, &mut z, EPS);
    assert_eq!(z[0], f64::from(10.000_000_1f64 as f32));
}
