//! The sediment steps on a grid after the rounds (GEN-151; A3 section 6.7): thermal, then the deposition sweep on the surface's own route, then the one-cell pit spreading, in the order of K7.
//!
//! They read 8 neighbours and cross divides, so they are not catchment-round steps. This function runs them on a whole grid, a cube grid (compact landmass) included; which tile and halo a
//! later wiring cuts them into is that wiring's decision. Nothing in the default bake calls it.

use super::deposit::{deposit, spread_pits, DepositParams, DepositReport, Kc};
use super::flood::priority_flood_with;
use super::route::{route_rules, MfdClip, RoundRules, Routing};
use super::thermal::{thermal, ThermalReport};
use super::tile::Pass2Params;
use super::Grid;

/// The land cells whose fill depth reaches a depth: the pits of a surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PitCells {
    pub land_cells: usize,
    pub pit_cells: usize,
    pub p50_m: f64,
    pub p99_m: f64,
    pub max_m: f64,
}

impl PitCells {
    /// The share of the land cells that are pits.
    pub fn share(&self) -> f64 {
        self.pit_cells as f64 / self.land_cells.max(1) as f64
    }
}

/// The cells of a cube grid that have fewer neighbours than on the whole cube: the edge of a compact or masked landmass (its coast ring borders the sea that is not in the grid). Empty on a plane grid and on the full cube.
pub fn boundary(grid: &Grid) -> Vec<bool> {
    let Some(cube) = grid.cube.as_ref() else { return vec![false; grid.len()] };
    (0..grid.len())
        .map(|p| {
            let id = if grid.is_compact() { grid.cube_id_of(p) } else { p };
            (0..8).filter(|&d| grid.neighbour(p, d).is_some()).count() < (0..8).filter(|&d| cube.neighbour(id, d).is_some()).count()
        })
        .collect()
}

/// The outlets of the sediment steps: the sea, the grid's border and the landmass's boundary cells (water leaves a coast ring cell towards the sea that is not in the grid, also where thermal lifted it above the sea).
pub fn outlets(grid: &Grid, z: &[f64], sea_level: f64) -> Vec<bool> {
    let b = boundary(grid);
    grid.outlets(z, sea_level).into_iter().zip(b).map(|(o, b)| o || b).collect()
}

/// The eroded column in cubic metres: `original - z` where positive, on land.
pub fn eroded_m3(grid: &Grid, z: &[f64], original: &[f64], sea_level: f64) -> f64 {
    let cube = grid.is_cube();
    (0..grid.len())
        .filter(|&p| z[p] > sea_level)
        .map(|p| (original[p] - z[p]).max(0.0) * if cube { grid.cell_area(p) } else { grid.dx() * grid.dx() })
        .sum()
}

/// The filled surface and the routing of `z` with the canonical flood (cells visited by level, then cell id; GEN-141, GEN-159): the surface the rounds and H1 read, so the pits and the sweep's route are those of H1. Outlets as given.
fn canonical_surface(grid: &Grid, z: &[f64], outlet: &[bool], epsilon: f64, seed: u64) -> (Vec<f64>, Routing) {
    let filled = priority_flood_with(grid, z, outlet, epsilon, true);
    let routing = route_rules(grid, z, outlet, epsilon, seed, 0.0, None, RoundRules { clip_mfd: MfdClip::Off, canonical_flood: true });
    (filled, routing)
}

/// The pits of `z` on any grid (outlets: the sea, the grid's border and a landmass's boundary cells) at depth `min_m`.
pub fn pit_cells(grid: &Grid, z: &[f64], sea_level: f64, epsilon: f64, min_m: f64) -> PitCells {
    let outlet = outlets(grid, z, sea_level);
    let filled = priority_flood_with(grid, z, &outlet, epsilon, true);
    let mut depths: Vec<f64> = Vec::new();
    let mut land = 0usize;
    for p in 0..z.len() {
        if z[p] > sea_level && !outlet[p] {
            land += 1;
            let d = filled[p] - z[p];
            if d >= min_m {
                depths.push(d);
            }
        }
    }
    depths.sort_by(|a, b| a.total_cmp(b));
    let at = |f: f64| if depths.is_empty() { 0.0 } else { depths[((depths.len() - 1) as f64 * f) as usize] };
    PitCells { land_cells: land, pit_cells: depths.len(), p50_m: at(0.5), p99_m: at(0.99), max_m: depths.last().copied().unwrap_or(0.0) }
}

/// The volume of `z` above 0 m times the cell areas in cubic metres: the quantity thermal and the pit spreading keep.
pub fn volume_m3(grid: &Grid, z: &[f64]) -> f64 {
    let cube = grid.is_cube();
    (0..grid.len()).map(|p| z[p] * if cube { grid.cell_area(p) } else { grid.dx() * grid.dx() }).sum()
}

