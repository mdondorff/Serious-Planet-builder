//! Tier A, GEN-153 (A3 section 6.6): the memo caches of the edit path have a budget in bytes; what is over it is evicted (previous results, then tile floods, then catchment rounds, each least recently
//! used first, never the data of the running solve), and eviction never changes a result: a solve after eviction, an edit and its undo and a replay are the fresh build bit for bit (G-EQ). Small grids.

use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
use planet_generators::bake::round::{solve_rounds_memo, solve_rounds_with, Drivers, RoundCache, RoundWork};
use planet_generators::bake::route::RoundRules;
use planet_generators::bake::Grid;

const N: usize = 72;
const R: f64 = 6_371_000.0;

fn terrain(salt: u64) -> Vec<f64> {
    let mut z = vec![0.0; N * N];
    for j in 0..N {
        for i in 0..N {
            let (x, y) = (i as f64, j as f64);
            let h = planet_generators::bake::route::jitter(salt, (j * N + i) as u64, 0);
            let edge = (i.min(j).min(N - 1 - i).min(N - 1 - j) as f64 / 12.0).min(1.0);
            let v = (350.0
                + 500.0 * libm::sin(x * 0.13 + salt as f64) * libm::cos(y * 0.11)
                + 250.0 * libm::sin(0.37 * x + 0.21 * y)
                + 30.0 * h)
                * edge
                + 2.0;
            z[j * N + i] = libm::floor(v / 60.0) * 60.0 + 2.0 + 0.3 * h.abs();
        }
    }
    z
}

fn grids() -> Vec<(&'static str, Grid)> {
    let dx = R * core::f64::consts::FRAC_PI_2 / 12288.0;
    vec![
        ("uniform", Grid::new(N, N, 800.0)),
        ("metric", Grid::new(N, N, dx).with_origin((8100, 900)).with_metric(Metric::cube_face(12288, R, (8100, 900), N, N))),
    ]
}

