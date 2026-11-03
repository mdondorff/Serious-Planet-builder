//! Tier B, GEN-101's cost gate (m9-bake PR 3b): thermal stabilisation on active cells over one synthetic pass-2 tile (3,072 core cells and a 256-cell halo, 3,584 a side, 101.8 m),
//! 100 passes on 4 threads. The gate is a count (ADR 0087 point 6): the cells evaluated over all passes against the passes times the cells of the full run; the wall time is
//! reported and bounded only by a loaded-pass margin (CON-20, ADR 0087: a CPU time under load runs 3.6 to 5.6 times slower), the 5 s of the reference machine being measured there.
//! The ground is synthetic (SPIKE-61's real tiles are not in the repository), a rough upland and a steeper one; the real-tile gate is PR 5's.

use std::time::Instant;

use planet_generators::bake::thermal::{thermal, thermal_all_cells, ThermalParams};
use planet_generators::bake::Grid;

const W: usize = 3584;
const DX: f64 = 101.8;
/// The reference machine's gate for one tile (seconds, 100 passes, 4 threads); the assertion allows ADR 0087's loaded-pass margin of 4.
const GATE_S: f64 = 5.0;
const LOADED_MARGIN: f64 = 4.0;

/// A rough upland: broad hills with sharp incised valleys, land everywhere above 100 m. `relief` scales the heights above 0 (1.4 is a steep one).
fn tile(w: usize, relief: f64) -> Vec<f64> {
    (0..w * w)
        .map(|p| {
            let (x, y) = ((p % w) as f64, (p / w) as f64);
            let hills = 1200.0
                + 700.0 * libm::sin(x / 150.0) * libm::sin(y / 170.0 + 0.5)
                + 250.0 * libm::sin(x / 41.0 + 2.0 * libm::sin(y / 67.0));
            let valley = libm::fabs(libm::sin(x / 23.0 + 3.0 * libm::sin(y / 89.0)));
            relief * (hills - 420.0 * (1.0 - valley) * (1.0 - valley))
        })
        .collect()
}

/// Runs thermal on a tile and returns the share of the full run's cell evaluations that the active-cell run needed.
fn gate(relief: f64, bound_time: bool) -> f64 {
    let g = Grid::new(W, W, DX);
    let mut z = tile(W, relief);
    let mass: f64 = z.iter().sum();
    let started = Instant::now();
    let report = thermal(&g, &mut z, &ThermalParams::default(), 4);
    let seconds = started.elapsed().as_secs_f64();
    let share = report.evaluations as f64 / (report.passes_run as f64 * (W * W) as f64);
    println!(
        "thermal, relief x{relief}: {} passes in {seconds:.2} s on 4 threads, {:.1} % of the cells evaluated, {} cells moving in the last pass, {:.3e} m3 moved in the first",
        report.passes_run,
        share * 100.0,
        report.moved_last,
        report.moved_volume_first
    );
    assert!(report.moved_volume_first > 0.0, "the tile has no ground over the critical slope: the gate measured nothing");
    // The steep tile is gated by the count alone: its time is reported, a wall-time bound there would fail under load (ADR 0087 point 6).
    if bound_time {
        assert!(
            seconds < GATE_S * LOADED_MARGIN,
            "thermal on one tile took {seconds:.2} s (reference gate {GATE_S} s, loaded margin {LOADED_MARGIN})"
        );
    }
    let after: f64 = z.iter().sum();
    assert!((after - mass).abs() < 1e-9 * mass, "the mass changed by {} m", after - mass);
    share
}

// spec: GEN-101
#[test]
fn thermal_on_active_cells_needs_a_few_per_cent_of_the_cell_evaluations_on_an_upland_tile() {
    let share = gate(1.0, true);
    assert!(share < 0.05, "the active-cell run evaluated {:.1} % of the full run's cells (gate 5 %)", share * 100.0);
}

// spec: GEN-101
#[test]
fn thermal_on_a_steep_tile_is_gated_by_its_share_of_the_cell_evaluations() {
    let share = gate(1.4, false);
    assert!(share < 0.25, "the active-cell run evaluated {:.1} % of the full run's cells on steep ground (gate 25 %)", share * 100.0);
}

// spec: GEN-101
#[test]
fn a_window_of_a_grid_with_its_origin_gives_the_whole_grids_heights_away_from_its_border() {
    let (w, win, at) = (384usize, 256usize, 64usize);
    let params = ThermalParams { passes: 30, ..ThermalParams::default() };
    let mut whole = tile(w, 1.0);
    let mut part: Vec<f64> = (0..win * win).map(|p| whole[(at + p / win) * w + at + p % win]).collect();
    thermal(&Grid::new(w, w, DX), &mut whole, &params, 4);
    thermal(&Grid::new(win, win, DX).with_origin((at as u32, at as u32)), &mut part, &params, 4);
    // The window's border is a sink the whole grid does not have; one cell a pass carries it inward, so beyond the passes it cannot show.
    let near = params.passes + 2;
    let mut compared = 0;
    for j in near..win - near {
        for i in near..win - near {
            assert_eq!(
                part[j * win + i].to_bits(),
                whole[(at + j) * w + at + i].to_bits(),
                "the window differs from the whole grid at ({i}, {j})"
            );
            compared += 1;
        }
    }
    assert!(compared > 10_000, "only {compared} cells were compared");
}

// spec: GEN-101
#[test]
fn the_active_cell_run_equals_the_full_run_on_a_corner_of_the_tile() {
    let w = 384;
    let g = Grid::new(w, w, DX);
    let (mut a, mut b) = (tile(w, 1.0), tile(w, 1.0));
    let params = ThermalParams { passes: 40, ..ThermalParams::default() };
    thermal(&g, &mut a, &params, 4);
    thermal_all_cells(&g, &mut b, &params, 4);
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()), "the active-cell run differs from the full one");
}
