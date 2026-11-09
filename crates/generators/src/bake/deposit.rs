//! The deposition sweep (GEN-101; SPIKE-64's kernel, transport-limited): one pass from upstream to downstream over a routing.
//!
//! The load of a cell is its donors' plus its own eroded column (`original - current` where it is positive, times the cell area; land only). The load first fills the cell's
//! depression up to the filled level, at most to the receiver's carved height plus half the filled drop (a bowl's route does not become a pillar of the bowl's fill), then, where it exceeds the capacity `kc sqrt(A) S` (cubic metres per step, `A` the drainage area, `S` the filled slope to the receiver),
//! `zeta` of the excess settles, raising the cell at most to the filled level plus half the drop to the receiver, so that the receiver stays lower. The rest moves on; what reaches
//! an outlet (the sea or the grid's border) leaves. A cell never ends above the lowest final height of its donors (so a settled cell stays below the cells that drain into it); what
//! the ceiling holds back moves on. Mass is kept: what was eroded is deposited or leaves. The eroded column is counted on land only (`z` above the sea level): a carve that
//! lowers land below the sea would lose that volume here, as the carve of this crate does not.
//!
//! On a cube grid (GEN-150) the cell area and the distance to the receiver are the sphere's, per cell.

use super::flood::priority_flood;
use super::route::{route, Routing, NONE};
use super::Grid;

/// The capacity coefficient `kc`: one value, or one per cell (GEN-097's classes blended).
#[derive(Clone, Copy, Debug)]
pub enum Kc<'a> {
    Uniform(f64),
    PerCell(&'a [f64]),
}

/// The parameters of the sweep.
#[derive(Clone, Copy, Debug)]
pub struct DepositParams<'a> {
    pub kc: Kc<'a>,
    /// The share of the load over the capacity that settles.
    pub zeta: f64,
    /// Land is above this height; the sea's cells take no eroded column.
    pub sea_level: f64,
}

/// What a sweep did, in cubic metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepositReport {
    pub eroded: f64,
    pub deposited: f64,
    pub exported: f64,
}

/// Deposits on `z` in place along `routing`, whose receivers and order were made on `filled` (the flooded surface of `z`'s carved ground). `original` is the ground before the carve:
/// the eroded column of a cell is `original - z` where positive. `area` is the routing's drainage area. Refused: arrays of another length, a `zeta` outside [0, 1], a `kc` that is
/// negative or not finite, a height that is not finite.
pub fn deposit(grid: &Grid, z: &mut [f64], original: &[f64], filled: &[f64], routing: &Routing, params: &DepositParams) -> DepositReport {
    deposit_core(grid, z, original, filled, routing, params, None)
}

/// [`deposit`] that also writes, for the cell where a route leaves (a cell with no receiver), the load that leaves there in cubic metres (`exports`, one entry per cell, set to 0 first): the per-catchment export of G-SED (GEN-152).
pub fn deposit_exports(
    grid: &Grid,
    z: &mut [f64],
    original: &[f64],
    filled: &[f64],
    routing: &Routing,
    params: &DepositParams,
    exports: &mut [f64],
) -> DepositReport {
    assert_eq!(exports.len(), grid.len(), "one export per cell");
    exports.fill(0.0);
    deposit_core(grid, z, original, filled, routing, params, Some(exports))
}

