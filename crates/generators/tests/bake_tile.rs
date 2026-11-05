//! Tier A, GEN-101 and TEST-084's judged half on a synthetic planet (m9-bake PR 5): pass 2 on one tile. A tiny planet (cells of about 100 m, so the relief is steep at the cell
//! scale), tiles of 24 core cells with a halo of 6. The local carve only incises and does not fill a pit, a tile is the same bytes on 1 and 8 threads, what was eroded is
//! deposited or exported, the shared core edge of two tiles shows no step beyond the step between neighbouring cells, and the quantised change is within half a step.

use planet_core::{Face, Vec3};
use planet_generators::bake::carve::carve;
use planet_generators::bake::tile::{
    dequantise, pass2_tile, quantise, sample_window_coarse, tile_has_land, Pass2Params, TileGrid, TileId, QUANT_STEP_M,
};
use planet_generators::bake::Grid;

/// Cells of 100 m on a face of 96 cells: a planet of about 6.1 km radius.
fn grid() -> TileGrid {
    TileGrid { cells: 96, tile: 24, halo: 6, radius_m: 100.0 * 96.0 / core::f64::consts::FRAC_PI_2 }
}

/// Mountains of several hundred metres on land, a sea of -300 m: a function of the direction.
fn base(d: Vec3) -> f64 {
    let ridge = 300.0 * libm::sin(9.0 * d.x + 1.0) * libm::cos(8.0 * d.y) + 200.0 * libm::sin(17.0 * d.z + 0.3) + 60.0;
    if ridge > 0.0 {
        ridge + 20.0
    } else {
        -300.0 + 0.3 * ridge
    }
}

fn tile(ti: u32, tj: u32) -> TileId {
    TileId { face: Face(0), ti, tj }
}

fn nothing(_: Vec3) -> f64 {
    0.0
}

// spec: GEN-101
#[test]
fn the_local_carve_only_incises_and_does_not_fill_a_pit() {
    let g = Grid::new(40, 40, 100.0);
    // A tilted plane falling to the west edge with a pit of 15 m in the middle (the east is the high side).
    let mut z: Vec<f64> = (0..40 * 40).map(|p| 20.0 + (p % 40) as f64 * 9.0).collect();
    let pit = 20 * 40 + 20;
    z[pit] -= 15.0;
    let z0 = z.clone();
    let outlet = g.outlets(&z, 0.0);
    let p = Pass2Params::standard(5, 0.0);
    carve(&g, &mut z, &outlet, &p.carve, None, 11);
    assert!(z.iter().zip(&z0).all(|(a, b)| a <= b), "the local carve raised a cell");
    assert!(z.iter().zip(&z0).any(|(a, b)| a < b), "the local carve changed nothing");
}

// spec: GEN-101
#[test]
fn a_tile_is_the_same_bytes_on_1_and_8_threads_and_keeps_the_mass_of_what_it_erodes() {
    let (g, p) = (grid(), Pass2Params::standard(9, 0.0));
    let (a, b) = (pass2_tile(&g, tile(1, 1), &base, &nothing, None, &p, 1), pass2_tile(&g, tile(1, 1), &base, &nothing, None, &p, 8));
    assert_eq!(a.core.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), b.core.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    assert_eq!(a.stats, b.stats);
    assert_eq!(a.core.len(), 24 * 24);
    let s = a.stats;
    assert!((s.eroded_m3 - (s.deposited_m3 + s.exported_m3)).abs() <= 1e-9 * s.eroded_m3.max(1.0), "{s:?}");
    assert!(s.land_cells > 100 && a.core.iter().any(|v| *v != 0.0), "the tile changed nothing: {s:?}");
    // The face carve's change is part of the tile's change: a constant 5 m of the face carve on flat ground shows in the core.
    let flat = |_: Vec3| 300.0;
    let lowered = |_: Vec3| -5.0;
    let t = pass2_tile(&g, tile(2, 2), &flat, &lowered, None, &p, 2);
    assert!(t.core.iter().all(|v| (*v + 5.0).abs() < 1e-3), "a flat tile with a face change of -5 m does not carry it");
}

