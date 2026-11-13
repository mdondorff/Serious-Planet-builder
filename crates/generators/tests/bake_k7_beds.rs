#![allow(clippy::needless_range_loop)]
//! Tier A, GEN-176 (TEST-124; architect, 2026-11-12): the K7 channel beds and the stages after the burn. A river bed never stands below the sea level on land (the floor applies after the 0.5 m code of
//! the bed rounds down); the channel cells are fixed for thermal, the deposition sweep and pit spreading (the material that would enter leaves the tile as exported sediment, counted); the reports count every
//! bound (the final pass counts all path cells, the burn reports its own cutoff). Small tiles; no store read.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::deposit::{deposit_on_own_route_fixed, DepositParams, Kc};
use planet_generators::bake::k7::{burn_channels_report, final_channel_pass, k7_tile, line_cells, Channel, K7Params, QUnit};
use planet_generators::bake::thermal::{thermal, thermal_fixed, ThermalParams};
use planet_generators::bake::tile::{TileGrid, TileId};
use planet_generators::bake::Grid;

fn tile_grid() -> TileGrid {
    TileGrid { cells: 384, tile: 96, halo: 24, radius_m: 100.0 * 384.0 * 2.0 / core::f64::consts::PI }
}

fn id() -> TileId {
    TileId { face: Face(1), ti: 1, tj: 2 }
}

fn face_xy(g: &TileGrid, d: Vec3) -> (f64, f64) {
    let n = g.cells as f64;
    let face = id().face;
    let w = d.dot(face.normal());
    let plane = |u: f64| libm::atan(u) / core::f64::consts::FRAC_PI_4;
    let (s, t) = (plane(d.dot(face.u_axis()) / w), plane(d.dot(face.v_axis()) / w));
    ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5)
}

fn cell(g: &TileGrid, a: i64, b: i64) -> (f64, f64) {
    let (t, h) = (g.tile as i64, g.halo as i64);
    ((i64::from(id().ti) * t - h + a) as f64, (i64::from(id().tj) * t - h + b) as f64)
}

fn direction(g: &TileGrid, x: f64, y: f64) -> Vec3 {
    face_to_direction(&TangentWarp, id().face, 2.0 * (x + 0.5) / g.cells as f64 - 1.0, 2.0 * (y + 0.5) / g.cells as f64 - 1.0)
}

/// A coastal plain falling 0.25 m a cell to the east from +20 m at the west edge of the window to -4 m at its east end.
fn coast(d: Vec3) -> f64 {
    let (x, _) = face_xy(&tile_grid(), d);
    20.0 - 0.25 * (x - 72.0)
}

/// A steep valley side: 1.5 m a cell to the east.
fn steep(d: Vec3) -> f64 {
    let (x, _) = face_xy(&tile_grid(), d);
    600.0 - 1.5 * (x - 72.0)
}

/// A channel along a row of the window from its west to its east border; `bed(x, k)` the level at the point `k` (face cell column `x`).
fn channel(g: &TileGrid, row: i64, bed: &dyn Fn(f64, usize) -> f64) -> Channel {
    let w = (g.tile + 2 * g.halo) as i64;
    let mut points: Vec<(f64, f64)> = (0..w).step_by(4).map(|a| cell(g, a, row)).collect();
    points.push(cell(g, w - 1, row));
    let levels = points.iter().enumerate().map(|(k, &(x, _))| bed(x, k)).collect();
    Channel { points, levels, q: Vec::new(), q_unit: QUnit::Discharge }
}

/// The window cell of a face cell.
fn window_cell(g: &TileGrid, x: i64, y: i64) -> Option<usize> {
    let (t, h) = (g.tile as i64, g.halo as i64);
    let (a, b) = (x - (i64::from(id().ti) * t - h), y - (i64::from(id().tj) * t - h));
    let w = t + 2 * h;
    (a >= 0 && b >= 0 && a < w && b < w).then(|| (b * w + a) as usize)
}

