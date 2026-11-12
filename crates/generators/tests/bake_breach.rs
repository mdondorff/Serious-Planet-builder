#![allow(clippy::needless_range_loop)]
//! Tier A, GEN-174 (TEST-121; SPIKE-108 follow-up): the round-0 breach of the closed basins. A closed basin is drained by the cheapest path to a lower cell within the profile's depth and
//! length; a basin that cannot be drained stays filled; the pass is a function of the cells only (the same bits on any thread count and partition); the closed bowl of GEN-162 is unchanged.
//! Small grids; no store read.

use planet_generators::bake::breach::{breach_basins, PreBreach};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::flood::priority_flood_ungraded;
use planet_generators::bake::round::{solve_rounds, solve_rounds_memo, Drivers, RoundCache};
use planet_generators::bake::route::{jitter, RoundRules};
use planet_generators::bake::Grid;

const EPS: f64 = 1e-3;

/// A plane falling to the west (3 m a cell from `base`, the two west columns the sea) with a square pit of `side` cells whose floor lies `depth` metres under the plane, centred at `(cx, cy)`.
fn plane_with_pit(n: usize, cx: usize, cy: usize, side: usize, depth: f64, base: f64) -> (Grid, Vec<f64>, Vec<bool>) {
    let g = Grid::new(n, n, 100.0);
    let mut z: Vec<f64> = (0..n * n).map(|p| if p % n < 2 { -5.0 } else { base - 5.0 + 3.0 * (p % n) as f64 }).collect();
    for j in cy..cy + side {
        for i in cx..cx + side {
            z[j * n + i] -= depth;
        }
    }
    let outlet = g.outlets(&z, 0.0);
    (g, z, outlet)
}

fn lift_cells(g: &Grid, z: &[f64], outlet: &[bool], min_m: f64) -> usize {
    let f = priority_flood_ungraded(g, z, outlet, EPS, true);
    (0..z.len()).filter(|&c| f[c] - z[c] >= min_m).count()
}

fn pb(halo: usize) -> PreBreach {
    PreBreach { passes: 1, halo, max_depth_m: 500.0, min_lift_m: 0.5 }
}

// spec: GEN-174, TEST-121
#[test]
fn a_closed_noise_basin_is_drained_by_the_cheapest_path_and_nothing_is_raised() {
    let (g, z0, outlet) = plane_with_pit(60, 30, 25, 6, 8.0, 0.0);
    assert!(lift_cells(&g, &z0, &outlet, 0.5) >= 12, "the fixture holds a closed basin");
    let mut z = z0.clone();
    let rep = breach_basins(&g, &mut z, &outlet, EPS, &pb(128));
    assert_eq!((rep.basins, rep.drained, rep.kept_lake, rep.kept_bound), (1, 1, 0, 0), "{rep:?}");
    assert_eq!(lift_cells(&g, &z, &outlet, 0.5), 0, "no closed basin is left");
    assert!(z.iter().zip(&z0).all(|(a, b)| a <= b), "the breach only lowers");
    // the cut is the cheapest path: the two rim cells west of the floor (5 m and 2 m over it), nothing else
    assert_eq!(rep.cells_cut, 2, "{rep:?}");
    assert!((rep.cut_m_cells - 7.0).abs() < 1e-9, "{rep:?}");
    // a second pass changes nothing
    let again = z.clone();
    let rep2 = breach_basins(&g, &mut z, &outlet, EPS, &pb(128));
    assert_eq!((rep2.basins, z == again), (0, true));
}

// spec: GEN-174, TEST-121
#[test]
fn a_basin_deeper_than_the_breach_limit_stays_a_lake_untouched() {
    let (g, z0, outlet) = plane_with_pit(60, 30, 25, 6, 600.0, 600.0);
    let mut z = z0.clone();
    let rep = breach_basins(&g, &mut z, &outlet, EPS, &pb(128));
    assert_eq!((rep.drained, rep.kept_lake), (0, 1), "{rep:?}");
    assert!(z.iter().zip(&z0).all(|(a, b)| a.to_bits() == b.to_bits()), "a lake is not touched");
    // and the same basin within a limit that covers it drains
    let rep = breach_basins(&g, &mut z, &outlet, EPS, &PreBreach { max_depth_m: 700.0, ..pb(128) });
    assert_eq!(rep.drained, 1, "{rep:?}");
}

