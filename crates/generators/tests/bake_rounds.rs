//! Tier A, GEN-142 (A3 section 6.2, gates G-ROUND, G-LOCAL, G-DET at kernel scale): the round solver (one region-wide routing under the round rules, every catchment's breach and
//! sweeps from its key inputs alone) is the whole-region carve with the same rules, bit for bit, on any thread count. Small grids; no store read.

use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::metric::Metric;
use planet_generators::bake::round::{catchment_round, cut, gather, solve_rounds};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;

const N: usize = 72;
const R: f64 = 6_371_000.0;

/// A land of ridges with a coast plain; `terraced` quantises it into flats and basins.
fn terrain(salt: u64, terraced: bool) -> Vec<f64> {
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
            z[j * N + i] = if terraced { libm::floor(v / 60.0) * 60.0 + 2.0 + 0.3 * h.abs() } else { v };
        }
    }
    z
}

fn grids() -> Vec<(&'static str, Grid)> {
    let flat = Grid::new(N, N, 800.0);
    let dx = R * core::f64::consts::FRAC_PI_2 / 3072.0;
    let metric = Grid::new(N, N, dx).with_origin((2014, 190)).with_metric(Metric::cube_face(3072, R, (2014, 190), N, N));
    vec![("uniform", flat), ("metric", metric)]
}

fn params(rounds: usize) -> CarveParams {
    CarveParams { steps: rounds * 5, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() }
}

// spec: GEN-142
#[test]
fn the_round_solver_is_the_whole_region_carve_on_any_thread_count() {
    for (name, g) in grids() {
        for terraced in [false, true] {
            let z0 = terrain(3, terraced);
            let outlet = g.outlets(&z0, 0.0);
            let p = params(4);
            let mut want = z0.clone();
            carve(&g, &mut want, &outlet, &p, None, 3);
            for threads in [1usize, 3, 8] {
                let mut got = z0.clone();
                let rep = solve_rounds(&g, &mut got, &outlet, &p, RoundRules::ROUND, 4, threads, 3);
                assert_eq!(rep.rounds.len(), 4);
                let differ = got.iter().zip(&want).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
                assert_eq!(differ, 0, "G-ROUND: {differ} cells differ ({name}, terraced {terraced}, {threads} threads)");
            }
        }
    }
}

// spec: GEN-142
#[test]
fn a_catchment_round_reads_only_its_key_inputs() {
    for (name, g) in grids() {
        let z0 = terrain(5, true);
        let outlet = g.outlets(&z0, 0.0);
        let p = params(1);
        let r = route_rules(&g, &z0, &outlet, p.epsilon, 5, 0.0, None, RoundRules::ROUND);
        let cuts = cut(&r);
        let taper: Vec<f32> = z0.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect();
        // The region round, the whole-window loops of carve on this one routing.
        let mut want = z0.clone();
        carve(&g, &mut want, &outlet, &p, None, 5);
        let mut checked = 0;
        for k in 0..cuts.members.len() {
            let mut inp = gather(&g, &z0, &outlet, &r, &cuts, &taper, k);
            // The inputs hold nothing else: a copy built from them alone, in a fresh value, gives the same bits.
            let mut copy = inp.clone();
            catchment_round(&mut inp, &p, 5);
            catchment_round(&mut copy, &p, 5);
            assert_eq!(inp, copy);
            for (&c, v) in cuts.members[k].iter().zip(&inp.z) {
                assert_eq!(v.to_bits(), want[c as usize].to_bits(), "G-LOCAL: cell {c} of catchment {k} ({name})");
            }
            checked += cuts.members[k].len();
        }
        assert_eq!(checked, N * N, "every cell is in one catchment");
    }
}
