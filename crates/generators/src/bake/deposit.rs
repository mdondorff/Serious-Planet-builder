//! The deposition sweep (GEN-101; SPIKE-64's kernel, transport-limited): one pass from upstream to downstream over a routing.
//!
//! The load of a cell is its donors' plus its own eroded column (`original - current` where it is positive, times the cell area; land only). The load first fills the cell's
//! depression up to the filled level, then, where it exceeds the capacity `kc sqrt(A) S` (cubic metres per step, `A` the drainage area, `S` the filled slope to the receiver),
//! `zeta` of the excess settles, raising the cell at most to the filled level plus half the drop to the receiver, so that the receiver stays lower. The rest moves on; what reaches
//! an outlet (the sea or the grid's border) leaves. Mass is kept: what was eroded is deposited or leaves.

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
    let mut carried = vec![0.0f64; n];
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
        // The depression first.
        let pit = ((fp - zp).max(0.0) * ca).min(load);
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
        z[p] = zp + rise;
        report.deposited += rise * ca;
        carried[r] += load;
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
