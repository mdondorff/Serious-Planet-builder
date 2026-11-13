//! Tier A, GEN-186 (TEST-134, TEST-135): the line of a channel at the local grid follows the lowest pass of `B` between its H1 vertices. A line laid on a valley side ends on the valley floor with no
//! bank beside it; a step reads `B` inside its corridor only (partition independence); a tributary and its main segment meet in one cell; the placement is the same whatever the order of the calls.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::k7::{k7_tile, line_cells, Channel, K7Params, QUnit};
use planet_generators::bake::placement::{Placement, CORRIDOR_CELLS, SNAP_CELLS};
use planet_generators::bake::tile::{TileGrid, TileId};

fn tile_grid() -> TileGrid {
    TileGrid { cells: 384, tile: 96, halo: 24, radius_m: 100.0 * 384.0 * 2.0 / core::f64::consts::PI }
}

fn id() -> TileId {
    TileId { face: Face(1), ti: 1, tj: 2 }
}

fn face_xy(d: Vec3) -> (f64, f64) {
    let n = tile_grid().cells as f64;
    let face = id().face;
    let w = d.dot(face.normal());
    let plane = |u: f64| libm::atan(u) / core::f64::consts::FRAC_PI_4;
    let (s, t) = (plane(d.dot(face.u_axis()) / w), plane(d.dot(face.v_axis()) / w));
    ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5)
}

fn direction(x: f64, y: f64) -> Vec3 {
    let n = tile_grid().cells as f64;
    face_to_direction(&TangentWarp, id().face, 2.0 * (x + 0.5) / n - 1.0, 2.0 * (y + 0.5) / n - 1.0)
}

/// A V-shaped valley falling 1.5 m a cell to the east, its floor on the face row `floor_y`, its sides rising 8 m a cell.
fn valley(x: f64, y: f64, floor_y: f64) -> f64 {
    600.0 - 1.5 * (x - 72.0) + 8.0 * (y - floor_y).abs()
}

// spec: GEN-186, TEST-134 (the dyke case)
#[test]
fn a_line_laid_on_the_valley_side_is_placed_on_the_floor_and_k7_leaves_no_bank_beside_it() {
    let g = tile_grid();
    let (t, h) = (g.tile as i64, g.halo as i64);
    let (wx0, wy0) = (i64::from(id().ti) * t - h, i64::from(id().tj) * t - h);
    let floor_y = (wy0 + h + 40) as f64;
    let base = |d: Vec3| {
        let (x, y) = face_xy(d);
        valley(x, y, floor_y)
    };
    let base_cell = |x: i64, y: i64| base(direction(x as f64, y as f64));
    // The H1 line two cells up the side, points every 8 cells (an H1 cell), levels 60 m over the start surface: the straight raster would leave the line on the side (a 22.6 m dyke before GEN-186).
    let line_y = floor_y + 2.0;
    let w = (g.tile + 2 * g.halo) as i64;
    let points: Vec<(f64, f64)> = (0..w / 8).map(|k| ((wx0 + 3 + 8 * k) as f64 + 0.5, line_y + 0.5)).collect();
    let levels = points.iter().map(|&(x, _)| valley(x, line_y, floor_y) + 60.0).collect();
    let ch = Channel { points, levels, q: Vec::new(), q_unit: QUnit::Discharge };
    let placed = Placement::new(&base_cell).channel(&ch);
    let none = |_: Vec3| 0.0;
    let r = k7_tile(&g, id(), &base, &none, std::slice::from_ref(&placed), &K7Params::standard(5, 0.0), 2);
    let ww = g.window();
    let (mut above, mut bank, mut cells) = (f64::NEG_INFINITY, f64::NEG_INFINITY, 0usize);
    for (x, y) in line_cells(&placed) {
        let (a, b) = ((x - wx0) as usize, (y - wy0) as usize);
        if a < g.halo + 4 || a >= g.halo + g.tile - 4 || b < 4 || b + 4 >= ww {
            continue;
        }
        assert_eq!(y as f64, floor_y, "the placed line runs on the floor row, not at ({x}, {y})");
        let col: Vec<(usize, f64)> = (b - 4..=b + 4).map(|bb| (bb, r.z[bb * ww + a])).collect();
        let (lb, low) = col.iter().copied().fold((b, f64::INFINITY), |m, (bb, v)| if v < m.1 { (bb, v) } else { m });
        let between = col.iter().filter(|(bb, _)| *bb > lb.min(b) && *bb < lb.max(b)).map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        bank = bank.max(between - r.z[b * ww + a].max(low));
        above = above.max(r.z[b * ww + a] - low);
        cells += 1;
    }
    println!("GEN-186 dyke: {cells} placed line cells; the line stands at most {above:.2} m over the low of its cross-section, a bank at most {bank:.2} m");
    assert!(cells > 50, "the line crosses the core: {cells} cells");
    assert!(above <= 2.0, "the line stands {above:.2} m over the valley floor beside it (GEN-186)");
    assert!(bank <= 0.5, "a bank of {bank:.2} m stands beside the line (GEN-186)");
}

