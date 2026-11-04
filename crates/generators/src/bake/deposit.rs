//! The deposition sweep (GEN-101; SPIKE-64's kernel, transport-limited): one pass from upstream to downstream over a routing.
//!
//! The load of a cell is its donors' plus its own eroded column (`original - current` where it is positive, times the cell area; land only). The load first fills the cell's
//! depression up to the filled level, at most to the receiver's carved height plus half the filled drop (a bowl's route does not become a pillar of the bowl's fill), then, where it exceeds the capacity `kc sqrt(A) S` (cubic metres per step, `A` the drainage area, `S` the filled slope to the receiver),
//! `zeta` of the excess settles, raising the cell at most to the filled level plus half the drop to the receiver, so that the receiver stays lower. The rest moves on; what reaches
//! an outlet (the sea or the grid's border) leaves. A cell never ends above the lowest final height of its donors (so a settled cell stays below the cells that drain into it); what
//! the ceiling holds back moves on. Mass is kept: what was eroded is deposited or leaves. The eroded column is counted on land only (`z` above the sea level): a carve that
//! lowers land below the sea would lose that volume here, as the carve of this crate does not.

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
    let n = grid.len();
    assert!(z.len() == n && original.len() == n && filled.len() == n && routing.receiver.len() == n, "one value per cell");
    assert!((0.0..=1.0).contains(&params.zeta), "zeta is a share in [0, 1], not {}", params.zeta);
    super::assert_finite("the heights", z);
    super::assert_finite("the original heights", original);
    if let Kc::PerCell(k) = params.kc {
        assert_eq!(k.len(), n, "one kc per cell");
    }
    let ca = grid.dx() * grid.dx();
    super::assert_finite("the filled heights", filled);
    assert!(routing.area.len() == n && routing.order.len() == n, "one drainage area and one order entry per cell");
    assert!(routing.area.iter().all(|a| a.is_finite() && *a >= 0.0), "the drainage areas are finite and not negative");
    let mut carried = vec![0.0f64; n];
    let mut ceiling = vec![f64::INFINITY; n];
    let mut report = DepositReport { eroded: 0.0, deposited: 0.0, exported: 0.0 };
    for &p in routing.order.iter().rev() {
        let p = p as usize;
        let land = z[p] > params.sea_level;
        let eroded = if land { (original[p] - z[p]).max(0.0) * ca } else { 0.0 };
        report.eroded += eroded;
        let mut load = carried[p] + eroded;
        let r = routing.receiver[p];
        if r == NONE {
            report.exported += load;
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
        let (di, dj) = ((p % grid.w()).abs_diff(r % grid.w()), (p / grid.w()).abs_diff(r / grid.w()));
        let dist = if di == 0 || dj == 0 { grid.dx() } else { grid.dx() * core::f64::consts::SQRT_2 };
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