/// The sum of `|z|` times the cell area in cubic metres: the scale a relative change of the volume is taken against.
pub fn abs_volume_m3(grid: &Grid, z: &[f64]) -> f64 {
    let cube = grid.is_cube();
    (0..grid.len()).map(|p| z[p].abs() * if cube { grid.cell_area(p) } else { grid.dx() * grid.dx() }).sum()
}

/// What the sediment steps did.
#[derive(Clone, Debug, PartialEq)]
pub struct SedimentReport {
    /// The pits before the steps, after thermal, after the deposition and after the pit spreading.
    pub pits: Vec<(&'static str, PitCells)>,
    pub thermal: ThermalReport,
    pub deposit: DepositReport,
    pub spread_passes: usize,
    /// The change of the height volume over the deposition in cubic metres: it equals `deposit.deposited` (the heights moved by what the sweep reports).
    pub deposit_height_volume_m3: f64,
    /// The eroded column (`original - z` on land) before thermal and after it, in cubic metres: the deposition reads the column after thermal, so thermal's moves onto cells above `original` or onto the sea count as load (K7 does the same).
    pub eroded_before_thermal_m3: f64,
    pub eroded_after_thermal_m3: f64,
    /// The relative change of the volume (against the sum of `|z|` times the area) over thermal and over the pit spreading (both keep it).
    pub thermal_volume_change: f64,
    pub spread_volume_change: f64,
}

/// Runs thermal, the deposition (the eroded column is `original - z` on land) and the pit spreading on `z` in place, with pass 2's parameters (`p.carve.epsilon` is the flood's, `p.seed` the jitter's stream).
pub fn sediment_pass(grid: &Grid, z: &mut [f64], original: &[f64], p: &Pass2Params, threads: usize) -> SedimentReport {
    assert_eq!(z.len(), grid.len(), "one height per cell");
    assert_eq!(original.len(), grid.len(), "one original height per cell");
    let eps = p.carve.epsilon;
    let stats = |z: &[f64]| pit_cells(grid, z, p.sea_level, eps, p.pit_min_m);
    let mut pits = vec![("start", stats(z))];
    let scale = abs_volume_m3(grid, z).max(1.0);
    let rel = |a: f64, b: f64| (b - a) / scale;
    let eroded_before_thermal_m3 = eroded_m3(grid, z, original, p.sea_level);
    let v0 = volume_m3(grid, z);
    let th = thermal(grid, z, &p.thermal, threads);
    let v1 = volume_m3(grid, z);
    let eroded_after_thermal_m3 = eroded_m3(grid, z, original, p.sea_level);
    pits.push(("after thermal", stats(z)));
    let v1b = volume_m3(grid, z);
    // The deposition on the surface's own route, with the boundary cells as outlets.
    let outlet = outlets(grid, z, p.sea_level);
    let (filled, routing) = canonical_surface(grid, z, &outlet, eps, p.seed);
    let deposit =
        deposit(grid, z, original, &filled, &routing, &DepositParams { kc: Kc::Uniform(p.kc), zeta: p.zeta, sea_level: p.sea_level });
    pits.push(("after deposition", stats(z)));
    let v2 = volume_m3(grid, z);
    let deposit_height_volume_m3 = v2 - v1b;
    let spread_passes = spread_pits(grid, z, p.sea_level, p.pit_min_m, p.pit_passes);
    let v3 = volume_m3(grid, z);
    pits.push(("after pit spreading", stats(z)));
    SedimentReport {
        pits,
        thermal: th,
        deposit,
        spread_passes,
        deposit_height_volume_m3,
        eroded_before_thermal_m3,
        eroded_after_thermal_m3,
        thermal_volume_change: rel(v0, v1),
        spread_volume_change: rel(v2, v3),
    }
}

/// The volume the breach alone lowers on `z` (a carve with no incision, as K7 measures it: pass 1's breach rule at the grid's own resolution), in cubic metres, land only. The deposition counts
/// this volume as eroded load when its `original` is the surface before the breach (K7's order; the departure of GEN-148, measured by GEN-151).
pub fn breach_lowered_m3(grid: &Grid, z: &[f64], outlet: &[bool], carve: &super::carve::CarveParams, seed: u64) -> f64 {
    let mut b = z.to_vec();
    super::carve::carve(grid, &mut b, outlet, &super::carve::CarveParams { k_dt: 0.0, steps: 1, reroute_every: 1, ..*carve }, None, seed);
    let cube = grid.is_cube();
    (0..grid.len())
        .filter(|&p| z[p] > carve.sea_level)
        .map(|p| (z[p] - b[p]).max(0.0) * if cube { grid.cell_area(p) } else { grid.dx() * grid.dx() })
        .sum()
}

/// G-SED per catchment (GEN-152): the deposition sweep on `z` (in place) with `original`, and for every catchment of the sweep's own route the balance of its heights against its flow.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CatchmentBalance {
    /// Catchments with an eroded column or a deposit.
    pub catchments: usize,
    pub eroded_m3: f64,
    /// The volume the heights rose by (from the heights, not from the sweep's report).
    pub raised_m3: f64,
    pub exported_m3: f64,
    /// The largest `|eroded - raised - exported|` of a catchment over the largest of the three terms of that catchment.
    pub max_relative_error: f64,
    /// The same over the whole region.
    pub total_relative_error: f64,
    /// The largest catchment's eroded column and its share of the total.
    pub largest_eroded_m3: f64,
}