// spec: GEN-176, TEST-124 (a)
#[test]
fn a_line_across_a_low_coastal_plain_has_no_bed_below_the_sea_level_and_the_profile_holds() {
    let g = tile_grid();
    let row = g.halo as i64 + 20;
    // beds 6 m under the plain, with the 0.3 m steps of a code that rounds down: they run to -10 m near the coast
    let ch = channel(&g, row, &|x, k| 20.0 - 0.25 * (x - 72.0) - 6.0 - 0.3 * f64::from((k % 3) as u8));
    let none = |_: Vec3| 0.0;
    let p = K7Params::standard(5, 0.0);
    let r = k7_tile(&g, id(), &coast, &none, std::slice::from_ref(&ch), &p, 2);
    let line = line_cells(&ch);
    let mut land_cells = 0;
    let mut prev: Option<usize> = None;
    for &(x, y) in &line {
        let Some(c) = window_cell(&g, x, y) else {
            prev = None;
            continue;
        };
        let start = coast(direction(&g, x as f64, y as f64));
        if start > 0.0 {
            land_cells += 1;
            assert!(r.z[c] >= 0.0, "the bed at ({x}, {y}) stands at {} m: below the sea level on land (start {start} m)", r.z[c]);
        }
        if let Some(pc) = prev {
            assert!(r.z[c] <= r.z[pc], "a rise of {} m along the line at ({x}, {y})", r.z[c] - r.z[pc]);
        }
        prev = Some(c);
    }
    assert!(land_cells > 50, "the line crosses the plain: {land_cells} land cells");
    // G-SED closes
    let s = r.sediment;
    assert!((s.eroded - (s.deposited + s.exported)).abs() <= 1e-9 * s.eroded.max(1.0), "{s:?}");
}

// spec: GEN-176, TEST-124 (b)
#[test]
fn the_channel_cells_are_fixed_after_the_burn_and_the_volume_that_would_enter_them_leaves_as_exported_sediment() {
    // the stages on their own: a one-cell trench of 30 m in a slope of 1.5 m a cell, one thermal pass
    let (n, trench_row) = (41usize, 20usize);
    let g = Grid::new(n, n, 10.0);
    let z0: Vec<f64> = (0..n * n)
        .map(|p| {
            let (i, j) = (p % n, p / n);
            let slope = 600.0 - 1.5 * i as f64;
            if j == trench_row && (4..n - 4).contains(&i) {
                slope - 30.0
            } else {
                slope
            }
        })
        .collect();
    let fixed: Vec<bool> = (0..n * n).map(|p| p / n == trench_row && (4..n - 4).contains(&(p % n))).collect();
    let tp = ThermalParams { passes: 1, ..ThermalParams::default() };
    let (mut free, mut held) = (z0.clone(), z0.clone());
    thermal(&g, &mut free, &tp, 2);
    let rep = thermal_fixed(&g, &mut held, &tp, 2, &fixed);
    let area = 10.0 * 10.0;
    let mut would_enter = 0.0;
    for c in 0..n * n {
        if fixed[c] {
            assert_eq!(held[c].to_bits(), z0[c].to_bits(), "a fixed cell is not changed by thermal");
            would_enter += (free[c] - z0[c]).max(0.0) * area;
        }
    }
    assert!(would_enter > 1.0e3, "the trench would take {would_enter} m3 in the pass");
    assert!(
        (rep.exported_to_fixed_m3 - would_enter).abs() <= 1e-9 * would_enter,
        "exported {} m3 against {would_enter} m3 that would enter",
        rep.exported_to_fixed_m3
    );
    // deposition: what would settle on a fixed cell leaves and is counted
    let original = z0.clone();
    let mut z = z0.clone();
    // carve the slope so that there is something to deposit: the cells beside the trench lose 5 m
    for p in 0..n * n {
        if p / n != trench_row && p / n + 1 >= trench_row && p / n <= trench_row + 1 {
            z[p] -= 5.0;
        }
    }
    let params = DepositParams { kc: Kc::Uniform(1.0), zeta: 0.5, sea_level: 0.0 };
    let (mut a, mut b) = (z.clone(), z.clone());
    let free_rep = deposit_on_own_route_fixed(&g, &mut a, &original, 1e-3, 3, &params, true, &[]);
    let held_rep = deposit_on_own_route_fixed(&g, &mut b, &original, 1e-3, 3, &params, true, &fixed);
    for c in 0..n * n {
        if fixed[c] {
            assert_eq!(b[c].to_bits(), z[c].to_bits(), "a fixed cell keeps its height in the deposition sweep");
        }
    }
    assert_eq!(free_rep.exported_to_fixed, 0.0);
    assert!(held_rep.exported >= held_rep.exported_to_fixed);
    assert!(
        (held_rep.eroded - (held_rep.deposited + held_rep.exported)).abs() <= 1e-9 * held_rep.eroded.max(1.0),
        "G-SED closes: {held_rep:?}"
    );
}