// spec: GEN-174, TEST-121
#[test]
fn the_path_stays_inside_the_halo_with_the_basin_and_a_basin_at_the_border_stays_a_lake() {
    let (g, z0, outlet) = plane_with_pit(60, 30, 25, 6, 8.0, 0.0);
    // the basin has diameter 5; the path is 3 cells: it needs halo - 1 - 5 >= 3
    for (halo, drained) in [(9, true), (8, false)] {
        let mut z = z0.clone();
        let rep = breach_basins(&g, &mut z, &outlet, EPS, &pb(halo));
        assert_eq!((rep.drained == 1, rep.kept_bound + rep.kept_lake), (drained, usize::from(!drained)), "halo {halo}: {rep:?}");
    }
    // a basin on the border ring is never breached (it may be larger than the window shows)
    let (g, z0, outlet) = plane_with_pit(60, 40, 1, 6, 8.0, 0.0);
    let mut z = z0.clone();
    let rep = breach_basins(&g, &mut z, &outlet, EPS, &pb(128));
    assert_eq!((rep.drained, rep.kept_bound), (0, 1), "{rep:?}");
}

// spec: GEN-174, TEST-121 (GEN-162's closed bowl: no lower cell exists, so nothing moves)
#[test]
fn a_closed_bowl_is_unchanged() {
    let n = 40;
    let g = Grid::new(n, n, 100.0);
    let mut z = vec![0.0; n * n];
    let mut outlet = vec![false; n * n];
    for j in 0..n {
        for i in 0..n {
            let ring = i.min(j).min(n - 1 - i).min(n - 1 - j);
            if ring < 4 {
                z[j * n + i] = 100.0;
            } else {
                z[j * n + i] = 5.0 * (ring as f64 - 4.0).min(3.0) * 0.0 + 1.0 * jitter(3, (j * n + i) as u64, 0);
            }
            outlet[j * n + i] = ring == 0;
        }
    }
    let before = z.clone();
    let rep = breach_basins(&g, &mut z, &outlet, EPS, &pb(128));
    assert_eq!(rep.drained, 0, "{rep:?}");
    assert!(z.iter().zip(&before).all(|(a, b)| a.to_bits() == b.to_bits()), "a closed bowl with no lower cell is unchanged");
}

const N: usize = 72;

fn terraced(salt: u64) -> Vec<f64> {
    let mut z = vec![0.0; N * N];
    for j in 0..N {
        for i in 0..N {
            let (x, y) = (i as f64, j as f64);
            let h = jitter(salt, (j * N + i) as u64, 0);
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

// spec: GEN-174, TEST-121
#[test]
fn the_breach_is_the_same_bits_in_the_carve_the_round_solver_on_any_thread_count_and_the_memoised_solve() {
    let g = Grid::new(N, N, 800.0);
    let z0 = terraced(3);
    let outlet = g.outlets(&z0, 0.0);
    let pre = PreBreach { passes: 1, halo: 40, max_depth_m: 500.0, min_lift_m: 0.5 };
    let mut probe = z0.clone();
    let rep = breach_basins(&g, &mut probe, &outlet, EPS, &pre);
    assert!(rep.drained >= 1, "the fixture has basins to drain: {rep:?}");
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, pre_breach: Some(pre), ..CarveParams::default() };
    let mut want = z0.clone();
    carve(&g, &mut want, &outlet, &p, None, 3);
    let mut plain = z0.clone();
    carve(&g, &mut plain, &outlet, &CarveParams { pre_breach: None, ..p }, None, 3);
    assert!(want.iter().zip(&plain).any(|(a, b)| a.to_bits() != b.to_bits()), "the pass moves the result");
    for threads in [1usize, 3, 8] {
        let mut got = z0.clone();
        solve_rounds(&g, &mut got, &outlet, &p, 4, &threads, 3);
        assert!(got.iter().zip(&want).all(|(a, b)| a.to_bits() == b.to_bits()), "G-ROUND with the breach: {threads} threads");
    }
    for tile in [16usize, 24] {
        let mut got = z0.clone();
        solve_rounds_memo(&g, &mut got, &outlet, &p, 4, &1usize, 3, &Drivers::default(), tile, &mut RoundCache::default());
        assert!(got.iter().zip(&want).all(|(a, b)| a.to_bits() == b.to_bits()), "G-PART with the breach: tile {tile}");
    }
}

// spec: GEN-174, TEST-121 (more passes drain the nested basins the first leaves; the result is still a function of the cells)
#[test]
fn more_passes_leave_fewer_closed_basins_and_never_raise_a_cell() {
    let g = Grid::new(N, N, 800.0);
    let z0 = terraced(5);
    let outlet = g.outlets(&z0, 0.0);
    let run = |passes: u32| {
        let mut z = z0.clone();
        breach_basins(&g, &mut z, &outlet, EPS, &PreBreach { passes, halo: 40, max_depth_m: 500.0, min_lift_m: 0.5 });
        z
    };
    let (one, four) = (run(1), run(4));
    assert!(four.iter().zip(&z0).all(|(a, b)| a <= b));
    assert!(lift_cells(&g, &four, &outlet, 0.5) <= lift_cells(&g, &one, &outlet, 0.5));
    assert!(lift_cells(&g, &one, &outlet, 0.5) < lift_cells(&g, &z0, &outlet, 0.5), "one pass drains basins");
    assert_eq!(run(4), four, "a rerun is the same bits");
}
