//! Tier A, TEST-095 (GEN-131, ADR 0105): the carve with deposition on synthetic surfaces: mass is kept (what is eroded is deposited or leaves), a pit receiving sediment rises toward its spill and
//! no higher, the result is the same bits on every run, and a coefficient of 0 is the carve without a sediment term. Small grids; no planet bake.

use planet_generators::bake::carve::{carve_rock_report, CarveParams, Rock};
use planet_generators::bake::Grid;

const DX: f64 = 800.0;
const N: usize = 24;

fn grid() -> Grid {
    Grid::new(N, N, DX)
}

/// A tilted plane draining west, `10 i` metres.
fn ramp() -> Vec<f64> {
    (0..N * N).map(|p| 10.0 * (p % N) as f64).collect()
}

/// The ramp with a pit of 40 m at `i` 8 to 10 in the middle row.
fn ramp_with_pit() -> Vec<f64> {
    let mut z = ramp();
    for i in 8..11 {
        z[12 * N + i] -= 40.0;
    }
    z
}

fn run(z0: &[f64], g: f64, steps: usize, keep_pits: bool) -> (Vec<f64>, f64, f64, f64) {
    let grid = grid();
    let outlet = grid.outlets(z0, -1.0);
    let p = CarveParams { steps, u_over_k: 20.0, uplift_taper_m: 0.0, keep_pits, deposit_g: g, ..CarveParams::default() };
    let mut z = z0.to_vec();
    let (_, rep) = carve_rock_report(&grid, &mut z, &outlet, &p, None, &Rock::none(), 3);
    (z, rep.eroded, rep.deposited, rep.exported)
}

fn bits(z: &[f64]) -> Vec<u64> {
    z.iter().map(|v| v.to_bits()).collect()
}

// spec: GEN-131
// spec: TEST-095
#[test]
fn what_is_eroded_is_deposited_or_leaves_through_an_outlet() {
    for (name, z0) in [("ramp", ramp()), ("ramp with a pit", ramp_with_pit())] {
        let (_, eroded, deposited, exported) = run(&z0, 1.0, 20, true);
        assert!(eroded > 0.0, "{name}: nothing was eroded");
        assert!(deposited > 0.0, "{name}: nothing was deposited at G = 1");
        let err = ((eroded - deposited) - exported).abs() / eroded;
        assert!(
            err < 1e-9,
            "{name}: eroded {eroded:.6e} - deposited {deposited:.6e} != exported {exported:.6e} (relative error {err:.2e})"
        );
    }
}

// spec: GEN-131
// spec: TEST-095
#[test]
fn a_pit_that_receives_sediment_rises_toward_its_spill_and_no_higher_than_its_receiver() {
    let z0 = ramp_with_pit();
    let pit = 12 * N + 9;
    let (off, ..) = run(&z0, 0.0, 20, true);
    let (on, ..) = run(&z0, 4.0, 20, true);
    assert!(on[pit] > off[pit] + 1.0, "the pit stands at {} with deposition and {} without", on[pit], off[pit]);
    // The spill of the pit is the ramp's height at its downstream rim (i = 7), 70 m, plus what the carve raised that cell by.
    assert!(on[pit] <= on[12 * N + 7] + 1e-6 + 40.0, "the pit stands at {} above its rim at {}", on[pit], on[12 * N + 7]);
    assert!(on.iter().all(|h| h.is_finite()), "a height is not finite");
}

// spec: GEN-131
// spec: TEST-095
#[test]
fn deposition_is_the_same_bits_on_every_run_and_a_zero_coefficient_adds_nothing() {
    let z0 = ramp_with_pit();
    let (a, ..) = run(&z0, 1.0, 20, true);
    let (b, ..) = run(&z0, 1.0, 20, true);
    assert_eq!(bits(&a), bits(&b));
    let (off, eroded, deposited, exported) = run(&z0, 0.0, 20, true);
    assert_eq!((eroded, deposited, exported), (0.0, 0.0, 0.0), "the account of a carve without deposition is empty");
    assert_ne!(bits(&off), bits(&a), "G = 1 changed nothing");
}

// ---- the basin infill (GEN-137, ADR 0109) ----

use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::infill::{infill, Infill, PIT_MIN_M};

const INFILL: Infill = Infill { min_area_km2: 25.0, min_depth_m: 50.0, residual_m: 100.0 };