// spec: GEN-176, TEST-124 (b)
#[test]
fn a_burned_trench_in_steep_terrain_keeps_its_depth_through_the_whole_pass() {
    let g = tile_grid();
    let row = g.halo as i64 + 20;
    let ch = channel(&g, row, &|x, _| 600.0 - 1.5 * (x - 72.0) - 120.0);
    let none = |_: Vec3| 0.0;
    let p = K7Params::standard(5, 0.0);
    let r = k7_tile(&g, id(), &steep, &none, std::slice::from_ref(&ch), &p, 2);
    assert!(r.fixed_cells > 100, "the channel cells are fixed: {}", r.fixed_cells);
    assert!(r.thermal_exported_m3 > 0.0, "the walls of the trench gave to it: {} m3", r.thermal_exported_m3);
    for (k, &(x, y)) in ch.points.iter().enumerate() {
        if let Some(c) = window_cell(&g, x as i64, y as i64) {
            assert!(
                r.z[c] <= ch.levels[k] + 1e-6,
                "the trench at point {k} stands at {} m over its bed of {} m: filled by a stage after the burn",
                r.z[c],
                ch.levels[k]
            );
        }
    }
    let s = r.sediment;
    assert!((s.eroded - (s.deposited + s.exported)).abs() <= 1e-9 * s.eroded.max(1.0), "G-SED closes: {s:?}");
}

fn row_channel(len: usize, y: f64) -> Channel {
    Channel { points: vec![(0.0, y), ((len - 1) as f64, y)], levels: vec![100.0; 2], q: Vec::new(), q_unit: QUnit::Discharge }
}

// spec: GEN-176, TEST-124 (c)
#[test]
fn the_final_pass_counts_every_path_cell_and_the_burn_reports_its_own_cutoff() {
    let w = 8;
    // a line with a hole at its second cell (5 m under the sea) and a sea outlet at its end (-1, -2 m): carry 3
    let row = [50.0, -5.0, 50.0, 50.0, 50.0, 50.0, -1.0, -2.0];
    let ch = row_channel(w, 0.0);
    let fresh = || -> Vec<f64> { (0..w * w).map(|p| if p / w == 0 { row[p % w] } else { 99.0 }).collect() };
    let mut z = fresh();
    let pass = final_channel_pass(&mut z, w, (0, 0), std::slice::from_ref(&ch), 0.0, 1.0, 3);
    // the cells 5, 6, 7 are more than 3 steps from the hole: the sub-sea cells 6 and 7 count too
    assert_eq!(pass.cutoff_binds, 3, "{pass:?}");
    assert_eq!(
        (pass.cutoff_diff.p50, pass.cutoff_diff.p99, pass.cutoff_diff.max),
        (4.0, 55.0, 55.0),
        "the bounded minus the unbounded target at cells 5, 6, 7 is 55, 4, 3 m: {pass:?}"
    );
    let mut z = fresh();
    let burn = burn_channels_report(&mut z, w, (0, 0), std::slice::from_ref(&ch), None, 3, None);
    assert_eq!(burn.cutoff_binds, 3, "{burn:?}");
    assert_eq!(burn.cutoff_diff.max, 55.0, "{burn:?}");
    let mut z = fresh();
    assert_eq!(
        burn_channels_report(&mut z, w, (0, 0), std::slice::from_ref(&ch), None, usize::MAX, None).cutoff_binds,
        0,
        "an unbounded carry has no cutoff"
    );
    // K7 reports both in the result
    let g = tile_grid();
    let row_k = g.halo as i64 + 20;
    let k7ch = channel(&g, row_k, &|x, _| 20.0 - 0.25 * (x - 72.0) - 6.0);
    let hole = |d: Vec3| {
        let (x, y) = face_xy(&tile_grid(), d);
        let (hx, hy) = cell(&tile_grid(), 30, row_k);
        if (x - hx).abs() <= 1.0 && (y - hy).abs() <= 1.0 {
            -25.0
        } else {
            0.0
        }
    };
    let r = k7_tile(&g, id(), &coast, &hole, std::slice::from_ref(&k7ch), &K7Params::standard(5, 0.0), 2);
    assert!(r.burn_cutoff_binds > 0, "the burn's cutoff is reported: {}", r.burn_cutoff_binds);
    assert!(r.final_pass.cutoff_binds > 0, "the final pass's cutoff is reported: {:?}", r.final_pass);
    let mark = r.mark_line();
    assert!(mark.contains("channel cells fixed") && mark.contains("burn cutoff") && mark.contains("final pass cutoff"), "{mark}");
}
// spec: GEN-176, TEST-124 (b)
#[test]
fn pit_spreading_leaves_fixed_cells_alone_and_keeps_the_volume() {
    use planet_generators::bake::deposit::{spread_pits, spread_pits_fixed};
    let n = 9;
    let g = Grid::new(n, n, 100.0);
    // a plateau of 50 m with a one-cell pit of 3 m in the middle
    let mut z = vec![50.0; n * n];
    z[4 * n + 4] = 47.0;
    let before = z.clone();
    let mut free = z.clone();
    assert!(spread_pits(&g, &mut free, 0.0, 0.5, 3) > 0, "the free pit is spread");
    // the pit is a fixed cell: nothing moves
    let mut fixed = vec![false; n * n];
    fixed[4 * n + 4] = true;
    spread_pits_fixed(&g, &mut z, 0.0, 0.5, 3, &fixed);
    assert_eq!(z, before, "a fixed pit is not filled");
    // a neighbour that is fixed is not lowered, and the rise is the share of the free neighbours, so the volume is kept
    let mut fixed = vec![false; n * n];
    fixed[4 * n + 5] = true;
    fixed[3 * n + 3] = true;
    let mut z = before.clone();
    spread_pits_fixed(&g, &mut z, 0.0, 0.5, 1, &fixed);
    assert_eq!(z[4 * n + 5].to_bits(), before[4 * n + 5].to_bits());
    assert_eq!(z[3 * n + 3].to_bits(), before[3 * n + 3].to_bits());
    assert!(z[4 * n + 4] > before[4 * n + 4], "the pit is filled from the free neighbours");
    let (sb, sa): (f64, f64) = (before.iter().sum(), z.iter().sum());
    assert!((sa - sb).abs() < 1e-9, "the volume is kept: {} against {}", sa, sb);
    // with no fixed cell it is the free run, bit for bit
    let mut z = before.clone();
    spread_pits_fixed(&g, &mut z, 0.0, 0.5, 3, &[]);
    assert!(z.iter().zip(&free).all(|(a, b)| a.to_bits() == b.to_bits()));
}