fn params(rounds: usize) -> CarveParams {
    CarveParams { steps: rounds * 5, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() }
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn fresh(g: &Grid, z0: &[f64], outlet: &[bool], p: &CarveParams, rounds: usize) -> Vec<f64> {
    let mut z = z0.to_vec();
    solve_rounds_with(g, &mut z, outlet, p, rounds, &1usize, 3, &Drivers::default());
    z
}

fn edited(z0: &[f64]) -> Vec<f64> {
    let mut e = z0.to_vec();
    for j in 30..36 {
        for i in 40..46 {
            e[j * N + i] += 400.0;
        }
    }
    e
}

fn solve(g: &Grid, z0: &[f64], outlet: &[bool], p: &CarveParams, cache: &mut RoundCache) -> (Vec<f64>, Vec<RoundWork>) {
    let mut z = z0.to_vec();
    let (_, w) = solve_rounds_memo(g, &mut z, outlet, p, 4, &1usize, 3, &Drivers::default(), 12, cache);
    (z, w)
}

// spec: GEN-153
#[test]
fn a_solve_after_any_eviction_an_edit_its_undo_and_a_replay_are_the_fresh_build() {
    for (name, g) in grids() {
        let z0 = terrain(5);
        let outlet = g.outlets(&z0, 0.0);
        let p = params(4);
        // The bytes an unlimited cache holds after one solve.
        let mut all = RoundCache::default();
        solve(&g, &z0, &outlet, &p, &mut all);
        let full = all.bytes();
        assert!(full > 0);
        for (label, budget) in [("a third", full / 3), ("nothing", 0), ("one byte under all", full - 1)] {
            let mut cache = RoundCache::with_budget(budget);
            let want0 = bits(&fresh(&g, &z0, &outlet, &p, 4));
            let (first, w) = solve(&g, &z0, &outlet, &p, &mut cache);
            assert_eq!(bits(&first), want0, "the first solve under a budget of {label} ({name})");
            assert!(
                cache.bytes() <= budget,
                "the cache holds {} bytes over the budget {budget} after the solve ({name}, {label})",
                cache.bytes()
            );
            assert!(w.iter().map(|x| x.evicted).sum::<usize>() > 0, "something was evicted ({name}, {label})");
            let e = edited(&z0);
            let want_e = bits(&fresh(&g, &e, &outlet, &p, 4));
            let (got_e, _) = solve(&g, &e, &outlet, &p, &mut cache);
            assert_eq!(bits(&got_e), want_e, "G-EQ after an edit under {label} ({name})");
            assert!(cache.bytes() <= budget);
            // Undo: the original definition again is the first result.
            let (undone, _) = solve(&g, &z0, &outlet, &p, &mut cache);
            assert_eq!(bits(&undone), want0, "undo under {label} ({name})");
            // Replay: the edit again, then the original again.
            let (again_e, _) = solve(&g, &e, &outlet, &p, &mut cache);
            assert_eq!(bits(&again_e), want_e, "replay of the edit under {label} ({name})");
            let (again, _) = solve(&g, &z0, &outlet, &p, &mut cache);
            assert_eq!(bits(&again), want0, "replay of the undo under {label} ({name})");
        }
    }
}

// spec: GEN-153
#[test]
fn a_budget_keeps_the_recent_work_so_a_repeat_solve_does_less_than_a_fresh_build_and_no_budget_evicts_nothing() {
    let (_, g) = grids().remove(0);
    let z0 = terrain(5);
    let outlet = g.outlets(&z0, 0.0);
    let p = params(4);
    let mut all = RoundCache::default();
    let (_, w_fresh) = solve(&g, &z0, &outlet, &p, &mut all);
    assert!(w_fresh.iter().all(|w| w.evicted == 0 && w.evicted_bytes == 0), "no budget, no eviction");
    let carved_fresh: usize = w_fresh.iter().map(|w| w.carved).sum();
    let (_, w_all) = solve(&g, &z0, &outlet, &p, &mut all);
    assert_eq!(w_all.iter().map(|w| w.carved).sum::<usize>(), 0, "an unlimited cache answers the repeat from the memo");
    let mut half = RoundCache::with_budget(all.bytes() / 2);
    solve(&g, &z0, &outlet, &p, &mut half);
    let (_, w_half) = solve(&g, &z0, &outlet, &p, &mut half);
    let carved_half: usize = w_half.iter().map(|w| w.carved).sum();
    assert!(carved_half > 0 && carved_half < carved_fresh, "half the budget: the repeat carves {carved_half} of {carved_fresh} catchments");
    assert!(half.bytes() <= all.bytes() / 2);
    let mut none = RoundCache::without_memory();
    solve(&g, &z0, &outlet, &p, &mut none);
    assert_eq!(none.bytes(), 0, "a cache that keeps nothing still keeps nothing");
}

// spec: GEN-153
#[test]
fn the_partitioned_route_under_a_budget_is_the_route_without_one_and_the_cache_does_not_grow_with_the_surfaces() {
    let (_, g) = grids().remove(0);
    let rules = RoundRules::ROUND;
    let outlets = |z: &[f64]| g.outlets(z, 0.0);
    let mut unlimited = PartCache::default();
    let z0 = terrain(5);
    partitioned_route_weighted(&g, &z0, &outlets(&z0), 1e-3, 3, 12, rules, None, &mut unlimited);
    let held = unlimited.bytes();
    let mut small = PartCache::with_budget(held / 3);
    for salt in [5u64, 6, 7, 5, 8, 9] {
        let z = terrain(salt);
        let got = partitioned_route_weighted(&g, &z, &outlets(&z), 1e-3, 3, 12, rules, None, &mut small);
        let want = partitioned_route_weighted(&g, &z, &outlets(&z), 1e-3, 3, 12, rules, None, &mut PartCache::without_memory());
        assert_eq!((got.routing.receiver, bits(&got.filled)), (want.routing.receiver, bits(&want.filled)), "salt {salt}");
    }
    // The previous result of the slot just written is kept (one routing's arrays), and the memo stays near the budget: it does not hold six surfaces' floods.
    assert!(small.bytes() < unlimited.bytes() * 2, "{} against {}", small.bytes(), unlimited.bytes());
}
