//! The river coverage of a tile (item 15 PR 8, REND-059): for every cell centre of a window the signed distance to the nearest river edge, in cells, and its 8-bit code. The plan owns this (CON-03: what is water is
//! decided here, the renderer only reads the texture); the width is the one function of the burn (`trench_width_cells`, capped at 32 cells) with a one-cell minimum, so the drawn river is the burned trench.
//!
//! The code of a distance `d` (cells, negative inside the river): `128 + 32 d` clamped to 0..255, that is +-4 cells at a step of one thirty-second of a cell; the fragment interpolates the texture and draws
//! water where the interpolated code is at or under [`CODE_ZERO`], which makes the banks smooth lines instead of cell staircases. Distances are of the cell centres in the face's cell-index space (cell `i` has
//! its centre at `i`, as the channels' points do) and a function of the channels alone, so any window of the same channels gives the same texels (CON-30).

use planet_generators::bake::k7::{trench_width_cells, Channel, WidthLaw};

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
    libm::round(f64::from(CODE_ZERO) + d * CODE_PER_CELL).clamp(0.0, 255.0) as u8
}

/// The signed distance, in cells, of each cell centre of the window `size` (cells wide, high) whose first cell is `origin` (face cell indices) to the edge of the nearest river: negative inside, zero on the
/// edge. A cell farther than [`RANGE_CELLS`] from every edge reads `f64::INFINITY` (its code is the far-outside one). The river is the channels' polylines with the width of `drawn_width_cells` at the
/// discharge interpolated along each segment (as the burn interpolates it); a channel without discharges is the one-cell river.
pub fn river_distance(channels: &[Channel], origin: (i64, i64), size: (usize, usize), law: &WidthLaw, cell_m: f64) -> Vec<f64> {
    let (w, h) = size;
    let mut out = vec![f64::INFINITY; w * h];
    let (i0, j0) = origin;
    for ch in channels {
        assert!(ch.points.len() == ch.levels.len() && !ch.points.is_empty(), "a level for every point");
        let q_of = |k: usize| if ch.q.is_empty() { 0.0 } else { ch.q[k] };
        let n = ch.points.len();
        // A single point is a segment of no length.
        let segments = if n == 1 { 1 } else { n - 1 };
        for k in 0..segments {
            let (p0, p1) = (ch.points[k], ch.points[(k + 1).min(n - 1)]);
            let (q0, q1) = (q_of(k), q_of((k + 1).min(n - 1)));
            let reach = drawn_width_cells(law, cell_m, q0).max(drawn_width_cells(law, cell_m, q1)) / 2.0 + RANGE_CELLS;
            let x_lo = (libm::floor(p0.0.min(p1.0) - reach) as i64).max(i0);
            let x_hi = (libm::ceil(p0.0.max(p1.0) + reach) as i64).min(i0 + w as i64 - 1);
            let y_lo = (libm::floor(p0.1.min(p1.1) - reach) as i64).max(j0);
            let y_hi = (libm::ceil(p0.1.max(p1.1) + reach) as i64).min(j0 + h as i64 - 1);
            let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
            let len2 = dx * dx + dy * dy;
            for y in y_lo..=y_hi {
                for x in x_lo..=x_hi {
                    let (px, py) = (x as f64, y as f64);
                    let t = if len2 > 0.0 { (((px - p0.0) * dx + (py - p0.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                    let (nx, ny) = (p0.0 + dx * t, p0.1 + dy * t);
                    let dist = libm::sqrt((px - nx) * (px - nx) + (py - ny) * (py - ny));
                    let half = drawn_width_cells(law, cell_m, q0 + (q1 - q0) * t) / 2.0;
                    let sd = dist - half;
                    let c = (y - j0) as usize * w + (x - i0) as usize;
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
