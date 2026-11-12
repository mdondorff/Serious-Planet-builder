//! The round-0 breach of the closed basins (GEN-174, SPIKE-108 follow-up; Lindsay's constrained least-cost breaching): the start surface holds far more large closed basins than real
//! terrain (22 to 32 % of the land against 3.3 % in the DEM), and the fill turns each into a flat whose drainage the carve then incises along the flat's routes. One pass drains
//! each basin by the cheapest path to a lower cell, within a depth and a length that are formulas of the profile; a basin that cannot be drained stays filled (a lake).
//!
//! - **Basin.** A flat of the canonical filled surface (`flood::lifted_flats`) whose deepest lift is at least `min_lift_m`. Its floor is its lowest cell, by (height, id).
//! - **Path.** From the floor, over the 8 neighbours, by least cost: entering a cell costs `max(0, z - floor)` (the depth the breach cuts there); a cell more than `max_depth_m` above the floor is not entered;
//!   the path has at most `halo - 1 - diameter` cells (diameter: Chebyshev, in cells), so basin and path lie inside the halo of every tile of its cells (the bound of GEN-173); a basin that touches the
//!   border of the grid, or has no such length, stays a lake. The path ends at the first cell outside the basin that stands below the floor (or an outlet at or below it). Order: (cost, cells, id).
//! - **Cut.** Every path cell above the floor is lowered to it; the exit is not touched.
//! - **Order.** Basins in order of (floor height, floor id), each on the surface the earlier ones left: a function of the cells only, so the same bits on any thread count and any partition.
//!
//! Planar grids only (like the flat grade); a cube grid is left as it is.

use super::flood::{lifted_flats, priority_flood_ungraded};
use super::Grid;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The parameters of the pass: the profile's halo in cells (the bound of the path), the deepest cut of one cell in metres (the profile's breach limit) and the shallowest basin that is breached.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreBreach {
    /// Passes: each pass drains the basins that the earlier one left (a nested basin whose ridge the first path did not cut); a pass that drains nothing ends the loop. 1 is the architect's single pass.
    pub passes: u32,
    pub halo: usize,
    pub max_depth_m: f64,
    pub min_lift_m: f64,
}

impl PreBreach {
    /// The production profile (GEN-174): one pass, a path inside the halo of 128 cells (`halo - 1 - diameter` cells), a cut of at most the profile's breach limit (500 m) a cell, basins lifted by at least 0.5 m (H1's pit limit).
    pub const PROFILE: PreBreach = PreBreach { passes: 1, halo: 128, max_depth_m: 500.0, min_lift_m: 0.5 };
}

/// What the pass did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BreachReport {
    /// Basins considered (flats lifted by at least `min_lift_m`).
    pub basins: usize,
    pub drained: usize,
    /// Kept: touching the border of the grid, or with no path length left inside the halo.
    pub kept_bound: usize,
    /// Kept: no path within the depth and the length.
    pub kept_lake: usize,
    pub cells_cut: usize,
    /// The volume cut, in metres of height summed over the cells (times the cell area for m3).
    pub cut_m_cells: f64,
    pub cells_in_kept_lakes: usize,
}

fn key(v: f64) -> u64 {
    let b = v.to_bits();
    if b & (1 << 63) != 0 {
        !b
    } else {
        b | (1 << 63)
    }
}

/// Drains the closed basins of `z` (see the module), `p.passes` times at most. `outlet` are the cells that keep their height; `epsilon` the flood's.
pub fn breach_basins(grid: &Grid, z: &mut [f64], outlet: &[bool], epsilon: f64, p: &PreBreach) -> BreachReport {
    assert!(p.passes >= 1, "a breach has at least one pass");
    let mut total = breach_pass(grid, z, outlet, epsilon, p);
    for _ in 1..p.passes {
        if total.drained == 0 {
            break;
        }
        let next = breach_pass(grid, z, outlet, epsilon, p);
        if next.drained == 0 {
            break;
        }
        total.drained += next.drained;
        total.cells_cut += next.cells_cut;
        total.cut_m_cells += next.cut_m_cells;
        total.basins = next.basins;
        total.kept_bound = next.kept_bound;
        total.kept_lake = next.kept_lake;
        total.cells_in_kept_lakes = next.cells_in_kept_lakes;
    }
    total
}