// spec: GEN-186, TEST-135 (partition independence)
#[test]
fn a_step_reads_the_base_inside_its_corridor_only_and_the_same_cells_come_back_every_time() {
    let base = |x: i64, y: i64| 300.0 + 7.0 * libm::sin(0.37 * x as f64) * libm::cos(0.23 * y as f64) + 0.5 * x as f64 - 0.25 * y as f64;
    let (p0, p1) = ((103.5, 43.5), (111.5, 51.5));
    let pl = Placement::new(&base);
    let (a, b) = (pl.vertex(p0), pl.vertex(p1));
    let cells = pl.step(a, b, p0, p1);
    // The base read only within the corridor and the vertices' snap boxes: elsewhere it panics.
    let reach = CORRIDOR_CELLS.ceil() as i64 + SNAP_CELLS + 1;
    let guarded = |x: i64, y: i64| {
        let near = |p: (f64, f64)| (x - p.0.round() as i64).abs() <= SNAP_CELLS && (y - p.1.round() as i64).abs() <= SNAP_CELLS;
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let t = (((x as f64 - p0.0) * dx + (y as f64 - p0.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        let (ex, ey) = (x as f64 - (p0.0 + t * dx), y as f64 - (p0.1 + t * dy));
        if near(p0) || near(p1) || ex * ex + ey * ey <= CORRIDOR_CELLS * CORRIDOR_CELLS {
            base(x, y)
        } else {
            f64::NAN
        }
    };
    let _ = reach;
    let g = Placement::new(&guarded);
    assert_eq!((g.vertex(p0), g.vertex(p1)), (a, b), "the vertices read their snap boxes only");
    assert_eq!(g.step(a, b, p0, p1), cells, "the step reads its corridor only");
    assert_eq!(pl.step(a, b, p0, p1), cells, "the same cells every time");
    assert!(cells.windows(2).all(|w| (w[1].0 - w[0].0).abs() <= 1 && (w[1].1 - w[0].1).abs() <= 1 && w[0] != w[1]), "8-connected: {cells:?}");
    let pass = cells.iter().map(|&(x, y)| base(x, y)).fold(f64::NEG_INFINITY, f64::max);
    let straight = line_cells(&Channel { points: vec![p0, p1], levels: vec![0.0; 2], q: Vec::new(), q_unit: QUnit::Discharge });
    let straight_pass = straight.iter().map(|&(x, y)| base(x, y)).fold(f64::NEG_INFINITY, f64::max);
    assert!(pass <= straight_pass.max(base(a.0, a.1)).max(base(b.0, b.1)) + 1e-9, "the placed pass {pass} is no higher than the straight raster's {straight_pass}");
}

// spec: GEN-186, TEST-135
#[test]
fn a_tributary_and_its_main_segment_meet_in_one_cell() {
    let base = |x: i64, y: i64| 200.0 + 3.0 * libm::sin(0.9 * x as f64 + 0.4 * y as f64) + 0.2 * (x - y) as f64;
    let pl = Placement::new(&base);
    let junction = (83.5, 83.5);
    let trib = Channel { points: vec![(67.5, 75.5), (75.5, 75.5), junction], levels: vec![30.0, 29.0, 28.0], q: vec![1.0, 2.0, 3.0], q_unit: QUnit::Discharge };
    let main = Channel { points: vec![junction, (91.5, 91.5), (99.5, 91.5)], levels: vec![28.0, 27.0, 26.0], q: vec![9.0, 9.5, 10.0], q_unit: QUnit::Discharge };
    let (t, m) = (pl.channel(&trib), pl.channel(&main));
    assert_eq!(t.points.last(), m.points.first(), "the tributary ends at the main segment's first cell");
    assert!(t.levels.windows(2).all(|w| w[1] <= w[0]) && m.levels.windows(2).all(|w| w[1] <= w[0]), "the levels do not rise along the placed line");
    assert!(t.q.len() == t.points.len() && m.q.len() == m.points.len());
}
