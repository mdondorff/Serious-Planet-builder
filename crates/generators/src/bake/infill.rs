//! The basin infill of the base surface before pass 1 (GEN-137, ADR 0109): depressions of the surface are filled up to their spill level, small or shallow basins completely and
//! larger, deeper ones up to a residual depth. What is left is a lake basin (the fill-depth channel of GEN-120); the thresholds are Earth-preset parameters (ADR 0108).
//!
//! A basin is a connected set (8 neighbours) of cells that stand more than [`PIT_MIN_M`] under the flood of the surface (the flood of GEN-100, `epsilon` over flats). A basin whose area is
//! under `min_area_km2` or whose deepest cell is under `min_depth_m` is filled to the flood; any other basin is raised to the flood minus `residual_m` where it is deeper than that. A cell is
//! never raised above its spill level (the flood) and never lowered.

use super::flood::priority_flood;
use super::Grid;

/// A cell standing less than this under the flood is not in a basin (the minimum of the fill-depth channel, GEN-120: the epsilon slope of the flood over flats stays below it).
pub const PIT_MIN_M: f64 = 0.5;

/// The thresholds of the infill (start values in the design notes of `pass1-deposition`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Infill {
    /// A basin smaller than this (square kilometres) is filled completely.
    pub min_area_km2: f64,
    /// A basin shallower than this (metres, at its deepest cell) is filled completely.
    pub min_depth_m: f64,
    /// A larger, deeper basin keeps at most this depth (metres); at least `min_depth_m`, so that a second infill leaves it as it is.
    pub residual_m: f64,
}

/// What an infill did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InfillReport {
    /// Basins found, and the cells in them before and after.
    pub basins: usize,
    pub cells_before: usize,
    pub cells_after: usize,
    /// The basins left (lake basins) and their deepest depth in metres.
    pub left: usize,
    pub deepest_left_m: f64,
}

/// Fills the basins of `z` in place. `outlet` cells keep their heights. Refused: thresholds that are negative or not finite, a residual under the minimum depth, arrays of another length.
pub fn infill(grid: &Grid, z: &mut [f64], outlet: &[bool], p: &Infill, epsilon: f64) -> InfillReport {
    let n = grid.len();
    assert!(z.len() == n && outlet.len() == n, "one height and one outlet flag per cell");
    assert!(
        [p.min_area_km2, p.min_depth_m, p.residual_m].iter().all(|v| v.is_finite() && *v >= 0.0),
        "the infill thresholds are finite and not negative: {p:?}"
    );
    assert!(
        p.residual_m >= p.min_depth_m,
        "the residual depth {} m is under the minimum depth {} m: a second infill would fill the residual",
        p.residual_m,
        p.min_depth_m
    );
    super::assert_finite("the heights", z);
    let filled = priority_flood(grid, z, outlet, epsilon);
    let in_basin = |c: usize| filled[c] - z[c] > PIT_MIN_M;
    // The basins by a depth-first walk in cell order: label, area and deepest depth.
    let mut label = vec![u32::MAX; n];
    let mut basins: Vec<(usize, f64)> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for s in 0..n {
        if label[s] != u32::MAX || !in_basin(s) {
            continue;
        }
        let id = basins.len() as u32;
        let (mut cells, mut deepest) = (0usize, 0.0f64);
        label[s] = id;
        stack.push(s);
        while let Some(c) = stack.pop() {
            cells += 1;
            deepest = deepest.max(filled[c] - z[c]);
            for d in 0..8 {
                if let Some(q) = grid.neighbour(c, d) {
                    if label[q] == u32::MAX && in_basin(q) {
                        label[q] = id;
                        stack.push(q);
                    }
                }
            }
        }
        basins.push((cells, deepest));
    }
    let cell_km2 = grid.dx() * grid.dx() * 1e-6;
    let full: Vec<bool> =
        basins.iter().map(|&(cells, deepest)| cells as f64 * cell_km2 < p.min_area_km2 || deepest < p.min_depth_m).collect();
    let mut report = InfillReport {
        basins: basins.len(),
        cells_before: label.iter().filter(|l| **l != u32::MAX).count(),
        cells_after: 0,
        left: full.iter().filter(|f| !**f).count(),
        deepest_left_m: 0.0,
    };
    for c in 0..n {
        let l = label[c];
        if l == u32::MAX {
            continue;
        }
        let target = if full[l as usize] { filled[c] } else { filled[c] - p.residual_m };
        if target > z[c] {
            z[c] = target;
        }
        if filled[c] - z[c] > PIT_MIN_M {
            report.cells_after += 1;
            report.deepest_left_m = report.deepest_left_m.max(filled[c] - z[c]);
        }
    }
    report
}
