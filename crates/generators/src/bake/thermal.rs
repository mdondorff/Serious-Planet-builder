//! Thermal stabilisation on active cells (GEN-101; SPIKE-64's kernel after Schott et al. 2024's thermal pass): a Jacobi gather, so the result does not depend on the thread count.
//!
//! Each pass every cell gives each lower neighbour `k` of the height difference above the critical one and takes the same from each higher neighbour, all read from the previous
//! pass: the mass is kept. The critical difference is `tan(theta) (1 + perturb u)` times the distance, with `u` in `[-1, 1)` an integer hash of the unordered pair of GLOBAL cell
//! ids, so both cells of a pair see the same critical slope and a window of a larger grid sees the one the whole grid does. Only land (above the sea level) off the grid's border
//! gives; any cell receives (the sea and the border are sinks).
//!
//! After the first pass only the cells that moved and their neighbours can change: a cell's transfer is a function of its own height and its neighbours', so a cell none of whose
//! neighbourhood changed moves exactly as it did, which is nothing if it did not move. The active set is that, exact: the result is bit for bit the one of a pass over every cell.

use super::Grid;
use planet_core::par::{self, Par};

/// The parameters of the thermal pass (GEN-101's start values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThermalParams {
    /// The critical angle in degrees (the angle of repose).
    pub theta_deg: f64,
    /// The perturbation of the critical slope per pair: `1 + perturb u`.
    pub perturb: f64,
    /// The share of the excess one pass moves.
    pub k: f64,
    /// A pair whose height difference is over the critical one by less than this many metres does not move (the excess falls by a fixed share each pass and would never reach 0).
    pub min_excess: f64,
    /// At most this many passes (the run stops earlier when nothing moves).
    pub passes: usize,
    /// Land is above this height; the sea and the grid's border only receive.
    pub sea_level: f64,
}

impl Default for ThermalParams {
    fn default() -> ThermalParams {
        ThermalParams { theta_deg: 33.0, perturb: 0.25, k: 1.0 / 16.0, min_excess: 1e-3, passes: 100, sea_level: 0.0 }
    }
}

/// What a thermal run did.
#[derive(Clone, Debug, PartialEq)]
pub struct ThermalReport {
    pub passes_run: usize,
    /// Cells that gave in the last pass run.
    pub moved_last: usize,
    /// The volume moved in the first pass in cubic metres (the sum of what the givers lost, times the cell area).
    pub moved_volume_first: f64,
    /// Cells evaluated over all passes (the cost: `passes_run * cells` for the full variant).
    pub evaluations: u64,
    /// The volume in cubic metres that cells gave to the fixed cells of the run (a fixed cell takes nothing: the material leaves the grid as exported sediment, like the sea and the border), over all passes (GEN-176).
    pub exported_to_fixed_m3: f64,
}