fn deposit_core(
    grid: &Grid,
    z: &mut [f64],
    original: &[f64],
    filled: &[f64],
    routing: &Routing,
    params: &DepositParams,
    mut exports: Option<&mut [f64]>,
) -> DepositReport {
    let n = grid.len();
    assert!(z.len() == n && original.len() == n && filled.len() == n && routing.receiver.len() == n, "one value per cell");
    assert!((0.0..=1.0).contains(&params.zeta), "zeta is a share in [0, 1], not {}", params.zeta);
    super::assert_finite("the heights", z);
    super::assert_finite("the original heights", original);
    if let Kc::PerCell(k) = params.kc {
        assert_eq!(k.len(), n, "one kc per cell");
    }
    // The cell area and the distance to the receiver: the sphere's metric on a cube grid (GEN-150), the uniform `dx` on a plane grid.
    let cube = grid.is_cube();
    let area_of = |p: usize| if cube { grid.cell_area(p) } else { grid.dx() * grid.dx() };
    super::assert_finite("the filled heights", filled);
    assert!(routing.area.len() == n && routing.order.len() == n, "one drainage area and one order entry per cell");
    assert!(routing.area.iter().all(|a| a.is_finite() && *a >= 0.0), "the drainage areas are finite and not negative");
    let mut carried = vec![0.0f64; n];
    let mut ceiling = vec![f64::INFINITY; n];
    let mut report = DepositReport { eroded: 0.0, deposited: 0.0, exported: 0.0 };
    for &p in routing.order.iter().rev() {
        let p = p as usize;
        let land = z[p] > params.sea_level;
        let ca = area_of(p);
        let eroded = if land { (original[p] - z[p]).max(0.0) * ca } else { 0.0 };
        report.eroded += eroded;
        let mut load = carried[p] + eroded;
        let r = routing.receiver[p];
        if r == NONE {
            report.exported += load;
            if let Some(e) = exports.as_deref_mut() {
                e[p] += load;
            }
            continue;
        }
        let r = r as usize;
        let (zp, fp, fr) = (z[p], filled[p], filled[r]);
        // The depression first, but not above what the receiver's carved ground (and half the drop of the filled surface) lets the cell stand: the fill of a closed bowl walks one route, and a
        // fill that took the whole depth would leave that route a pillar a cell wide in a bowl whose other cells keep their carved height (SPIKE-98 diagnosis, `change/deposition-pits`).
        let pit_cap = ((z[r] - zp).max(0.0) + 0.5 * (fp - fr).max(0.0)) * ca;
        let pit = ((fp - zp).max(0.0) * ca).min(load).min(pit_cap);
        let mut rise = pit / ca;
        load -= pit;
        // Then the excess over the capacity, up to half the drop to the receiver above the filled level.
        let dist = if cube {
            let d = (0..8).find(|&d| grid.neighbour(p, d) == Some(r)).expect("a receiver is a neighbour");
            grid.distance_at(p, d)
        } else {
            let (di, dj) = ((p % grid.w()).abs_diff(r % grid.w()), (p / grid.w()).abs_diff(r / grid.w()));
            if di == 0 || dj == 0 {
                grid.dx()
            } else {
                grid.dx() * core::f64::consts::SQRT_2
            }
        };
        let slope = ((fp - fr) / dist).max(0.0);
        let kc = match params.kc {
            Kc::Uniform(k) => k,
            Kc::PerCell(k) => k[p],
        };
        assert!(kc.is_finite() && kc >= 0.0, "kc is finite and not negative, not {kc} at cell {p}");
        let capacity = kc * libm::sqrt(routing.area[p]) * slope;
        if load > capacity {
            let room = (0.5 * (fp - fr).max(0.0) + (fp - zp - rise).max(0.0)) * ca;
            let settled = (params.zeta * (load - capacity)).min(room);
            rise += settled / ca;
            load -= settled;
        }
        // Not above the lowest donor's final height: what does not fit moves on.
        let allowed = (ceiling[p] - zp).max(0.0);
        if rise > allowed {
            load += (rise - allowed) * ca;
            rise = allowed;
        }
        z[p] = zp + rise;
        report.deposited += rise * ca;
        carried[r] += load;
        ceiling[r] = ceiling[r].min(z[p]);
    }
    report
}

/// A sweep on its own route: floods the carved ground `z`, routes it (`seed` the jitter's stream, `epsilon` the flood's) and deposits. Outlets are the grid's border and the sea.
pub fn deposit_on_own_route(
    grid: &Grid,
    z: &mut [f64],
    original: &[f64],
    epsilon: f64,
    seed: u64,
    params: &DepositParams,
) -> DepositReport {
    let outlet = grid.outlets(z, params.sea_level);
    let filled = priority_flood(grid, z, &outlet, epsilon);
    let routing = route(grid, z, &outlet, epsilon, seed, 0.0);
    deposit(grid, z, original, &filled, &routing, params)
}