/// Runs the deposition on `z` and balances it per catchment (a catchment is the cells whose route ends at the same outlet cell): eroded column = the heights' rise + the load exported at the outlet. The rise is read from the heights, the export from the sweep's carried load, so the identity checks the sweep's accounting against the heights and against the catchment border (no load crosses it).
pub fn catchment_balance(grid: &Grid, z: &mut [f64], original: &[f64], p: &Pass2Params) -> CatchmentBalance {
    let n = grid.len();
    let outlet = outlets(grid, z, p.sea_level);
    let (filled, routing) = canonical_surface(grid, z, &outlet, p.carve.epsilon, p.seed);
    let before = z.to_vec();
    let mut exports = vec![0.0f64; n];
    let params = DepositParams { kc: Kc::Uniform(p.kc), zeta: p.zeta, sea_level: p.sea_level };
    super::deposit::deposit_exports(grid, z, original, &filled, &routing, &params, &mut exports);
    // The outlet cell of every cell's route: the order lists a receiver before its donors.
    let mut root = vec![u32::MAX; n];
    for &c in &routing.order {
        let c = c as usize;
        let r = routing.receiver[c];
        root[c] = if r == super::route::NONE { c as u32 } else { root[r as usize] };
    }
    let cube = grid.is_cube();
    let area = |c: usize| if cube { grid.cell_area(c) } else { grid.dx() * grid.dx() };
    let grid_area = (0..n).map(&area).fold(0.0f64, f64::max);
    let (mut eroded, mut raised) = (vec![0.0f64; n], vec![0.0f64; n]);
    let mut cells = vec![0u32; n];
    let zmax = z.iter().chain(original).fold(0.0f64, |m, v| m.max(v.abs()));
    for c in 0..n {
        let r = root[c] as usize;
        cells[r] += 1;
        if before[c] > p.sea_level {
            eroded[r] += (original[c] - before[c]).max(0.0) * area(c);
        }
        raised[r] += (z[c] - before[c]) * area(c);
    }
    let mut out = CatchmentBalance::default();
    for r in 0..n {
        let (e, d, x) = (eroded[r], raised[r], exports[r]);
        if e == 0.0 && d == 0.0 && x == 0.0 {
            continue;
        }
        out.catchments += 1;
        out.eroded_m3 += e;
        out.raised_m3 += d;
        out.exported_m3 += x;
        out.largest_eroded_m3 = out.largest_eroded_m3.max(e);
        // The rounding of `z + rise - z` is absolute, about one ulp of the height times the cell area per cell: a floor under the scale, so a sub-ulp deposit is not read as a balance error.
        let floor = f64::EPSILON * zmax * grid_area * f64::from(cells[r]);
        let scale = e.max(d.abs()).max(x).max(floor);
        out.max_relative_error = out.max_relative_error.max((e - d - x).abs() / scale.max(f64::MIN_POSITIVE));
    }
    out.total_relative_error = (out.eroded_m3 - out.raised_m3 - out.exported_m3).abs() / out.eroded_m3.max(f64::MIN_POSITIVE);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bake::route::route;

    // spec: GEN-159
    #[test]
    fn the_sweeps_surface_and_route_are_the_canonical_flood_s() {
        // A flat chain (columns 3 to 9 of row 4, 0.01 m) between two sea cells (columns 2 and 11), walls rising inwards: on the canonical surface the chain's last cell drains to the cell beside the second sea cell;
        // on the plain flood it drains back along the chain.
        let g = Grid::new(16, 9, 100.0);
        let mut z: Vec<f64> = (0..16 * 9).map(|p| 100.0 + (p % 16).min(p / 16).min(15 - p % 16).min(8 - p / 16) as f64).collect();
        let at = |x: usize| 4 * 16 + x;
        z[at(2)] = 0.0;
        z[at(11)] = 0.0;
        for x in 3..=10 {
            z[at(x)] = 0.01;
        }
        let outlet = g.outlets(&z, 0.0);
        let (filled, routing) = canonical_surface(&g, &z, &outlet, 0.02, 7);
        assert_eq!(filled, priority_flood_with(&g, &z, &outlet, 0.02, true));
        let canonical = route_rules(&g, &z, &outlet, 0.02, 7, 0.0, None, RoundRules { clip_mfd: MfdClip::Off, canonical_flood: true });
        assert_eq!(routing.receiver, canonical.receiver);
        assert_eq!(routing.receiver[at(9)] as usize, at(10), "column 9 drains towards the second sea cell");
        let plain = route(&g, &z, &outlet, 0.02, 7, 0.0);
        assert_eq!(plain.receiver[at(9)] as usize, at(8), "the plain flood drains it back along the chain");
    }
}