fn breach_pass(grid: &Grid, z: &mut [f64], outlet: &[bool], epsilon: f64, p: &PreBreach) -> BreachReport {
    let mut rep = BreachReport::default();
    assert!(
        p.max_depth_m >= 0.0 && p.max_depth_m.is_finite() && p.min_lift_m >= 0.0,
        "the breach depth and the basin depth are finite and not negative"
    );
    if grid.is_cube() || p.halo < 2 {
        return rep;
    }
    let filled = priority_flood_ungraded(grid, z, outlet, epsilon, true);
    let mut basins: Vec<(f64, u32, usize)> = Vec::new();
    let flats = lifted_flats(grid, z, outlet, epsilon, &filled);
    drop(filled);
    for (i, f) in flats.iter().enumerate() {
        if f.max_lift_m < p.min_lift_m {
            continue;
        }
        let floor =
            f.cells.iter().map(|&c| (z[c as usize], c)).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).expect("a flat has cells");
        basins.push((floor.0, floor.1, i));
    }
    basins.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    rep.basins = basins.len();
    let n = grid.len();
    let mut stamp = vec![u32::MAX; n];
    let mut best = vec![f64::INFINITY; n];
    let mut cells = vec![0u32; n];
    let mut parent = vec![u32::MAX; n];
    let mut touched: Vec<u32> = Vec::new();
    for &(zp, floor, fi) in &basins {
        let flat = &flats[fi];
        let room = p.halo as i64 - 1 - flat.diameter as i64;
        if flat.touches_border || room < 1 {
            rep.kept_bound += 1;
            continue;
        }
        let room = room as u32;
        for &c in &flat.cells {
            stamp[c as usize] = fi as u32;
        }
        let is_exit = |q: usize, z: &[f64]| stamp[q] != fi as u32 && (z[q] < zp || (outlet[q] && z[q] <= zp));
        let mut heap: BinaryHeap<Reverse<(u64, u32, u32)>> = BinaryHeap::new();
        best[floor as usize] = 0.0;
        cells[floor as usize] = 0;
        touched.push(floor);
        heap.push(Reverse((key(0.0), 0, floor)));
        let mut found: Option<usize> = None;
        while let Some(Reverse((k, steps, c))) = heap.pop() {
            let cu = c as usize;
            if key(best[cu]) != k || cells[cu] != steps {
                continue;
            }
            if is_exit(cu, z) {
                found = Some(cu);
                break;
            }
            if steps + 1 > room {
                continue;
            }
            for d in 0..8 {
                let Some(q) = grid.neighbour(cu, d) else { continue };
                let exit = is_exit(q, z);
                let up = z[q] - zp;
                if !exit && up > p.max_depth_m {
                    continue;
                }
                let (nc, ns) = (best[cu] + if exit { 0.0 } else { up.max(0.0) }, steps + 1);
                let old = if best[q].is_infinite() { (f64::INFINITY, u32::MAX) } else { (best[q], cells[q]) };
                if (nc, ns) < old {
                    if best[q].is_infinite() {
                        touched.push(q as u32);
                    }
                    best[q] = nc;
                    cells[q] = ns;
                    parent[q] = c;
                    heap.push(Reverse((key(nc), ns, q as u32)));
                }
            }
        }
        match found {
            Some(exit) => {
                let mut c = parent[exit];
                while c != u32::MAX {
                    let cu = c as usize;
                    if z[cu] > zp {
                        rep.cut_m_cells += z[cu] - zp;
                        rep.cells_cut += 1;
                        z[cu] = zp;
                    }
                    c = if cu == floor as usize { u32::MAX } else { parent[cu] };
                }
                rep.drained += 1;
            }
            None => {
                rep.kept_lake += 1;
                rep.cells_in_kept_lakes += flat.cells.len();
            }
        }
        for &t in &touched {
            best[t as usize] = f64::INFINITY;
            parent[t as usize] = u32::MAX;
        }
        touched.clear();
        for &c in &flat.cells {
            stamp[c as usize] = u32::MAX;
        }
    }
    rep
}
