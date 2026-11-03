//! Tier B, GEN-101's timing gate (m9-bake PR 3b): thermal stabilisation on active cells over one pass-2 tile (3,072 core cells and a 256-cell halo, 3,584 a side, 101.8 m) runs in
//! under 5 s on 4 threads for 100 passes, moves mass-neutrally, and gives the bytes of the run over every cell on a window of the tile. SPIKE-64 measured the full-pass variant
//! at 16 to 41 s a tile; the active set is what makes the bake fit.

use std::time::Instant;

use planet_generators::bake::thermal::{thermal, thermal_all_cells, ThermalParams};
use planet_generators::bake::Grid;

const W: usize = 3584;
const DX: f64 = 101.8;
/// The gate of GEN-101's tile (seconds, 100 passes, 4 threads).
const GATE_S: f64 = 5.0;

/// A rough upland: broad hills with sharp incised valleys, land everywhere above 100 m, so a few per cent of the cells stand over the critical slope at the start.
fn tile(w: usize) -> Vec<f64> {
    (0..w * w)
        .map(|p| {
            let (x, y) = ((p % w) as f64, (p / w) as f64);
            let hills = 1200.0
                + 700.0 * libm::sin(x / 150.0) * libm::sin(y / 170.0 + 0.5)
                + 250.0 * libm::sin(x / 41.0 + 2.0 * libm::sin(y / 67.0));
            let valley = libm::fabs(libm::sin(x / 23.0 + 3.0 * libm::sin(y / 89.0)));
            hills - 420.0 * (1.0 - valley) * (1.0 - valley)
        })
        .collect()
}

// spec: GEN-101
#[test]
fn thermal_on_active_cells_runs_one_tile_in_under_5_s() {
    let g = Grid::new(W, W, DX);
    let mut z = tile(W);
    let mass: f64 = z.iter().sum();
    let params = ThermalParams::default();
    let started = Instant::now();
    let report = thermal(&g, &mut z, &params, 4);
    let seconds = started.elapsed().as_secs_f64();
    let share = report.evaluations as f64 / (report.passes_run as f64 * (W * W) as f64);
    println!(
        "thermal: {} passes in {seconds:.2} s on 4 threads, {:.1} % of the cells evaluated, {} cells moving in the last pass, {:.3e} m3 moved in the first",
        report.passes_run,
        share * 100.0,
        report.moved_last,
        report.moved_volume_first
    );
    assert!(report.moved_volume_first > 0.0, "the tile has no ground over the critical slope: the gate measured nothing");
    assert!(
        seconds < GATE_S,
        "thermal on one tile took {seconds:.2} s, the gate is {GATE_S} s ({} passes, {:.1} % of the cells evaluated)",
        report.passes_run,
        share * 100.0
    );
    let after: f64 = z.iter().sum();
    assert!((after - mass).abs() < 1e-9 * mass, "the mass changed by {} m", after - mass);
}

// spec: GEN-101
#[test]
fn the_active_cell_run_equals_the_full_run_on_a_window_of_the_tile() {
    let w = 384;
    let g = Grid::new(w, w, DX);
    let (mut a, mut b) = (tile(w), tile(w));
    let params = ThermalParams { passes: 40, ..ThermalParams::default() };
    thermal(&g, &mut a, &params, 4);
    thermal_all_cells(&g, &mut b, &params, 4);
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()), "the active-cell run differs from the full one");
}