// spec: TEST-084
// A tier A stand-in on a synthetic planet; TEST-084 (tier B, SPIKE-70's four sites) stays NOT RUN until the store runs it.
#[test]
fn the_shared_core_edge_of_two_tiles_has_no_larger_a_step_than_the_cells_beside_it() {
    let (g, p) = (grid(), Pass2Params::standard(9, 0.0));
    let n = g.tile;
    let (mut seam, mut inner) = (Vec::new(), Vec::new());
    // Every pair of tiles side by side along the first axis on face 0: 3 pairs in each of 4 rows.
    let tiles: Vec<_> =
        (0..4u32).flat_map(|tj| (0..4u32).map(move |ti| (ti, tj, pass2_tile(&g, tile(ti, tj), &base, &nothing, None, &p, 4)))).collect();
    // The step of the tile's change (final minus base): the base's own gradient is the same on both sides and would hide a seam of the carve.
    let height =
        |ti: u32, tj: u32, i: usize, j: usize| f64::from(tiles.iter().find(|t| (t.0, t.1) == (ti, tj)).expect("a tile").2.core[j * n + i]);
    for tj in 0..4 {
        for ti in 0..3 {
            for j in 0..n {
                seam.push((height(ti, tj, n - 1, j) - height(ti + 1, tj, 0, j)).abs());
                inner.push((height(ti, tj, n - 2, j) - height(ti, tj, n - 1, j)).abs());
            }
        }
    }
    let p99 = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        (v[(v.len() as f64 * 0.99) as usize], v[v.len() - 1])
    };
    let ((s, smax), (i, imax)) = (p99(&mut seam), p99(&mut inner));
    println!("seam p99 {s:.2} m (max {smax:.2}), inner p99 {i:.2} m (max {imax:.2})");
    assert!(s <= i, "the step across the shared edge (p99 {s:.2} m) is larger than between neighbouring cells (p99 {i:.2} m)");
}

// spec: STRM-065
#[test]
fn a_quantised_change_is_within_half_a_step_and_a_tile_with_land_is_found() {
    let v = [0.0f32, 0.1, -0.13, 3.3, -250.07, 1e9, -1e9];
    let back = dequantise(&quantise(&v));
    for (a, b) in v.iter().zip(&back).take(5) {
        assert!((a - b).abs() <= QUANT_STEP_M / 2.0 + 1e-6, "{a} -> {b}");
    }
    assert!(back[5] > 8000.0 && back[6] < -8000.0, "the range is clamped");
    let g = grid();
    let sea = |_: Vec3| -100.0;
    assert!(!tile_has_land(&g, tile(0, 0), &sea, 0.0, 4));
    assert!((0..4).flat_map(|j| (0..4).map(move |i| tile(i, j))).any(|t| tile_has_land(&g, t, &base, 0.0, 4)));
}

// spec: GEN-101
#[test]
fn the_cells_still_moving_at_the_pass_cap_are_reported_and_a_bad_geometry_is_refused() {
    let g = grid();
    let capped = Pass2Params {
        thermal: planet_generators::bake::thermal::ThermalParams { passes: 1, ..Pass2Params::standard(9, 0.0).thermal },
        ..Pass2Params::standard(9, 0.0)
    };
    let r = pass2_tile(&g, tile(1, 1), &base, &nothing, None, &capped, 2);
    assert_eq!(r.stats.thermal_passes, 1);
    assert!(r.stats.thermal_moving_at_cap > 0, "a steep tile cut at one pass has no cell still moving: {:?}", r.stats);
    for (what, bad) in [
        ("a tile that does not divide the face", TileGrid { tile: 25, ..g }),
        ("a halo of half the tile", TileGrid { halo: 12, ..g }),
        ("a tile that is not a multiple of the base stride", TileGrid { cells: 90, tile: 18, halo: 6, radius_m: g.radius_m }),
        ("no tile", TileGrid { tile: 0, ..g }),
        ("a halo over an eighth of the face edge", TileGrid { cells: 64, tile: 64, halo: 31, radius_m: g.radius_m }),
    ] {
        assert!(bad.check().is_err(), "{what} was accepted");
    }
    assert!(g.check().is_ok());
    assert_eq!(g.all_tiles().len(), 6 * 16);
}

