#![allow(clippy::needless_range_loop)]
//! Tier A, GEN-166 (TEST-120; A3 section 9, section 12.4 P0 b): the round surface is stored f32. The heights are narrowed once at the end of every round and read widened by the next
//! round; the round computes in f64. The round solver, the memoised solve and the whole-region carve are the same bits under the flag, on any thread count; the narrowing is idempotent;
//! the near-degenerate fixtures (flats, shallow pits, narrow channels, spill levels, low gradients) solve to f32-representable heights and drain. Small grids; no store read.

use planet_generators::bake::carve::{carve, narrow_f32, CarveParams};
use planet_generators::bake::round::{solve_rounds, solve_rounds_memo, Drivers, RoundCache};
use planet_generators::bake::route::{jitter, route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;

const N: usize = 48;

fn params(rounds: usize, f32_surface: bool) -> CarveParams {
    CarveParams { steps: rounds * 5, reroute_every: 5, round_rules: RoundRules::ROUND, round_f32: f32_surface, ..CarveParams::default() }
}

/// The near-degenerate land of the A3 12.4 P0 b list, one class a name, on a ridge-and-valley base of a few hundred metres.
fn fixture(class: &str) -> Vec<f64> {
    let mut z = vec![0.0; N * N];
    for j in 0..N {
        for i in 0..N {
            let (x, y) = (i as f64, j as f64);
            let edge = (i.min(j).min(N - 1 - i).min(N - 1 - j) as f64 / 8.0).min(1.0);
            let u = jitter(9, (j * N + i) as u64, 0);
            let rolling = 300.0 + 120.0 * libm::sin(x * 0.2) * libm::cos(y * 0.17) + 40.0 * libm::sin(0.5 * x + 0.3 * y);
            z[j * N + i] = match class {
                // terraces: flats of 60 m steps with a sub-metre texture
                "flats" => (libm::floor(rolling / 60.0) * 60.0 + 2.0 + 0.2 * u.abs()) * edge + 2.0,
                // a smooth slope with dimples of 0.5 to 3 mm
                "shallow pits" => (800.0 - 2.0 * x - 1.0 * y - 0.003 * (u + 1.0) * f64::from(u32::from((i + 3 * j) % 7 == 0))) * edge + 2.0,
                // a plain with a one-cell-wide channel 1 cm lower
                "narrow channels" => (300.0 - 0.5 * x + if i == N / 2 { -0.01 } else { 0.0 }) * edge + 2.0,
                // two basins at 400 m joined by a sill 1 mm above their floor
                "spill levels" => {
                    let sill = if i == N / 2 && (N / 2 - 4..N / 2 + 4).contains(&j) { 400.001 } else { f64::NAN };
                    let wall = if i == N / 2 { 450.0 } else { 400.0 };
                    (if sill.is_nan() { wall } else { sill }) * edge + 2.0
                }
                // a plane of 1e-5 m a cell: neighbours collapse to equal heights in f32
                "low gradients" => (800.0 + 1e-5 * x + 0.5e-5 * y) * edge + 2.0,
                _ => unreachable!(),
            };
        }
    }
    z
}

const CLASSES: [&str; 5] = ["flats", "shallow pits", "narrow channels", "spill levels", "low gradients"];

// spec: GEN-166, TEST-120
#[test]
fn narrowing_is_the_f32_value_widened_and_idempotent() {
    let mut z = vec![0.1, 800.000_03, -3.5, 1.0 / 3.0, 4_096.000_01, 0.0];
    let want: Vec<f64> = z.iter().map(|&v| f64::from(v as f32)).collect();
    narrow_f32(&mut z);
    assert_eq!(z.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), want.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    let again = z.clone();
    narrow_f32(&mut z);
    assert_eq!(z, again, "a value that is already f32 does not move");
    assert!((800.000_03f64 - z[1]).abs() <= 3.1e-5, "800 m rounds by at most half an ulp of 6.1e-5 m");
}

// spec: GEN-166, TEST-120
#[test]
#[should_panic(expected = "does not fit an f32")]
fn a_height_that_does_not_fit_an_f32_is_refused() {
    narrow_f32(&mut [1e300]);
}

// spec: GEN-166, TEST-120
#[test]
fn the_f32_round_solver_is_the_whole_region_carve_on_any_thread_count_and_leaves_f32_heights() {
    let g = Grid::new(N, N, 800.0);
    for class in CLASSES {
        let z0 = fixture(class);
        let outlet = g.outlets(&z0, 0.0);
        let p = params(4, true);
        let mut want = z0.clone();
        carve(&g, &mut want, &outlet, &p, None, 3);
        assert!(want.iter().all(|&v| v == f64::from(v as f32)), "{class}: the carve leaves f32-representable heights");
        for threads in [1usize, 3, 8] {
            let mut got = z0.clone();
            solve_rounds(&g, &mut got, &outlet, &p, 4, &threads, 3);
            let differ = got.iter().zip(&want).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
            assert_eq!(differ, 0, "G-ROUND f32: {differ} cells differ ({class}, {threads} threads)");
        }
        // every land cell drains on the final surface
        let r = route_rules(&g, &want, &outlet, p.epsilon, 3, 0.0, None, RoundRules::ROUND);
        assert!((0..N * N).all(|c| outlet[c] || r.receiver[c] != NONE), "{class}: a land cell without a receiver");
        // the f32 surface is not the f64 one (the fixture is moved by the narrowing), and stays near it
        let mut f64run = z0.clone();
        carve(&g, &mut f64run, &outlet, &params(4, false), None, 3);
        let (mut moved, mut worst) = (0usize, 0.0f64);
        for c in 0..N * N {
            if want[c].to_bits() != f64run[c].to_bits() {
                moved += 1;
            }
            worst = worst.max((want[c] - f64run[c]).abs());
        }
        eprintln!("{class}: {moved} of {} cells differ from the f64 run, largest {worst:.4} m", N * N);
    }
}

// spec: GEN-166, TEST-120
#[test]
fn the_memoised_f32_solve_is_the_fresh_solve_and_a_changed_flag_is_not_a_cache_hit() {
    let g = Grid::new(N, N, 800.0);
    let z0 = fixture("flats");
    let outlet = g.outlets(&z0, 0.0);
    let (pf, pd) = (params(3, true), params(3, false));
    let mut fresh = z0.clone();
    solve_rounds(&g, &mut fresh, &outlet, &pf, 3, &1usize, 3);
    let mut cache = RoundCache::default();
    for _ in 0..2 {
        let mut z = z0.clone();
        solve_rounds_memo(&g, &mut z, &outlet, &pf, 3, &1usize, 3, &Drivers::default(), 16, &mut cache);
        assert!(z.iter().zip(&fresh).all(|(a, b)| a.to_bits() == b.to_bits()), "the memoised f32 solve is the fresh one");
    }
    // the same cache, the flag off: the f64 solve, not the f32 one
    let mut f64fresh = z0.clone();
    solve_rounds(&g, &mut f64fresh, &outlet, &pd, 3, &1usize, 3);
    let mut z = z0.clone();
    solve_rounds_memo(&g, &mut z, &outlet, &pd, 3, &1usize, 3, &Drivers::default(), 16, &mut cache);
    assert!(z.iter().zip(&f64fresh).all(|(a, b)| a.to_bits() == b.to_bits()), "a cache filled by an f32 solve gives the f64 solve's bits");
}

// spec: GEN-166, TEST-120 (architect's small item: an unhashed requirement is refused, not assumed)
#[test]
#[should_panic(expected = "the canonical flood is required")]
fn the_memoised_solve_refuses_a_run_without_the_canonical_flood() {
    let g = Grid::new(N, N, 800.0);
    let z0 = fixture("flats");
    let outlet = g.outlets(&z0, 0.0);
    let p = CarveParams { round_rules: RoundRules::default(), ..params(1, false) };
    solve_rounds_memo(&g, &mut z0.clone(), &outlet, &p, 1, &1usize, 3, &Drivers::default(), 16, &mut RoundCache::default());
}
