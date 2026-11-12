//! The build's phase plan on a compact landmass (GEN-158, A3 section 6.6 rule 1): the rounds, H1 and the sediment pass in one process, resident, in the order that keeps the process's peak lowest.
//! The plan owns what is alive in each phase and nothing else:
//! - the pre-carve surface is written to a spill file before the rounds and read back only for the sediment pass (8 bytes a cell on disk, deleted after the read-back);
//! - H1's drivers (the runoff weight, the runoff and the open-water evaporation, f32 a cell) are computed from a climate closure; the weight is dropped after the weighted route, the evaporation after H1;
//! - H1's per-cell potential and routed discharge (`q`, `qw`) are temporaries: the conservation check (G-CON, [`verify`]) is made on them and they are released before the sediment pass; the result keeps the channel graph, the basins and the labels;
//! - the weighted route's own filled surface is H1's (one flood, not two);
//! - the routing, the filled surface and the drivers are dropped as soon as H1 has its result.
//!
//! `mark` is called at every allocation boundary, so a caller can read the process's memory there. The stage probe (BUILD-046) is called at the landmass and at H1; the rounds and the sediment steps emit their own.
//! Nothing here changes a bit of what the steps compute: the plan is the order and the lifetimes.

use super::carve::CarveParams;
use super::channel_graph::ChannelGraph;
use super::hydro::{hydrology, verify_with_area, weighted_cell_area, Hydro, HydroParams, HydroStats, Verify};
use super::lakes::{lake_records, river_pieces, Lakes};
use super::precision::WorldPrecision;
use super::probe;
use super::round::{solve_rounds, RoundReport};
use super::route::{route_rules_filled, RoundRules};
use super::sediment::{sediment_pass, SedimentReport};
use super::tile::Pass2Params;
use super::Grid;
use super::Heights;
use crate::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Seconds in a Julian year: the runoff and evaporation of the drivers are in metres a second.
const YEAR_S: f64 = 31_557_600.0;
/// The values moved to and from the spill file at a time (the spill adds this many bytes to the process, not the surface's size).
const SPILL_CHUNK: usize = 1 << 20;

/// The parameters of the plan.
#[derive(Clone, Debug)]
pub struct PlanParams {
    pub seed: u64,
    pub threads: usize,
    pub rounds: usize,
    pub carve: CarveParams,
    pub hydro: HydroParams,
    /// The epsilon of the weighted route and the filled surface of H1.
    pub epsilon: f64,
    pub sediment: Pass2Params,
    /// The world's precision (GEN-175), when the plan was made for known height bounds: the epsilons that act on an f32 surface are its `epsilon_w` and the heights after the rounds are checked against its `R`. `None`: no check (the epsilons are the caller's).
    pub precision: Option<WorldPrecision>,
    /// Where the pre-carve surface waits (created and deleted by the plan).
    pub spill: PathBuf,
}

impl PlanParams {
    /// The prototype's values: 4 rounds of 20 steps (the route rebuilt every 5) under the round rules, H1's defaults, pass 2's sediment parameters with pass 1's breach limit.
    pub fn standard(seed: u64, threads: usize, spill: PathBuf) -> PlanParams {
        let mut sediment = Pass2Params::standard(seed, 0.0);
        sediment.carve.breach_max_m = CarveParams::default().breach_max_m;
        PlanParams {
            seed,
            threads,
            rounds: 4,
            carve: CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, round_f32: true, ..CarveParams::default() },
            hydro: HydroParams::default(),
            epsilon: 1e-3,
            sediment,
            precision: None,
            spill,
        }
    }
}

impl PlanParams {
    /// [`PlanParams::standard`] for a world with known height bounds (GEN-175): the carve, H1's fill and pass 2 (K7 and the sediment pass) flood with the world's `epsilon_w`, the rounds are f32 or f64 as the world chooses,
    /// and `build_resident` refuses a surface whose heights leave the bounds.
    pub fn standard_for(seed: u64, threads: usize, spill: PathBuf, world: &WorldPrecision) -> PlanParams {
        let mut p = PlanParams::standard(seed, threads, spill);
        p.apply_precision(world);
        p
    }