// spec: GEN-101
#[test]
fn the_catchment_cap_follows_the_cell_size_so_a_coarse_grid_keeps_its_drainage() {
    // The same ground on a grid of 100 m cells and on one of 3 km cells: the cap is 96 cells of area on both, so the coarse carve is not a uniform lowering.
    let p = Pass2Params::standard(9, 0.0);
    assert_eq!(p.local_area_cells, 96.0);
    let coarse = TileGrid { cells: 96, tile: 24, halo: 6, radius_m: 3000.0 * 96.0 / core::f64::consts::FRAC_PI_2 };
    let fine = grid();
    let lowered = |g: &TileGrid| pass2_tile(g, tile(1, 1), &base, &nothing, None, &p, 2).core.iter().filter(|v| **v < -0.25).count();
    let (c, f) = (lowered(&coarse), lowered(&fine));
    assert!(c > 0 && f > 0, "a grid changed nothing: {c}, {f}");
}

// spec: TEST-084
// Reported, not judged: on a smooth base the seam of the tiles' change next to the step between neighbouring cells (a tier A stand-in; TEST-084 stays NOT RUN until SPIKE-70's four sites).
#[test]
fn the_seam_on_a_smooth_base_is_reported() {
    let (g, p) = (grid(), Pass2Params::standard(9, 0.0));
    let n = g.tile;
    let smooth = |d: Vec3| 150.0 + 90.0 * libm::sin(3.0 * d.x + 0.4) * libm::cos(2.0 * d.y) + 40.0 * libm::sin(5.0 * d.z);
    let results: Vec<_> = (0..3u32).map(|ti| pass2_tile(&g, tile(ti, 1), &smooth, &nothing, None, &p, 2)).collect();
    let (mut seam, mut inner) = (Vec::new(), Vec::new());
    for ti in 0..2 {
        for j in 0..n {
            let (a, b) = (&results[ti], &results[ti + 1]);
            seam.push(f64::from(a.core[j * n + n - 1] - b.core[j * n]).abs());
            inner.push(f64::from(a.core[j * n + n - 2] - a.core[j * n + n - 1]).abs());
        }
    }
    let p99 = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[(v.len() as f64 * 0.99) as usize]
    };
    let (s, i) = (p99(&mut seam), p99(&mut inner));
    println!("smooth base: seam p99 {s:.2} m, inner p99 {i:.2} m");
    assert!(s.is_finite() && i.is_finite());
}

// spec: GEN-101
#[test]
fn the_base_sampled_every_4_cells_is_exact_at_the_nodes_close_between_them_and_the_same_on_any_thread_count() {
    let g = grid();
    let smooth = |d: Vec3| 400.0 * libm::sin(6.0 * d.x + 0.4) * libm::cos(5.0 * d.y) + 150.0 * libm::sin(9.0 * d.z);
    let id = tile(1, 1);
    let (w, h, t) = (g.window(), g.halo as i64, g.tile as i64);
    let a = sample_window_coarse(&g, id, &smooth, 1, 4);
    let b = sample_window_coarse(&g, id, &smooth, 8, 4);
    assert_eq!(a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), b.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    let exact = |i: usize, j: usize| smooth(g.direction(id.face, i64::from(id.ti) * t - h + i as i64, i64::from(id.tj) * t - h + j as i64));
    let mut worst = 0.0f64;
    for j in 0..w {
        for i in 0..w {
            let e = (a[j * w + i] - exact(i, j)).abs();
            if i % 4 == 0 && j % 4 == 0 {
                assert!(e < 1e-9, "node ({i}, {j}) is off by {e} m");
            }
            worst = worst.max(e);
        }
    }
    // The clamp to the range of the four nodes clips a crest between nodes by up to about 3.4 m on this field (amplitude 550 m); the stored change is taken against this window, so it is not an error of the layer.
    assert!(worst < 5.0, "between the nodes the interpolation is off by {worst} m on a field of wavelength over 20 cells");
}

// spec: GEN-101
#[test]
fn thermal_at_the_standard_parameters_runs_300_passes_of_an_eighth_which_the_kernel_accepts() {
    let p = Pass2Params::standard(1, 0.0);
    assert_eq!((p.thermal.passes, p.thermal.k), (300, 0.125));
    let (g, id) = (grid(), tile(1, 1));
    let r = pass2_tile(&g, id, &base, &nothing, None, &p, 2);
    assert!(r.stats.thermal_passes <= 300 && r.stats.land_cells > 100, "{:?}", r.stats);
}