/// A number in `[-1, 1)`: an integer hash of the unordered pair of global cell ids.
pub fn pair_u(a: u64, b: u64) -> f64 {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    let x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let x = (x ^ (x >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let x = x ^ (x >> 32);
    (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// The change of cell `p` in one pass, from the heights `z` of the previous one, and the part of what it gave that went to a fixed cell (`fixed` empty: none; a fixed cell neither gives nor takes: GEN-176).
fn delta(grid: &Grid, z: &[f64], p: usize, tan_theta: f64, params: &ThermalParams, fixed: &[bool]) -> (f64, f64) {
    let is_fixed = |c: usize| !fixed.is_empty() && fixed[c];
    if is_fixed(p) {
        return (0.0, 0.0);
    }
    let (zp, gp) = (z[p], grid.global_id(p));
    let gives = zp > params.sea_level && !grid.is_edge(p);
    let mut dh = 0.0;
    let mut to_fixed = 0.0;
    // On a cube grid (GEN-150) the distance is the sphere's and a cell takes the giver's volume over its own area, so the volume is kept; on a plane grid both are 1 and `dx`.
    let cube = grid.is_cube();
    let ap = if cube { grid.cell_area(p) } else { 1.0 };
    for d in 0..8 {
        let Some(q) = grid.neighbour(p, d) else { continue };
        let dist = if cube { grid.distance_at(p, d) } else { grid.distance(d) };
        let crit = tan_theta * (1.0 + params.perturb * pair_u(gp, grid.global_id(q))) * dist;
        let diff = zp - z[q];
        if diff - crit > params.min_excess && gives {
            dh -= params.k * (diff - crit);
            if is_fixed(q) {
                to_fixed += params.k * (diff - crit);
            }
        } else if -diff - crit > params.min_excess && z[q] > params.sea_level && !grid.is_edge(q) && !is_fixed(q) {
            let share = if cube { grid.cell_area(q) / ap } else { 1.0 };
            dh += params.k * (-diff - crit) * share;
        }
    }
    (dh, to_fixed)
}

/// Cells per piece of work.
const CHUNK: usize = 4096;

fn run(grid: &Grid, z: &mut [f64], params: &ThermalParams, threads: usize, all_cells: bool, fixed: &[bool]) -> ThermalReport {
    assert_eq!(z.len(), grid.len(), "one height per cell");
    assert!(fixed.is_empty() || fixed.len() == grid.len(), "one fixed flag per cell, or none");
    super::assert_finite("the heights", z);
    assert!(grid.len() <= u32::MAX as usize, "the active set holds u32 cell indices: {} cells are too many", grid.len());
    assert!(params.sea_level.is_finite(), "the sea level is finite, not {}", params.sea_level);
    assert!(
        params.min_excess >= 0.0 && params.min_excess.is_finite(),
        "the minimum excess is finite and not negative, not {}",
        params.min_excess
    );
    assert!(params.theta_deg > 0.0 && params.theta_deg < 90.0, "the critical angle is between 0 and 90 degrees, not {}", params.theta_deg);
    assert!(
        (0.0..1.0).contains(&params.perturb),
        "the perturbation is under 1 (the critical slope stays positive), not {}",
        params.perturb
    );
    assert!(
        params.k > 0.0 && params.k <= 0.125,
        "the share moved per pass is in (0, 1/8] (a cell has up to 8 neighbours and must not overshoot), not {}",
        params.k
    );
    let tan_theta = libm::tan(params.theta_deg.to_radians());
    let par: &dyn Par = &threads;
    let mut active: Vec<u32> = (0..grid.len() as u32).collect();
    let mut report = ThermalReport { passes_run: 0, moved_last: 0, moved_volume_first: 0.0, evaluations: 0, exported_to_fixed_m3: 0.0 };
    let area = grid.dx() * grid.dx();
    let cube = grid.is_cube();
    for pass in 0..params.passes {
        if active.is_empty() {
            break;
        }
        let pieces = active.len().div_ceil(CHUNK);
        let deltas: Vec<Vec<(f64, f64)>> = {
            let (zr, act) = (&*z, &active);
            par::map(par, pieces, &|c| {
                act[c * CHUNK..((c + 1) * CHUNK).min(act.len())]
                    .iter()
                    .map(|&p| delta(grid, zr, p as usize, tan_theta, params, fixed))
                    .collect()
            })
        };
        report.evaluations += active.len() as u64;
        let mut changed: Vec<u32> = Vec::new();
        let mut moved = 0usize;
        let mut volume = 0.0;
        for (c, piece) in deltas.iter().enumerate() {
            for (o, &(dh, to_fixed)) in piece.iter().enumerate() {
                report.exported_to_fixed_m3 += if cube { 0.0 } else { to_fixed * area };
                if dh != 0.0 {
                    let p = active[c * CHUNK + o];
                    let before = z[p as usize];
                    z[p as usize] += dh;
                    // A change that rounding absorbed moves nothing and wakes no neighbour (its inputs are the same next pass).
                    if z[p as usize].to_bits() != before.to_bits() {
                        changed.push(p);
                        if dh < 0.0 {
                            moved += 1;
                            volume -= if cube { dh * grid.cell_area(p as usize) } else { dh };
                        }
                    }
                }
            }
        }
        report.passes_run = pass + 1;
        report.moved_last = moved;
        if pass == 0 {
            report.moved_volume_first = if cube { volume } else { volume * area };
        }
        if all_cells {
            if changed.is_empty() {
                break;
            }
            continue;
        }
        // The cells that changed and their neighbours are the next pass's: nothing else can move.
        let mut next: Vec<u32> = Vec::with_capacity(changed.len() * 9);
        for &p in &changed {
            next.push(p);
            next.extend((0..8).filter_map(|d| grid.neighbour(p as usize, d)).map(|q| q as u32));
        }
        next.sort_unstable();
        next.dedup();
        active = next;
    }
    report
}

/// Thermal stabilisation of `z` in place on active cells (see the module): at most `params.passes` passes, fewer when nothing moves. `z` must be finite. Refused: a critical angle
/// outside (0, 90) degrees, a perturbation of 1 or more, a share `k` over 1/8 or not positive, a negative minimum excess.
pub fn thermal(grid: &Grid, z: &mut [f64], params: &ThermalParams, threads: usize) -> ThermalReport {
    run(grid, z, params, threads, false, &[])
}

/// [`thermal`] with fixed cells (GEN-176): a fixed cell neither gives nor takes (a boundary like the sea and the grid's border); what its neighbours would have given it leaves the grid and is
/// counted in `exported_to_fixed_m3`. `fixed` has one flag per cell. Not for a cube grid.
pub fn thermal_fixed(grid: &Grid, z: &mut [f64], params: &ThermalParams, threads: usize, fixed: &[bool]) -> ThermalReport {
    assert!(!grid.is_cube() || fixed.is_empty(), "fixed cells are for a plane grid");
    run(grid, z, params, threads, false, fixed)
}

/// The same as [`thermal`] over every cell every pass: the reference the active-cell run is tested against.
pub fn thermal_all_cells(grid: &Grid, z: &mut [f64], params: &ThermalParams, threads: usize) -> ThermalReport {
    run(grid, z, params, threads, true, &[])
}