    /// Set every epsilon of the plan that acts on an f32 surface to the world's (GEN-175): the carve (and its f32 flag), H1's fill and weighted route (`epsilon`) and the sediment pass (`sediment.carve.epsilon`;
    /// pass 2 never takes the f32 flag). K7 has its own parameters: `WorldPrecision::apply_k7`. The plan remembers the world so that `build_resident` checks the heights against `R`.
    pub fn apply_precision(&mut self, world: &WorldPrecision) {
        world.apply(&mut self.carve);
        self.epsilon = world.epsilon_w;
        self.sediment.carve.epsilon = world.epsilon_w;
        self.precision = Some(*world);
    }
}

/// What the plan did.
#[derive(Debug)]
pub struct PlanReport {
    pub rounds: RoundReport,
    pub h1: HydroStats,
    /// The conservation check of H1 (G-CON), made before `q` and `qw` were released.
    pub g_con: Verify,
    /// The sums of `Q` and `Q_w` over the cells before the release (m3/s; each discharge is counted along its whole path: not a conservation figure).
    pub sum_q: f64,
    pub sum_qw: f64,
    pub sediment: SedimentReport,
    /// The size of the spill file in bytes.
    pub spill_bytes: u64,
    /// The wall seconds of each phase.
    pub phases: Vec<(&'static str, f64)>,
}

/// What the plan leaves resident: the surface after the sediment pass and H1's result without the per-cell discharges.
pub struct Built {
    pub z: Vec<f64>,
    pub hydro: Hydro,
    /// The lake producer's result on H1 (GEN-163): the lake records and the river pieces outside the lakes.
    pub lakes: Lakes,
    /// The channel graph as compact arrays (item 15 PR 7, D51): chains, `Q_w` codes and bed codes, built before the discharges are released.
    pub graph: ChannelGraph,
    pub report: PlanReport,
}

/// Writes `v` to `path` as little-endian f64, `SPILL_CHUNK` values at a time.
fn spill_write(path: &Path, v: &[f64]) -> std::io::Result<u64> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut buf: Vec<u8> = Vec::with_capacity(SPILL_CHUNK * 8);
    for chunk in v.chunks(SPILL_CHUNK) {
        buf.clear();
        buf.extend(chunk.iter().flat_map(|x| x.to_le_bytes()));
        f.write_all(&buf)?;
    }
    f.flush()?;
    Ok(v.len() as u64 * 8)
}

#[allow(clippy::chunks_exact_to_as_chunks)]
/// Reads `n` little-endian f64 from `path`, `SPILL_CHUNK` values at a time (no second copy of the surface).
fn spill_read(path: &Path, n: usize) -> std::io::Result<Vec<f64>> {
    let mut f = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut out: Vec<f64> = Vec::with_capacity(n);
    let mut buf = vec![0u8; SPILL_CHUNK * 8];
    while out.len() < n {
        let take = (n - out.len()).min(SPILL_CHUNK);
        f.read_exact(&mut buf[..take * 8])?;
        out.extend(buf[..take * 8].chunks_exact(8).map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])));
    }
    Ok(out)
}

/// Deletes the spill file when it goes out of scope (also on an error or a panic); a failed removal is a warning, not a lost build.
struct SpillGuard(PathBuf);

impl Drop for SpillGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.0) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("warning: the spill file {} was not removed: {e}", self.0.display());
            }
        }
    }
}

/// The climate of a cell: (precipitation in mm a year, temperature in degrees C).
pub type Climate<'a> = &'a (dyn Fn(usize) -> (f64, f64) + Sync);

