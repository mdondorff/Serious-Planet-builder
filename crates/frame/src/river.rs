//! The river coverage of a tile (item 15 PR 8, REND-059): for every cell centre of a window the signed distance to the nearest river edge, in cells, and its 8-bit code. The plan owns this (CON-03: what is water is
//! decided here, the renderer only reads the texture); the width is the one function of the burn (`trench_width_cells`, capped at 32 cells) with a one-cell minimum, so the drawn river is the burned trench.
//!
//! The code of a distance `d` (cells, negative inside the river): `128 + 32 d` clamped to 0..255, that is +-4 cells at a step of one thirty-second of a cell; the fragment interpolates the texture and draws
//! water where the interpolated code is at or under [`CODE_ZERO`], which makes the banks smooth lines instead of cell staircases. Distances are of the cell centres in the face's cell-index space (cell `i` has
//! its centre at `i`, as the channels' points do) and a function of the channels alone, so any window of the same channels gives the same texels (CON-30).

use planet_generators::bake::k7::{trench_width_cells, Channel, QUnit, WidthLaw};

/// The distance, in cells, from the edge up to which a code differs from the clamps.
pub const RANGE_CELLS: f64 = 4.0;
/// The code of the edge: water where the code is at or under it.
pub const CODE_ZERO: u8 = 128;
/// Codes a cell.
pub const CODE_PER_CELL: f64 = 32.0;

/// The width of a river in cells for a discharge `q` (m3/s) on cells of `cell_m` metres: the burn's trench width, at least one cell (GEN-165: `max(min(w, 32 cells), 1 cell)`).
pub fn drawn_width_cells(law: &WidthLaw, cell_m: f64, q: f64) -> f64 {
    trench_width_cells(law, cell_m, q).max(1.0)
}

/// The code of a signed distance in cells.
pub fn distance_code(d: f64) -> u8 {
    if d.is_nan() {
        return 255;
    }
    libm::round(f64::from(CODE_ZERO) + d * CODE_PER_CELL).clamp(0.0, 255.0) as u8
}

/// The signed distance, in cells, of each cell centre of the window `size` (cells wide, high) whose first cell is `origin` (face cell indices) to the edge of the nearest river: negative inside, zero on the
/// edge. A cell farther than [`RANGE_CELLS`] from every edge reads `f64::INFINITY` (its code is the far-outside one). The river is the channels' polylines with the width of `drawn_width_cells` at the
/// discharge interpolated along each segment (as the burn interpolates it); a channel without discharges is the one-cell river.
pub fn river_distance(channels: &[Channel], origin: (i64, i64), size: (usize, usize), law: &WidthLaw, cell_m: f64) -> Vec<f64> {
    river_distance_grid(channels, (origin.0 as f64, origin.1 as f64), 1.0, size, law, cell_m)
}