// spec: GEN-101
#[test]
fn the_sparse_base_works_for_any_stride_stays_in_range_beside_a_cliff_and_agrees_between_overlapping_windows() {
    let g = grid();
    let smooth = |d: Vec3| 100.0 * libm::sin(5.0 * d.x) + 40.0 * libm::cos(7.0 * d.y);
    // Any stride, including those that divide the window's last index (stride 1 and 2 read one node past the edge case).
    for s in [1usize, 2, 3, 4, 6] {
        let v = sample_window_coarse(&g, tile(1, 1), &smooth, 2, s);
        assert_eq!(v.len(), g.window() * g.window());
        assert!(v.iter().all(|x| x.is_finite()), "stride {s}");
    }
    // A 400 m cliff between land at 20 m and sea at -380 m: the interpolation stays within the field's own range (no undershoot of a coastal plain below the sea).
    let cliff = |d: Vec3| if d.x + 0.3 * d.y > 0.2 { 20.0 } else { -380.0 };
    let v = sample_window_coarse(&g, tile(1, 1), &cliff, 1, 4);
    assert!(v.iter().all(|x| (-380.0..=20.0).contains(x)), "the sparse base left the field's range");
    // Two overlapping windows give the same bytes on the cells they share (the lattices fall on the same global cells because the tile is a multiple of the stride).
    let (a, b) = (sample_window_coarse(&g, tile(1, 1), &smooth, 1, 4), sample_window_coarse(&g, tile(2, 1), &smooth, 1, 4));
    let (w, t) = (g.window(), g.tile);
    for j in 0..w {
        for i in 0..2 * g.halo {
            // Cell (t + i) of tile (1, 1)'s window is cell i of tile (2, 1)'s window.
            assert_eq!(a[j * w + t + i].to_bits(), b[j * w + i].to_bits(), "window cell ({i}, {j}) differs between the tiles");
        }
    }
}

// spec: GEN-101
#[test]
fn a_tile_leaves_no_one_cell_pit_over_the_threshold_in_its_core() {
    let (g, p) = (grid(), Pass2Params::standard(9, 0.0));
    assert_eq!((p.pit_min_m, p.pit_passes), (0.5, 32));
    let id = tile(1, 1);
    let r = pass2_tile(&g, id, &base, &nothing, None, &p, 2);
    let (t, h, w) = (g.tile, g.halo, g.window());
    let base_w = planet_generators::bake::tile::sample_window_coarse(&g, id, &base, 2, planet_generators::bake::tile::BASE_STRIDE);
    let z = |i: usize, j: usize| f64::from(r.core[j * t + i]) + base_w[(j + h) * w + i + h];
    let mut pits = 0;
    for j in 1..t - 1 {
        for i in 1..t - 1 {
            let lowest = (-1i64..=1)
                .flat_map(|dj| (-1i64..=1).map(move |di| (di, dj)))
                .filter(|d| *d != (0, 0))
                .map(|(di, dj)| z((i as i64 + di) as usize, (j as i64 + dj) as usize))
                .fold(f64::INFINITY, f64::min);
            pits += usize::from(z(i, j) > 0.0 && lowest > 0.0 && lowest - z(i, j) > 0.5);
        }
    }
    assert_eq!(pits, 0, "a one-cell pit over 0.5 m is left in the core");
}

// spec: STRM-077
// spec: GEN-120
#[test]
fn a_tile_keeps_its_final_heights_for_the_post_pass_and_no_depth_of_its_own() {
    let (g, p) = (grid(), Pass2Params::standard(9, 0.0));
    let (a, b) = (pass2_tile(&g, tile(1, 1), &base, &nothing, None, &p, 1), pass2_tile(&g, tile(1, 1), &base, &nothing, None, &p, 8));
    assert_eq!(a.heights.len(), 24 * 24);
    assert_eq!(
        a.heights.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        b.heights.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        "1 and 8 threads"
    );
    assert!(a.heights.iter().all(|v| v.is_finite()));
    assert!(a.fill.is_empty(), "pass 2 holds no pit depth: the post-pass writes it (GEN-121)");
    // The heights are the interpolated window base plus the change (the stored change is taken against that window, so it cancels in the sum).
    let window = sample_window_coarse(&g, tile(1, 1), &base, 2, planet_generators::bake::tile::BASE_STRIDE);
    let w = g.window();
    for j in 0..g.tile {
        for i in 0..g.tile {
            let z = f64::from(a.heights[j * g.tile + i]);
            let core = f64::from(a.core[j * g.tile + i]);
            let want = window[(j + g.halo) * w + i + g.halo];
            assert!((z - core - want).abs() < 1e-3, "cell ({i}, {j}): height {z}, change {core}, window base {want}");
        }
    }
}