/// Spreads the one-cell pits of `z` over their neighbours, keeping the volume (GEN-101): a land cell whose 8 neighbours are inside the grid's border (the border cells only receive and are never lowered), that is lower than all its 8 neighbours by more than
/// `min_depth_m` (and has no neighbour at or below the sea level) rises by 0.8 of its depth below the lowest neighbour while its 8 neighbours each fall by an eighth of that rise, so the
/// heights' sum is unchanged. The pits are taken in cell order and the pass repeats (the neighbours' fall can make a smaller pit) up to `max_passes` times, until no pit is deeper than
/// `min_depth_m`. Returns the passes run. Deterministic: serial, fixed order, no hash or thread.
pub fn spread_pits(grid: &Grid, z: &mut [f64], sea_level: f64, min_depth_m: f64, max_passes: usize) -> usize {
    if grid.is_cube() {
        return spread_pits_cube(grid, z, sea_level, min_depth_m, max_passes);
    }
    let w = grid.w();
    assert!(w > 0 && z.len().is_multiple_of(w), "one height per cell");
    let h = z.len() / w;
    super::assert_finite("the heights", z);
    let around = [-1i64, 0, 1]
        .into_iter()
        .flat_map(|dj| [-1i64, 0, 1].into_iter().map(move |di| (di, dj)))
        .filter(|d| *d != (0, 0))
        .collect::<Vec<_>>();
    for pass in 0..max_passes {
        let mut moved = false;
        // A pit whose ring reaches the border is left: the border cells are outlets and only receive.
        for j in 2..h.saturating_sub(2) {
            for i in 2..w.saturating_sub(2) {
                let p = j * w + i;
                if z[p] <= sea_level {
                    continue;
                }
                let mut lowest = f64::INFINITY;
                for (di, dj) in &around {
                    let q = ((j as i64 + dj) as usize) * w + (i as i64 + di) as usize;
                    lowest = lowest.min(z[q]);
                }
                let depth = lowest - z[p];
                let rise = 0.8 * depth;
                if depth > min_depth_m && lowest - rise / 8.0 > sea_level {
                    z[p] += rise;
                    for (di, dj) in &around {
                        let q = ((j as i64 + dj) as usize) * w + (i as i64 + di) as usize;
                        z[q] -= rise / 8.0;
                    }
                    moved = true;
                }
            }
        }
        if !moved {
            return pass;
        }
    }
    max_passes
}

/// [`spread_pits`] on a cube grid (GEN-150): a land cell with all 8 neighbours on the grid that is lower than all of them by more than `min_depth_m` rises by 0.8 of its depth below the lowest
/// neighbour; each neighbour falls by the rise's volume over 8 and its own area, so the volume (height times cell area) is kept. Cells in cell order (the cube-id order of a compact grid), passes
/// as on the plane; a cell at the edge of the landmass (a neighbour off the grid) is left. Deterministic: serial, fixed order.
fn spread_pits_cube(grid: &Grid, z: &mut [f64], sea_level: f64, min_depth_m: f64, max_passes: usize) -> usize {
    assert_eq!(z.len(), grid.len(), "one height per cell");
    super::assert_finite("the heights", z);
    let mut ring = [(0usize, 0.0f64); 8];
    let cube = grid.cube.as_ref().expect("a cube grid");
    let id_of = |p: usize| if grid.is_compact() { grid.cube_id_of(p) } else { p };
    for pass in 0..max_passes {
        let mut moved = false;
        for p in 0..grid.len() {
            if z[p] <= sea_level {
                continue;
            }
            let mut lowest = f64::INFINITY;
            // The neighbours the cell has on the cube (a cell on a cube edge has 6, a corner 5: the diagonals across the seam are not in the graph) against those on this grid.
            let full = (0..8).filter(|&d| cube.neighbour(id_of(p), d).is_some()).count();
            let mut k = 0;
            for d in 0..8 {
                if let Some(q) = grid.neighbour(p, d) {
                    ring[k] = (q, grid.cell_area(q));
                    lowest = lowest.min(z[q]);
                    k += 1;
                }
            }
            if k < full || k == 0 {
                continue;
            }
            let ring = &ring[..k];
            let depth = lowest - z[p];
            let rise = 0.8 * depth;
            if depth <= min_depth_m {
                continue;
            }
            let ca = grid.cell_area(p);
            // The fall of a neighbour keeps the volume: rise * ca / 8 over its own area.
            let fall = |area: f64| rise * ca / (k as f64 * area);
            if ring.iter().any(|&(q, area)| z[q] - fall(area) <= sea_level) {
                continue;
            }
            z[p] += rise;
            for &(q, area) in ring {
                z[q] -= fall(area);
            }
            moved = true;
        }
        if !moved {
            return pass;
        }
    }
    max_passes
}