/// A ramp (`10 i + 100`) with three basins: a small deep one (3 cells, 80 m), a large shallow one (49 cells, 20 m) and a large deep one (100 cells, 300 m).
fn three_basins() -> (Grid, Vec<f64>, Vec<bool>) {
    let n = 48;
    let g = Grid::new(n, n, DX);
    let mut z: Vec<f64> = (0..n * n).map(|p| 10.0 * (p % n) as f64 + 100.0).collect();
    let mut dig = |i0: usize, j0: usize, w: usize, depth: f64| {
        for j in j0..j0 + w {
            for i in i0..i0 + w {
                z[j * n + i] -= depth;
            }
        }
    };
    dig(10, 5, 2, 80.0); // 4 cells under 25 km2 (2.6 km2)
    dig(20, 20, 7, 20.0); // 49 cells (31 km2), shallow
    dig(30, 30, 10, 300.0); // 100 cells (64 km2), deep
    let outlet = g.outlets(&z, -1.0);
    (g, z, outlet)
}

fn basin_cells(g: &Grid, z: &[f64], outlet: &[bool]) -> Vec<(usize, f64)> {
    let f = priority_flood(g, z, outlet, 1e-3);
    (0..z.len()).filter(|&c| f[c] - z[c] > PIT_MIN_M).map(|c| (c, f[c] - z[c])).collect()
}

// spec: GEN-137
// spec: TEST-095
#[test]
fn the_infill_fills_small_and_shallow_basins_and_leaves_a_large_deep_one_at_the_residual_depth() {
    let (g, z0, outlet) = three_basins();
    let before = basin_cells(&g, &z0, &outlet);
    assert!(before.iter().any(|(_, d)| *d > 290.0), "the deep basin was not built");
    let mut z = z0.clone();
    let rep = infill(&g, &mut z, &outlet, &INFILL, 1e-3);
    let after = basin_cells(&g, &z, &outlet);
    assert!(!after.is_empty(), "the large deep basin vanished");
    let deepest = after.iter().map(|(_, d)| *d).fold(0.0, f64::max);
    assert!((deepest - INFILL.residual_m).abs() < 1.0, "the deepest left basin is {deepest} m, not the residual {} m", INFILL.residual_m);
    // Only the deep basin is left: its cells are around (30..40, 30..40).
    assert!(after.iter().all(|(c, _)| (30..40).contains(&(c % 48)) && (30..40).contains(&(c / 48))), "a small or shallow basin was left");
    assert_eq!(rep.cells_after, after.len());
    assert!(rep.cells_before > rep.cells_after);
}

// spec: GEN-137
// spec: TEST-095
#[test]
fn the_infill_never_raises_a_cell_above_its_spill_level_and_never_lowers_one() {
    let (g, z0, outlet) = three_basins();
    let flood = priority_flood(&g, &z0, &outlet, 1e-3);
    let mut z = z0.clone();
    infill(&g, &mut z, &outlet, &INFILL, 1e-3);
    for c in 0..z.len() {
        assert!(z[c] >= z0[c], "cell {c} was lowered from {} to {}", z0[c], z[c]);
        assert!(z[c] <= flood[c].max(z0[c]) + 1e-9, "cell {c} stands at {} above its spill level {}", z[c], flood[c]);
        assert!(!outlet[c] || z[c] == z0[c], "an outlet was changed");
    }
}

// spec: GEN-137
// spec: TEST-095
#[test]
fn the_infill_is_idempotent_deterministic_and_leaves_no_basin_under_the_thresholds() {
    let (g, z0, outlet) = three_basins();
    let (mut a, mut b) = (z0.clone(), z0.clone());
    infill(&g, &mut a, &outlet, &INFILL, 1e-3);
    infill(&g, &mut b, &outlet, &INFILL, 1e-3);
    assert_eq!(bits(&a), bits(&b), "two runs differ");
    let mut again = a.clone();
    infill(&g, &mut again, &outlet, &INFILL, 1e-3);
    assert_eq!(bits(&again), bits(&a), "a second infill changed the surface");
    // No basin is smaller or shallower than the thresholds.
    let cells = basin_cells(&g, &a, &outlet);
    let deepest = cells.iter().map(|(_, d)| *d).fold(0.0, f64::max);
    assert!(deepest >= INFILL.min_depth_m, "a basin of {deepest} m is under the minimum depth");
    assert!(cells.len() as f64 * DX * DX * 1e-6 >= INFILL.min_area_km2, "the basins left are under the minimum area");
}