/// H1's drivers on the land of `z` (a cell at or under the sea has weight 1 and no runoff): the runoff weight, the runoff and the open-water evaporation, f32 a cell.
fn drivers(z: &(impl Heights + ?Sized), threads: usize, climate: Climate) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let n = z.len();
    let mut weight = vec![1.0f32; n];
    let mut runoff = vec![0.0f32; n];
    let mut evap = vec![0.0f32; n];
    let per = n.div_ceil(threads.max(1));
    std::thread::scope(|s| {
        for (((wc, rc), ec), t) in weight.chunks_mut(per).zip(runoff.chunks_mut(per)).zip(evap.chunks_mut(per)).zip(0..) {
            s.spawn(move || {
                for c in 0..wc.len() {
                    let p = t * per + c;
                    if z.at(p) <= 0.0 {
                        continue;
                    }
                    let (pm, tc) = climate(p);
                    let ro = runoff_mm(pm, tc);
                    wc[c] = discharge_weight(ro) as f32;
                    rc[c] = (ro / 1000.0 / YEAR_S) as f32;
                    ec[c] = ((pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / YEAR_S) as f32;
                }
            });
        }
    });
    (weight, runoff, evap)
}

/// What the H1 stage of the plan leaves.
struct H1Stage {
    hy: Hydro,
    lakes: Lakes,
    graph: ChannelGraph,
    h1: HydroStats,
    g_con: Verify,
    sum_q: f64,
    sum_qw: f64,
}