// spec: GEN-176, TEST-124 (c) (numerics review: the burn's cutoff counts the window's cells on the heights the burn applies)
#[test]
fn the_burn_cutoff_counts_cells_of_the_window_only_and_on_floored_heights() {
    // a line of 12 cells in a window of 8 columns: the hole and every cutoff cell lie outside the window
    let w = 8;
    let ch = Channel { points: vec![(0.0, 0.0), (23.0, 0.0)], levels: vec![100.0; 2], q: Vec::new(), q_unit: QUnit::Discharge };
    let mut z = vec![60.0; w * w];
    let r = burn_channels_report(&mut z, w, (8, 0), std::slice::from_ref(&ch), None, 3, None);
    // the cells 8..16 are in the window; the hole (cell 0) is 8 steps from the window: no cell of the window has a cutoff
    assert_eq!(r.cutoff_binds, 0, "{r:?}");
    // with the floor: both targets below the sea level are floored to it, and nothing changes
    let ch = Channel { points: vec![(0.0, 0.0), (7.0, 0.0)], levels: vec![-10.0, -10.0], q: Vec::new(), q_unit: QUnit::Discharge };
    let mut z: Vec<f64> = (0..w * w).map(|p| if p == 0 { -20.0 } else { 5.0 }).collect();
    let floored = burn_channels_report(&mut z, w, (0, 0), std::slice::from_ref(&ch), None, 3, Some(0.0));
    assert_eq!((floored.cutoff_binds, floored.cutoff_diff.max), (0, 0.0), "both targets are floored to 0 m: {floored:?}");
    let mut z: Vec<f64> = (0..w * w).map(|p| if p == 0 { -20.0 } else { 5.0 }).collect();
    let raw = burn_channels_report(&mut z, w, (0, 0), std::slice::from_ref(&ch), None, 3, None);
    assert!(raw.cutoff_binds > 0 || raw.cutoff_diff.max >= 0.0);
}

// spec: GEN-176, TEST-124 (the stages with an all-false mask are the free stages, bit for bit)
#[test]
fn an_all_false_fixed_mask_is_the_free_run_in_every_stage() {
    use planet_generators::bake::carve::{carve, carve_fixed, CarveParams};
    use planet_generators::bake::deposit::{spread_pits, spread_pits_fixed};
    let n = 40;
    let g = Grid::new(n, n, 50.0);
    let z0: Vec<f64> = (0..n * n)
        .map(|p| 300.0 + 0.7 * (p % n) as f64 + 9.0 * libm::sin(0.7 * (p / n) as f64) + 4.0 * libm::cos(0.9 * (p % n) as f64))
        .collect();
    let none = vec![false; n * n];
    let tp = ThermalParams::default();
    let (mut a, mut b) = (z0.clone(), z0.clone());
    thermal(&g, &mut a, &tp, 2);
    thermal_fixed(&g, &mut b, &tp, 2, &none);
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()), "thermal");
    let (mut a, mut b) = (z0.clone(), z0.clone());
    spread_pits(&g, &mut a, 0.0, 0.1, 3);
    spread_pits_fixed(&g, &mut b, 0.0, 0.1, 3, &none);
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()), "pit spreading");
    let outlet = g.outlets(&z0, 0.0);
    let p = CarveParams::default();
    let (mut a, mut b) = (z0.clone(), z0.clone());
    carve(&g, &mut a, &outlet, &p, None, 3);
    carve_fixed(&g, &mut b, &outlet, &p, None, 3, &none);
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()), "the carve");
}