/// [`river_distance`] on a lattice that is not the cells: the texel `(x, y)` of the window lies at `origin + (x, y) * step` in face cell-index space (a terrain node whose samples are `step` cells apart, REND-059);
/// the distances are still in cells. With `step` 1 and an integer origin the texels are the cell centres and the result is that of [`river_distance`] bit for bit.
pub fn river_distance_grid(
    channels: &[Channel],
    origin: (f64, f64),
    step: f64,
    size: (usize, usize),
    law: &WidthLaw,
    cell_m: f64,
) -> Vec<f64> {
    assert!(step > 0.0 && step.is_finite(), "a positive sample step");
    let (w, h) = size;
    let mut out = vec![f64::INFINITY; w * h];
    let (ox, oy) = origin;
    for ch in channels {
        assert!(ch.points.len() == ch.levels.len() && !ch.points.is_empty(), "a level for every point");
        assert!(ch.q.is_empty() || ch.q.len() == ch.points.len(), "a discharge for every point, or none");
        assert!(ch.q.is_empty() || ch.q_unit == QUnit::Discharge, "the river width needs a discharge in m3/s, not an area");
        assert!(ch.q.iter().all(|q| q.is_finite()), "the discharges of a channel are finite");
        assert!(ch.points.iter().all(|p| p.0.is_finite() && p.1.is_finite()), "the points of a channel are finite");
        let q_of = |k: usize| if ch.q.is_empty() { 0.0 } else { ch.q[k] };
        let n = ch.points.len();
        // A single point is a segment of no length.
        let segments = if n == 1 { 1 } else { n - 1 };
        for k in 0..segments {
            let (p0, p1) = (ch.points[k], ch.points[(k + 1).min(n - 1)]);
            let (q0, q1) = (q_of(k), q_of((k + 1).min(n - 1)));
            let reach = drawn_width_cells(law, cell_m, q0).max(drawn_width_cells(law, cell_m, q1)) / 2.0 + RANGE_CELLS;
            // The texels (indices) a point within `reach` of the segment can have: one index of slack each way.
            let x_lo = (libm::floor((p0.0.min(p1.0) - reach - ox) / step) as i64).saturating_sub(1).max(0);
            let x_hi = (libm::ceil((p0.0.max(p1.0) + reach - ox) / step) as i64).saturating_add(1).min(w as i64 - 1);
            let y_lo = (libm::floor((p0.1.min(p1.1) - reach - oy) / step) as i64).saturating_sub(1).max(0);
            let y_hi = (libm::ceil((p0.1.max(p1.1) + reach - oy) / step) as i64).saturating_add(1).min(h as i64 - 1);
            let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
            let len2 = dx * dx + dy * dy;
            let half_at = |t: f64| drawn_width_cells(law, cell_m, q0 + (q1 - q0) * t) / 2.0;
            let (h0, h1) = (half_at(0.0), half_at(1.0));
            for y in y_lo..=y_hi {
                for x in x_lo..=x_hi {
                    let (px, py) = (ox + x as f64 * step, oy + y as f64 * step);
                    // The signed distance at the point of the segment with parameter t: its distance minus the half width there.
                    let sd_at = |t: f64| {
                        let (nx, ny) = (p0.0 + dx * t, p0.1 + dy * t);
                        libm::sqrt((px - nx) * (px - nx) + (py - ny) * (py - ny)) - half_at(t)
                    };
                    let sd = if h0 == h1 {
                        // A river of one width along the segment: the nearest point of the segment.
                        let t = if len2 > 0.0 { (((px - p0.0) * dx + (py - p0.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                        let (nx, ny) = (p0.0 + dx * t, p0.1 + dy * t);
                        libm::sqrt((px - nx) * (px - nx) + (py - ny) * (py - ny)) - h0
                    } else {
                        // A river that widens along the segment is the union of discs the burn cuts along it (its edge is the envelope of the discs, not the nearest point's width): the minimum over the
                        // segment of `distance - half width`, a convex function of t (the width is concave in t), found by a fixed number of ternary steps (deterministic, the same in every window).
                        let (mut lo, mut hi) = (0.0f64, 1.0f64);
                        for _ in 0..50 {
                            let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
                            if sd_at(m1) < sd_at(m2) {
                                hi = m2;
                            } else {
                                lo = m1;
                            }
                        }
                        sd_at(0.5 * (lo + hi)).min(sd_at(0.0)).min(sd_at(1.0))
                    };
                    let c = y as usize * w + x as usize;
                    if sd < out[c] {
                        out[c] = sd;
                    }
                }
            }
        }
    }
    // Beyond the range the distance is not kept (the same whatever window or channel set reached the cell).
    for d in &mut out {
        if *d > RANGE_CELLS {
            *d = f64::INFINITY;
        }
    }
    out
}

/// The 8-bit coverage codes of the window: [`distance_code`] of [`river_distance`] (255 where no river is within range).
pub fn river_coverage(channels: &[Channel], origin: (i64, i64), size: (usize, usize), law: &WidthLaw, cell_m: f64) -> Vec<u8> {
    river_distance(channels, origin, size, law, cell_m).into_iter().map(|d| if d.is_finite() { distance_code(d) } else { 255 }).collect()
}

/// The grid of the stored layer the channels live on: the face edge in L cells, the L tile edge in cells and the cell size in metres (the width law needs it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiverGrid {
    pub cells: u32,
    pub tile: u32,
    pub cell_m: f64,
}

/// Where the plan gets the channels of an L tile: the runtime's reader of the channel graph (STRM-085) in the app, a list in tests. An error names the tile.
pub trait RiverSource {
    fn grid(&self) -> RiverGrid;
    /// The channels that can touch the core of L tile `(ti, tj)` of `face`, in the face's cell-index space.
    fn tile_channels(&self, face: planet_core::Face, ti: u32, tj: u32) -> Result<std::sync::Arc<Vec<Channel>>, String>;
}

/// What the river plan needs.
pub struct RiverRequest<'a> {
    pub source: &'a dyn RiverSource,
    pub law: &'a WidthLaw,
    /// Cells a terrain node's edge (its samples are `res + 1` a side).
    pub res: u32,
    /// A node whose samples are farther apart than this many L cells carries no coverage (its rivers are narrower than its samples): the draw's limit, a parameter.
    pub max_step_cells: f64,
}

/// The river coverage of one terrain node: `size * size` codes (row-major, `size = res + 1`), one a sample of the node.
#[derive(Clone, Debug, PartialEq)]
pub struct RiverNode {
    /// Index into the terrain plan's nodes (the instance the river draws with).
    pub node: u32,
    pub size: u32,
    pub coverage: Vec<u8>,
}

/// The rivers of one frame (REND-059): only nodes with a river within range.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RiverPlan {
    pub nodes: Vec<RiverNode>,
}

/// Where the samples of terrain node `tile` (`res` cells an edge, `res + 1` samples) lie in the face's cell-index space on an L grid of `cells` cells an edge: the first sample's position and the distance between
/// samples, in cells. Exact (every factor is a power of two or an integer) while the face edge is at most 2^20 cells and the node level at most 28; it takes the bake's own face mapping (`TangentWarp`, the only production
/// one) for granted, as the L grid does. The node's face coordinate `s` (the bake's own mapping: cell `i` has its centre at `s = 2 (i + 1/2) / cells - 1`) is at the cell-index position `(s + 1) cells / 2 - 1/2`.
pub fn node_window(tile: planet_core::TileId, res: u32, cells: u32) -> ((f64, f64), f64) {
    let (s0, s1, t0, _) = tile.bounds();
    let half = f64::from(cells) / 2.0;
    let step = (s1 - s0) / f64::from(res) * half;
    (((s0 + 1.0) * half - 0.5, (t0 + 1.0) * half - 0.5), step)
}

/// The river coverage of the nodes `nodes` (index in the terrain plan and tile): a node whose samples are farther apart than the request's limit, or with no river within range of a sample, carries none. The
/// channels come from the L tiles the node's window meets widened by the widest trench and the code's range; a channel handed out by two tiles counts once. An error names the tile that failed.
pub fn plan_rivers(req: &RiverRequest<'_>, nodes: &[(u32, planet_core::TileId)]) -> Result<RiverPlan, String> {
    let grid = req.source.grid();
    if req.res == 0 || !(req.max_step_cells.is_finite() && req.max_step_cells > 0.0) {
        return Err(format!(
            "the river plan needs a node of at least one cell and a finite positive step limit (res {}, limit {})",
            req.res, req.max_step_cells
        ));
    }
    if grid.tile == 0 || grid.cells == 0 || !grid.cells.is_multiple_of(grid.tile) {
        return Err(format!("the river grid of {} cells a face edge is not a whole number of tiles of {} cells", grid.cells, grid.tile));
    }
    let per_edge = i64::from(grid.cells / grid.tile);
    let reach = crate::river::reach_cells();
    let size = req.res as usize + 1;
    let mut out = Vec::new();
    for &(index, tile) in nodes {
        let (origin, step) = node_window(tile, req.res, grid.cells);
        if step > req.max_step_cells {
            continue;
        }
        let span = f64::from(req.res) * step;
        // The tile of a position is its floor; a cell centre `i` owns `[i - 1/2, i + 1/2)`, so near a tile's upper edge this can pick the tile below: harmless because the reader widens a tile by the widest trench.
        let range = |o: f64| {
            let lo = (libm::floor((o - reach) / f64::from(grid.tile)) as i64).clamp(0, per_edge - 1);
            let hi = (libm::floor((o + span + reach) / f64::from(grid.tile)) as i64).clamp(0, per_edge - 1);
            (lo, hi)
        };
        let ((ti0, ti1), (tj0, tj1)) = (range(origin.0), range(origin.1));
        let mut seen = std::collections::HashSet::new();
        let mut channels: Vec<Channel> = Vec::new();
        for tj in tj0..=tj1 {
            for ti in ti0..=ti1 {
                for c in req.source.tile_channels(tile.face(), ti as u32, tj as u32)?.iter() {
                    // The whole channel is the key: two pieces that share a start and a length are still two rivers.
                    let key: Vec<u64> = c
                        .points
                        .iter()
                        .flat_map(|p| [p.0.to_bits(), p.1.to_bits()])
                        .chain(c.q.iter().map(|q| q.to_bits()))
                        .chain(c.levels.iter().map(|l| l.to_bits()))
                        .collect();
                    if seen.insert(key) {
                        channels.push(c.clone());
                    }
                }
            }
        }
        if channels.is_empty() {
            continue;
        }
        let coverage: Vec<u8> = river_distance_grid(&channels, origin, step, (size, size), req.law, grid.cell_m)
            .into_iter()
            .map(|d| if d.is_finite() { distance_code(d) } else { 255 })
            .collect();
        if coverage.iter().all(|&c| c == 255) {
            continue;
        }
        out.push(RiverNode { node: index, size: size as u32, coverage });
    }
    Ok(RiverPlan { nodes: out })
}

/// How far from a node's window a channel can matter: the widest half trench and the code's range.
pub fn reach_cells() -> f64 {
    planet_generators::bake::k7::DISC_RADIUS_MAX as f64 + RANGE_CELLS
}