/// The plan's H1 stage (drivers, the weighted route and its filled surface, hydrology, river pieces, the channel graph, the conservation check, the lakes) on heights read through [`Heights`]: the f32-stored
/// round surface (GEN-166) or the f64 one give the same bits.
#[allow(clippy::too_many_arguments)]
fn h1_stage<Z: Heights + ?Sized>(
    grid: &Grid,
    z: &Z,
    outlet: &[bool],
    p: &PlanParams,
    climate: Climate,
    mark: &dyn Fn(&str),
    lap: &mut dyn FnMut(&'static str),
) -> H1Stage {
    // ---- H1: drivers, the weighted route, the filled surface, the hydrology
    let (weight, runoff, evap) = drivers(z, p.threads, climate);
    lap("drivers");
    mark("drivers");
    // One flood: the weighted route's own filled surface is H1's (it was computed twice before the plan; the bits are the same).
    let (routing, filled) = route_rules_filled(grid, z, outlet, p.epsilon, p.seed, 0.0, Some(&weight), RoundRules::ROUND);
    lap("weighted route and filled surface");
    mark("routing and filled surface");
    // The conservation check's reference for the area (the weighted sum of the cell areas) is taken now, so that the weights are not regenerated later.
    let weighted_area = weighted_cell_area(grid, Some(&weight));
    drop(weight);
    mark("weight dropped");
    let mut hy = hydrology(grid, z, &filled, &routing, outlet, Some(&runoff), Some(&evap), &p.hydro);
    lap("hydrology");
    mark("hydrology");
    // The pieces of the river segments carry `Q_w` at their ends: they are cut now and are the only part of the lake producer that reads the discharges (GEN-163, D22).
    let pre = river_pieces(grid, &routing, &hy, z);
    let mut graph = ChannelGraph::from_hydro(grid, &routing, &hy, z);
    graph.set_cuts(pre.cuts.clone());
    mark("channel graph");
    lap("river pieces");
    mark("river pieces");
    // The open-water evaporation is H1's input only.
    drop(evap);
    let g_con = verify_with_area(grid, &hy, &routing, &filled, outlet, Some(&runoff), None, Some(weighted_area));
    let (sum_q, sum_qw) = (hy.q.iter().sum::<f64>(), hy.qw.iter().sum::<f64>());
    lap("conservation check");
    // The probe reads f64: f64 heights are borrowed; f32 ones are widened into a copy that exists only while a probe is installed and only for this emit (a measurement run has the probes off).
    {
        let widened: Option<Vec<f64>> = (probe::enabled() && z.as_f64().is_none()).then(|| (0..z.len()).map(|c| z.at(c)).collect());
        let zf: &[f64] = z.as_f64().or(widened.as_deref()).unwrap_or(&[]);
        probe::emit(probe::Stage::H1, |s| {
            probe::StageView::new(s, grid)
                .f64("area", probe::FieldKind::Area, &routing.area)
                .f64("z", probe::FieldKind::Height, zf)
                .number("segments", hy.stats.segments as f64)
                .number("basins", hy.stats.basins as f64)
                .number("channel cells", hy.stats.channel_cells as f64)
        });
    }
    let h1 = hy.stats.clone();
    drop((routing, filled, runoff));
    hy.q = Vec::new();
    hy.qw = Vec::new();
    mark("H1 result kept, discharges released");
    // The lake records come after the release: they read the basins only, and the working set has its headroom here (D22).
    let lakes =
        Lakes { lakes: lake_records(grid, &hy), pieces: pre.pieces, dropped_cells: pre.dropped_cells, water: pre.water, ring: pre.ring };
    lap("lakes");
    mark("lakes");
    H1Stage { hy, lakes, graph, h1, g_con, sum_q, sum_qw }
}

/// The complete resident build after the landmass: `z` is the landmass's heights on `grid` (a compact landmass), `outlet` its outlets, `climate` the climate of a cell. Returns the surface after the sediment pass and
/// H1's result. `mark` is called at every allocation boundary with its name.
pub fn build_resident(
    grid: &Grid,
    mut z: Vec<f64>,
    outlet: &[bool],
    p: &PlanParams,
    climate: Climate,
    mark: &dyn Fn(&str),
) -> std::io::Result<Built> {
    assert_eq!(z.len(), grid.len(), "one height per cell");
    assert_eq!(outlet.len(), grid.len(), "one outlet flag per cell");
    let n = z.len();
    let mut phases: Vec<(&'static str, f64)> = Vec::new();
    let mut clock = Instant::now();
    let mut lap = |name: &'static str, phases: &mut Vec<(&'static str, f64)>| {
        phases.push((name, clock.elapsed().as_secs_f64()));
        clock = Instant::now();
    };
    probe::emit(probe::Stage::Landmass, |s| {
        probe::StageView::new(s, grid)
            .f64("z", probe::FieldKind::Height, &z)
            .number("land cells", z.iter().filter(|&&h| h > 0.0).count() as f64)
    });
    mark("landmass");

    if let Some(w) = &p.precision {
        w.check_within_radius(grid, &z, |c| grid.global_id(c))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("the landmass: {e}")))?;
    }
    // ---- the pre-carve surface waits on disk
    let guard = SpillGuard(p.spill.clone());
    let spill_bytes = spill_write(&p.spill, &z)?;
    lap("spill", &mut phases);
    mark("pre-carve surface spilled");

    // ---- the rounds, resident
    let rounds = solve_rounds(grid, &mut z, outlet, &p.carve, p.rounds, &p.threads, p.seed);
    lap("rounds", &mut phases);
    mark("after the rounds");
    if let Some(w) = &p.precision {
        w.check_within_radius(grid, &z, |c| grid.global_id(c))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("after the rounds: {e}")))?;
    }

    // ---- H1 on the rounds' surface; under `round_f32` the surface is stored f32 (half the bytes, exact) from here to the sediment pass (GEN-166 PR 2)
    let st = if p.carve.round_f32 {
        assert!(z.iter().all(|&h| f64::from(h as f32) == h), "the round surface is f32-representable under round_f32");
        let z32: Vec<f32> = z.iter().map(|&h| h as f32).collect();
        drop(std::mem::take(&mut z));
        mark("heights stored f32");
        let st = h1_stage(grid, &z32[..], outlet, p, climate, mark, &mut |name| lap(name, &mut phases));
        z = z32.iter().map(|&h| f64::from(h)).collect();
        drop(z32);
        mark("heights widened");
        st
    } else {
        h1_stage(grid, &z[..], outlet, p, climate, mark, &mut |name| lap(name, &mut phases))
    };
    let H1Stage { hy, lakes, graph, h1, g_con, sum_q, sum_qw } = st;

    // ---- the sediment pass on the surface read back
    let z_start = spill_read(&p.spill, n)?;
    drop(guard);
    mark("pre-carve surface read back");
    let sediment = sediment_pass(grid, &mut z, &z_start, &p.sediment, p.threads);
    drop(z_start);
    lap("sediment pass", &mut phases);
    mark("after the sediment pass");
    Ok(Built { z, hydro: hy, lakes, graph, report: PlanReport { rounds, h1, g_con, sum_q, sum_qw, sediment, spill_bytes, phases } })
}
