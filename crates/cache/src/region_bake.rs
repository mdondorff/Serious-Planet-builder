//! The region bake (GEN-177): a rectangle of regional cells of one face to a commit with a channel graph. The pipeline of `generators/examples/k7_region.rs` as a function: the regional solve from the base, H1
//! hydrology on the solved surface with the runoff of the climate, the lakes, the channel graph, then the writer ([`bake_l`]) over the tiles the region meets. The base and the climate are closures and the sizes a
//! spec, so a test runs it on a small face with analytic inputs and the example `bake_region` on the real ones. A region within [`FACE_EDGE_CELLS`] regional cells of a face edge is refused (no channel graph
//! across a face edge: the PR 7 limit, architect 2026-11-12).

use std::path::Path;

use planet_core::cube::face_to_direction;
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::channel_graph::{channels_from_graph, ChannelGraph};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::k7::{k7_tile, K7Params, RegionField};
use planet_generators::bake::lakes::lakes_from_hydro;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::precision::{WorldPrecision, EPSILON_MAX_M};
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::tile::{tile_has_land, TileGrid, TileId};
use planet_generators::bake::Grid;

use crate::basin_table::BasinSpec;
use crate::global_store::GlobalWorld;
use crate::graph_store::GraphSpec;
use crate::l_writer::{bake_l, ChannelFeed, Commit, GraphInput, LBake, MemoryFeed};
use crate::layer_l::LTileSpec;

/// Regional cells from a face edge within which a region is refused.
pub const FACE_EDGE_CELLS: usize = 4;
/// Local cells to a regional (H1) cell.
pub const LOCAL_PER_REGIONAL: usize = 8;

/// The runoff, open-water evaporation (both m/s) and discharge weight at a direction of land.
pub type Climate<'a> = &'a (dyn Fn(Vec3) -> (f64, f64, f64) + Sync);

/// What a region bake is made of besides its base and climate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionSpec {
    pub seed: u64,
    pub face: Face,
    pub radius_m: f64,
    /// Regional (H1) cells a face edge; the local grid has [`LOCAL_PER_REGIONAL`] times as many.
    pub regional: usize,
    /// The rectangle of regional cells, inclusive: `(i0, i1, j0, j1)`.
    pub region: (usize, usize, usize, usize),
    /// Regional cells added round the region for the solve and the hydrology.
    pub margin: usize,
    /// The L tile side and halo in local cells.
    pub tile: usize,
    pub halo: usize,
    /// Rounds of the regional solve.
    pub rounds: usize,
    pub threads: usize,
    /// The resolution the keys are made with, and the identities of the base and the other inputs (the caller's).
    pub res: u32,
    pub base_id: u64,
    pub inputs_id: u64,
    /// The world's height bounds `(lo, hi)` in metres above the sea level: the base heights plus the profile's largest uplift (GEN-175). The rounds' flood epsilon, its f32 or f64 choice and the check that no height leaves the bounds come from them (`WorldPrecision`); Earth's span is `(-11_000.0, 9_000.0)`.
    pub height_bounds: (f64, f64),
    /// Keep the channels the bake burnt in the report (a test's: they are millions on a real region).
    pub keep_channels: bool,
}

impl RegionSpec {
    /// L tiles (and graph blocks, which are the L tiles: the reader files a block under its L tile's index) a face edge.
    pub fn tiles_per_edge(&self) -> u32 {
        (self.local() / self.tile.max(1)) as u32
    }

    /// The world's precision (GEN-175): the flood epsilon every stage of the bake takes (the carve's, H1's route and fill, K7's), and whether the rounds are f32.
    pub fn precision(&self) -> WorldPrecision {
        WorldPrecision::new(self.height_bounds.0, self.height_bounds.1, CarveParams::default().epsilon, EPSILON_MAX_M)
    }

    /// The identity the keys carry for this region: the caller's `inputs_id` with everything the regional solve and the hydrology depend on besides the base and the climate (the window, the rounds, the carve), so two regions or
    /// margins never share keys (the threads do not enter: the bake is the same at any count).
    pub fn effective_inputs_id(&self) -> u64 {
        let mut h = self.inputs_id ^ 0xcbf2_9ce4_8422_2325;
        let mut put = |v: u64| h = (h ^ v).wrapping_mul(0x0000_0100_0000_01b3);
        for v in [
            u64::from(self.face.0),
            self.regional as u64,
            self.region.0 as u64,
            self.region.1 as u64,
            self.region.2 as u64,
            self.region.3 as u64,
            self.margin as u64,
            self.rounds as u64,
            self.tile as u64,
            self.halo as u64,
            self.seed,
            self.radius_m.to_bits(),
            self.height_bounds.0.to_bits(),
            self.height_bounds.1.to_bits(),
        ] {
            put(v);
        }
        h
    }

    pub fn local(&self) -> usize {
        self.regional * LOCAL_PER_REGIONAL
    }

    /// The tiles of the local grid the region meets.
    pub fn tiles(&self) -> Vec<TileId> {
        let (i0, i1, j0, j1) = self.region;
        let l = LOCAL_PER_REGIONAL;
        let (ti0, ti1, tj0, tj1) = (i0 * l / self.tile, (i1 * l + l - 1) / self.tile, j0 * l / self.tile, (j1 * l + l - 1) / self.tile);
        (tj0..=tj1).flat_map(|tj| (ti0..=ti1).map(move |ti| TileId { face: self.face, ti: ti as u32, tj: tj as u32 })).collect()
    }

    /// Whether the spec can be baked (the same refusals `bake_region` makes, without the work).
    pub fn check(&self) -> Result<(), String> {
        let (i0, i1, j0, j1) = self.region;
        if i0 > i1 || j0 > j1 || self.regional == 0 {
            return Err(format!("the region {:?} of {} regional cells is empty", self.region, self.regional));
        }
        let near = |a: usize, b: usize| a < FACE_EDGE_CELLS || b + FACE_EDGE_CELLS >= self.regional;
        if near(i0, i1) || near(j0, j1) {
            return Err(format!(
                "the region {:?} of {} regional cells lies within {FACE_EDGE_CELLS} cells of a face edge: no channel graph crosses a face edge (the PR 7 limit)",
                self.region, self.regional
            ));
        }
        if i0 < self.margin || j0 < self.margin || i1 + self.margin >= self.regional || j1 + self.margin >= self.regional {
            return Err(format!("the margin of {} cells takes the window outside the face's {} cells", self.margin, self.regional));
        }
        let (lo, hi) = self.height_bounds;
        if !(lo.is_finite() && hi.is_finite() && lo <= hi) {
            return Err(format!("the world's height bounds ({lo}, {hi}) are not finite and ordered"));
        }
        let blocks = self.tiles_per_edge();
        if self.tile == 0 || !self.local().is_multiple_of(self.tile) || blocks == 0 || !blocks.is_power_of_two() {
            return Err(format!(
                "the local grid of {} cells is {} tiles of {} cells a face edge: it must be a power of two (the tiles and the graph blocks are filed under quadtree ids)",
                self.local(),
                self.local() as f64 / self.tile.max(1) as f64,
                self.tile
            ));
        }
        if !self.regional.is_multiple_of(blocks as usize) {
            return Err(format!("{blocks} blocks a face edge do not divide {} regional cells", self.regional));
        }
        // Every tile with its halo lies inside the solved window (the regional change and the base read are not extended past it).
        let (lo, hi) = (LOCAL_PER_REGIONAL * (self.region.0 - self.margin), LOCAL_PER_REGIONAL * (self.region.1 + self.margin + 1));
        let (lo_j, hi_j) = (LOCAL_PER_REGIONAL * (self.region.2 - self.margin), LOCAL_PER_REGIONAL * (self.region.3 + self.margin + 1));
        for id in self.tiles() {
            let (a, b) = (id.ti as usize * self.tile, (id.ti as usize + 1) * self.tile);
            let (c, d) = (id.tj as usize * self.tile, (id.tj as usize + 1) * self.tile);
            if a < lo + self.halo || b + self.halo > hi || c < lo_j + self.halo || d + self.halo > hi_j {
                return Err(format!(
                    "tile ({}, {}) with its halo of {} cells reaches outside the solved window (margin {} regional cells): a larger margin",
                    id.ti, id.tj, self.halo, self.margin
                ));
            }
        }
        Ok(())
    }
}

/// What the bake found beside the commit (for the log).
#[derive(Clone, Debug, PartialEq)]
pub struct RegionReport {
    pub window: (usize, usize),
    pub channel_cells: usize,
    pub segments: usize,
    pub lakes: usize,
    pub tiles: usize,
    /// The world's precision of the bake (GEN-175), as the MARK line records it.
    pub precision_mark: String,
    /// The channels burnt, when the spec asked to keep them.
    pub channels: Vec<planet_generators::bake::k7::Channel>,
}

/// The regional solve of a region (GEN-175): the base and the solved surface on the window's regional cells, with the world's precision applied.
#[derive(Clone, Debug)]
pub struct RegionSolve {
    pub face_spec: FaceSpec,
    pub grid: Grid,
    pub outlet: Vec<bool>,
    pub zb: Vec<f64>,
    pub zs: Vec<f64>,
    pub stream: u64,
    pub ri0: usize,
    pub rj0: usize,
    pub rw: usize,
    pub rh: usize,
    pub precision: WorldPrecision,
}

/// Solves the regional surface of the region: the first stage of [`bake_region`], public so that a test (or a tool) can run it without the rest.
pub fn solve_region(spec: &RegionSpec, base: &(dyn Fn(Vec3) -> f64 + Sync)) -> Result<RegionSolve, String> {
    spec.check()?;
    let (i0, i1, j0, j1) = spec.region;
    let (ri0, rj0) = (i0 - spec.margin, j0 - spec.margin);
    let (rw, rh) = (i1 + spec.margin - ri0 + 1, j1 + spec.margin - rj0 + 1);
    let n = rw * rh;
    let face_spec = FaceSpec { face: spec.face, cells: spec.regional, radius_m: spec.radius_m, sea_level: 0.0, seed: spec.seed, halo: 0 };
    let dir_of = |c: usize| face_spec.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);
    let precision = spec.precision();

    // The regional solve from B read at the regional cells' centres, under the world's precision (GEN-175): its epsilon and f32 choice, and no height outside its bounds before or after the rounds.
    let zb: Vec<f64> = (0..n).map(|c| base(dir_of(c))).collect();
    let grid = Grid::new(rw, rh, face_spec.dx()).with_origin((ri0 as u32, rj0 as u32)).with_metric(Metric::cube_face(
        spec.regional,
        spec.radius_m,
        (ri0 as i64, rj0 as i64),
        rw,
        rh,
    ));
    precision.check_within_radius(&grid, &zb, |c| grid.global_id(c)).map_err(|e| format!("the base of the region: {e}"))?;
    let outlet = grid.outlets(&zb, 0.0);
    let mut carve = CarveParams {
        steps: 5 * spec.rounds,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    precision.apply(&mut carve);
    let stream = face_spec.stream();
    let mut zs = zb.clone();
    // The rounds' own narrowing guard asserts when a round lifts a summit past the binade the epsilon was chosen for: that is the same refusal as the check after them, returned as an error naming the cell.
    let solved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        solve_rounds(&grid, &mut zs, &outlet, &carve, spec.rounds, &spec.threads, stream)
    }));
    if let Err(panic) = solved {
        let why =
            panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string())).unwrap_or_default();
        return Err(format!("after the rounds of the region: the rounds refused the surface (GEN-175): {why}"));
    }
    precision.check_within_radius(&grid, &zs, |c| grid.global_id(c)).map_err(|e| format!("after the rounds of the region: {e}"))?;
    Ok(RegionSolve { face_spec, grid, outlet, zb, zs, stream, ri0, rj0, rw, rh, precision })
}

/// The front half of the bake, which the tile layout does not enter: the regional solve, H1 on the solved surface, the lakes, the channel graph and its channels.
pub struct RegionFront {
    pub solve: RegionSolve,
    pub regional_field: RegionField,
    pub hydro_stats: planet_generators::bake::hydro::HydroStats,
    pub lakes: planet_generators::bake::lakes::Lakes,
    pub graph: ChannelGraph,
    pub segment_ids: Vec<u64>,
    pub feed: MemoryFeed,
}

/// Runs the stages of [`bake_region`] before the writer: the same code for the bake and for the layout report (BUILD-052).
pub fn region_front(spec: &RegionSpec, base: &(dyn Fn(Vec3) -> f64 + Sync), climate: Climate<'_>) -> Result<RegionFront, String> {
    let solve = solve_region(spec, base)?;
    let RegionSolve { face_spec, grid, outlet, zb, zs, stream, ri0, rj0, rw, rh, precision } = &solve;
    let (ri0, rj0, rw, rh, stream) = (*ri0, *rj0, *rw, *rh, *stream);
    let n = rw * rh;
    let dir_of = |c: usize| face_spec.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);
    let change: Vec<f32> = zs.iter().zip(zb).map(|(s, b)| (s - b) as f32).collect();
    let regional_field = RegionField::new(spec.face, spec.regional, (ri0 as i64, rj0 as i64), rw, rh, change);
    let (mut weight, mut runoff, mut evap) = (vec![1.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
    for c in (0..n).filter(|&c| zs[c] > 0.0) {
        let (ro, ev, w) = climate(dir_of(c));
        runoff[c] = ro as f32;
        evap[c] = ev as f32;
        weight[c] = w as f32;
    }
    let routing = route_rules(grid, zs, outlet, precision.epsilon_w, stream, 0.0, Some(&weight), RoundRules::ROUND);
    let filled = priority_flood_with(grid, zs, outlet, precision.epsilon_w, true);
    let hydro = hydrology(grid, zs, &filled, &routing, outlet, Some(&runoff), Some(&evap), &HydroParams::default());
    let lakes = lakes_from_hydro(grid, &routing, &hydro, zs);
    let graph = ChannelGraph::from_hydro(grid, &routing, &hydro, zs);
    let segment_ids: Vec<u64> = hydro.segments.iter().map(|s| s.id).collect();
    let to_local = |c: u32| {
        let c = c as usize;
        (
            (LOCAL_PER_REGIONAL * (ri0 + c % rw)) as f64 + (LOCAL_PER_REGIONAL as f64 - 1.0) / 2.0,
            (LOCAL_PER_REGIONAL * (rj0 + c / rw)) as f64 + (LOCAL_PER_REGIONAL as f64 - 1.0) / 2.0,
        )
    };
    let feed = MemoryFeed { face: spec.face, channels: channels_from_graph(&graph, grid, &to_local), discharge: true };
    let _ = rh;
    Ok(RegionFront { solve, regional_field, hydro_stats: hydro.stats.clone(), lakes, graph, segment_ids, feed })
}

/// Bakes the region into `dir` as a commit with a channel graph. `base` is the base height (the bake's `B`), `climate` the runoff of the land.
pub fn bake_region(
    world: &GlobalWorld,
    spec: &RegionSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    climate: Climate<'_>,
    dir: &Path,
) -> Result<(Commit, RegionReport), String> {
    spec.check()?;
    // The K7 of the commit is the standard one: a world whose flood epsilon is not the profile's cannot be committed yet (the commit spec records no epsilon), and it is refused before any work is done.
    let precision = spec.precision();
    let k7_epsilon = K7Params::standard(spec.seed, 0.0).pass2.carve.epsilon;
    if precision.epsilon_w != k7_epsilon {
        return Err(format!(
            "the world's height radius is R = {} m, which gives it the flood epsilon {} m, not the K7 epsilon {} m the commit spec describes (GEN-175): the commit spec records no flood epsilon, so a commit of such a world could not be read back with the keys it was made under: a format question for the owner (CON-10, decided with the ADR 0117 schema)",
            precision.radius, precision.epsilon_w, k7_epsilon
        ));
    }
    let RegionFront { solve, regional_field, hydro_stats, lakes, graph, segment_ids, feed } = region_front(spec, base, climate)?;
    let RegionSolve { grid, ri0, rj0, rw, rh, precision, .. } = solve;
    let regional = |d: Vec3| regional_field.at(d);

    let local = spec.local();
    let basin = BasinSpec {
        hydro: HydroParams::default(),
        tile_cells: spec.tile as u32,
        tiles_per_edge: spec.tiles_per_edge(),
        base_id: spec.base_id,
        inputs_id: spec.effective_inputs_id(),
    };
    let l_spec = LTileSpec {
        grid: TileGrid { cells: local, tile: spec.tile, halo: spec.halo, radius_m: spec.radius_m },
        k7: K7Params::standard(spec.seed, 0.0),
        base_id: spec.base_id,
        inputs_id: spec.effective_inputs_id(),
    };
    // The H1 cells the graph reaches beyond its blocks: the tile halo in H1 cells, and one more (the writer refuses less: GEN-169).
    let mut graph_spec = GraphSpec { basin, h1_cells: spec.regional as u32, margin_h1: 0 };
    graph_spec.margin_h1 = graph_spec.required_margin(spec.halo as u32);
    let face = spec.face;
    let cell_xy = |c: u32| (face.0, (ri0 + c as usize % rw) as i64, (rj0 + c as usize / rw) as i64);
    let h1_cell = |f: u8, i: i64, j: i64| {
        let (i, j) = (i - ri0 as i64, j - rj0 as i64);
        (f == face.0 && i >= 0 && j >= 0 && (i as usize) < rw && (j as usize) < rh).then(|| j as usize * rw + i as usize)
    };
    // The tiles with land: sea-only tiles are not baked (K7 on one is minutes for nothing).
    let tile_grid = l_spec.grid;
    let surface = |d: Vec3| base(d) + regional(d);
    let tiles: Vec<TileId> = spec.tiles().into_iter().filter(|&id| tile_has_land(&tile_grid, id, &surface, 0.0, 8)).collect();
    if tiles.is_empty() {
        return Err("the region holds no land tile".into());
    }
    let b = LBake {
        world,
        res: spec.res,
        spec: l_spec,
        basin,
        base,
        regional: &regional,
        feed: &feed,
        lakes: &lakes,
        h1_cells: spec.regional,
        h1_cell: &h1_cell,
        face_of: &|_| face,
        threads: spec.threads,
        graph: Some(GraphInput { spec: graph_spec, graph: &graph, segment_ids: &segment_ids, grid: &grid, cell_xy: &cell_xy }),
    };
    let commit = bake_l(dir, &b, &tiles)?;
    let report = RegionReport {
        window: (rw, rh),
        channel_cells: hydro_stats.channel_cells,
        segments: hydro_stats.segments,
        lakes: lakes.lakes.len(),
        tiles: tiles.len(),
        precision_mark: precision.mark_line(),
        channels: if spec.keep_channels { feed.channels.clone() } else { Vec::new() },
    };
    Ok((commit, report))
}

/// One row of the layout report (BUILD-052): the cells of a set, and the difference of K7's height between the two layouts over them, in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutRow {
    pub set: &'static str,
    pub band: &'static str,
    pub cells: usize,
    pub differ: usize,
    pub p50: f64,
    pub p99: f64,
    pub max: f64,
    pub over_025: usize,
    pub over_1: usize,
}

impl LayoutRow {
    /// The row as one report line.
    pub fn line(&self) -> String {
        let pct = if self.cells == 0 { 0.0 } else { 100.0 * self.differ as f64 / self.cells as f64 };
        format!(
            "LAYOUT | {} | {} | cells {} | differ in bits {} ({pct:.2} %) | over 0.25 m {} | over 1 m {} | p50 {:.4} p99 {:.4} max {:.4} m",
            self.set, self.band, self.cells, self.differ, self.over_025, self.over_1, self.p50, self.p99, self.max
        )
    }
}

/// The difference between two tile layouts of the same region (BUILD-052).
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutDiff {
    /// `(tile, halo)` of the canonical layout and of the alternative.
    pub layouts: [(usize, usize); 2],
    /// Land tiles baked in each layout.
    pub tiles: [usize; 2],
    pub rows: Vec<LayoutRow>,
}

/// Distance bands from a seam, in cells: `[lo, hi)`.
const SEAM_BANDS: [(&str, usize, usize); 4] = [("0-7", 0, 8), ("8-31", 8, 32), ("32-127", 32, 128), ("128+", 128, usize::MAX)];

fn percentile(sorted: &[f64], p: usize) -> f64 {
    if sorted.is_empty() {
        0.0
    } else {
        sorted[(p * sorted.len()).div_ceil(100).saturating_sub(1).min(sorted.len() - 1)]
    }
}

fn row(set: &'static str, band: &'static str, a: &[f64], b: &[f64], pick: impl Fn(usize) -> bool) -> LayoutRow {
    let mut d: Vec<f64> = Vec::new();
    let mut differ = 0;
    for k in (0..a.len()).filter(|&k| pick(k)) {
        differ += usize::from(a[k].to_bits() != b[k].to_bits());
        d.push((a[k] - b[k]).abs());
    }
    d.sort_by(f64::total_cmp);
    LayoutRow {
        set,
        band,
        cells: d.len(),
        differ,
        p50: percentile(&d, 50),
        p99: percentile(&d, 99),
        max: d.last().copied().unwrap_or(0.0),
        over_025: d.iter().filter(|&&x| x > 0.25).count(),
        over_1: d.iter().filter(|&&x| x > 1.0).count(),
    }
}

/// K7's heights of the cells of the region in one layout (unquantised `z`, row-major over the region's local rectangle; NaN where no land tile of the layout covers a cell), and the land tiles baked.
fn k7_layout(spec: &RegionSpec, front: &RegionFront, base: &(dyn Fn(Vec3) -> f64 + Sync)) -> Result<(Vec<f64>, usize), String> {
    let l = LOCAL_PER_REGIONAL;
    let (x0, x1, y0, y1) = (spec.region.0 * l, (spec.region.1 + 1) * l, spec.region.2 * l, (spec.region.3 + 1) * l);
    let w = x1 - x0;
    let mut out = vec![f64::NAN; w * (y1 - y0)];
    let grid = TileGrid { cells: spec.local(), tile: spec.tile, halo: spec.halo, radius_m: spec.radius_m };
    let regional = |d: Vec3| front.regional_field.at(d);
    let surface = |d: Vec3| base(d) + regional(d);
    let k7 = K7Params::standard(spec.seed, 0.0);
    let ids: Vec<TileId> = spec.tiles().into_iter().filter(|&id| tile_has_land(&grid, id, &surface, 0.0, 8)).collect();
    for &id in &ids {
        let (t, h, wn) = (spec.tile as i64, spec.halo as i64, grid.window());
        let (i0, j0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
        let channels = front.feed.channels_near(id.face, i0, j0, wn as i64);
        let r = k7_tile(&grid, id, base, &regional, &channels, &k7, spec.threads);
        for cj in 0..spec.tile {
            for ci in 0..spec.tile {
                let (gi, gj) = (id.ti as usize * spec.tile + ci, id.tj as usize * spec.tile + cj);
                if (x0..x1).contains(&gi) && (y0..y1).contains(&gj) {
                    out[(gj - y0) * w + (gi - x0)] = r.z[(cj + spec.halo) * wn + ci + spec.halo];
                }
            }
        }
    }
    Ok((out, ids.len()))
}

/// The difference in K7's height between the layout of `spec` and the layout `alt = (tile, halo)` of the same region, over the cells both layouts bake (BUILD-052). The heights are K7's own (unquantised); the seam
/// is the tile boundaries only one of the layouts has (all boundaries when they are the same lines).
pub fn layout_difference(
    spec: &RegionSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    climate: Climate<'_>,
    alt: (usize, usize),
) -> Result<LayoutDiff, String> {
    let other = RegionSpec { tile: alt.0, halo: alt.1, ..*spec };
    other.check()?;
    let front = region_front(spec, base, climate)?;
    let (za, na) = k7_layout(spec, &front, base)?;
    let (zb, nb) = k7_layout(&other, &front, base)?;
    let l = LOCAL_PER_REGIONAL;
    let (x0, x1, y0, y1) = (spec.region.0 * l, (spec.region.1 + 1) * l, spec.region.2 * l, (spec.region.3 + 1) * l);
    let w = x1 - x0;
    // The cells both layouts bake, as parallel lists.
    let common: Vec<usize> = (0..za.len()).filter(|&k| !za[k].is_nan() && !zb[k].is_nan()).collect();
    let (a, b): (Vec<f64>, Vec<f64>) = common.iter().map(|&k| (za[k], zb[k])).unzip();
    // The seam lines (boundary `k` lies between cells `k - 1` and `k`), per axis: those of one layout only, else all.
    let lines = |lo: usize, hi: usize| -> Vec<usize> {
        let (ta, tb) = (spec.tile, other.tile);
        let mut only: Vec<usize> = (lo..=hi).filter(|&k| (k % ta == 0) != (k % tb == 0)).collect();
        if only.is_empty() {
            only = (lo..=hi).filter(|&k| k % ta == 0).collect();
        }
        only
    };
    let (sx, sy) = (lines(x0, x1), lines(y0, y1));
    let near = |v: usize, ls: &[usize]| ls.iter().map(|&k| if v >= k { v - k } else { k - 1 - v }).min().unwrap_or(usize::MAX);
    let dist: Vec<usize> = common.iter().map(|&k| near(x0 + k % w, &sx).min(near(y0 + k / w, &sy))).collect();
    let line: std::collections::HashSet<(i64, i64)> =
        front.feed.channels.iter().flat_map(planet_generators::bake::k7::line_cells).collect();
    let is_line: Vec<bool> = common.iter().map(|&k| line.contains(&((x0 + k % w) as i64, (y0 + k / w) as i64))).collect();
    let mut rows = vec![
        row("all cells", "whole", &a, &b, |_| true),
        row("line cells", "whole", &a, &b, |k| is_line[k]),
        row("non-line cells", "whole", &a, &b, |k| !is_line[k]),
    ];
    for (name, lo, hi) in SEAM_BANDS {
        rows.push(row("all cells", name, &a, &b, |k| (lo..hi).contains(&dist[k])));
    }
    Ok(LayoutDiff { layouts: [(spec.tile, spec.halo), alt], tiles: [na, nb], rows })
}

/// The direction of a lattice vertex `(i, j)` of a face of `nodes` nodes an edge (the store lattice is vertex-based: vertex `k` lies at `2k/nodes - 1`).
pub fn lattice_direction(face: Face, nodes: usize, i: usize, j: usize) -> Vec3 {
    face_to_direction(&TangentWarp, face, 2.0 * i as f64 / nodes as f64 - 1.0, 2.0 * j as f64 / nodes as f64 - 1.0)
}

const FIXTURE_RADIUS_M: f64 = 6_371_000.0;

/// The region of the fixture face of BUILD-051 and BUILD-052: 24 by 24 regional cells of face 2, tiles of 64 cells with a halo of 24.
fn fixture_spec(threads: usize) -> RegionSpec {
    RegionSpec {
        seed: 3,
        face: Face(2),
        radius_m: FIXTURE_RADIUS_M,
        regional: 64,
        region: (16, 39, 16, 39),
        margin: 6,
        tile: 64,
        halo: 24,
        rounds: 2,
        threads,
        res: 64,
        base_id: 7,
        inputs_id: 11,
        height_bounds: (-11_000.0, 9_000.0),
        keep_channels: false,
    }
}

/// The layout report of the fixture (BUILD-052): the fixture's layout (tile 64, halo 24) against the alternative of 128-cell tiles with a halo of 24 (a margin of 12 regional cells holds the larger tiles; K7's halo is at least 24 cells, GEN-169), at `threads` threads.
pub fn layout_fixture(threads: usize) -> Result<LayoutDiff, String> {
    let base = |d: Vec3| 500.0 - 2400.0 * (d.x - 0.1) + 120.0 * libm::sin(40.0 * d.z);
    let climate = |_: Vec3| (1.0e-8, 0.0, 1.0);
    layout_difference(&RegionSpec { margin: 12, ..fixture_spec(threads) }, &base, &climate, (128, 24))
}

/// The determinism fixture of the cross-OS manifest (BUILD-051): a small analytic face (a ridge that falls to the sea, constant runoff) baked
/// to a commit with a channel graph at `threads` threads, and a 64-bit hash of every file the bake wrote (path and bytes, in path order).
/// The manifest lines of one process hold the hash at one thread twice and at eight threads: a thread count or a repeat that changes a byte
/// changes a line, and a platform whose float results differ changes the same line on one operating system against the other.
pub fn determinism_fixture(threads: usize) -> Result<u64, String> {
    use planet_world_def::WorldDefinition;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let world = GlobalWorld::from_definition(&WorldDefinition::single_planet(FIXTURE_RADIUS_M, 3))
        .map_err(|e| format!("the fixture world: {e:?}"))?
        .with_block_limit(4);
    let spec = fixture_spec(threads);
    let base = |d: Vec3| 500.0 - 2400.0 * (d.x - 0.1) + 120.0 * libm::sin(40.0 * d.z);
    let climate = |_: Vec3| (1.0e-8, 0.0, 1.0);
    let dir = std::env::temp_dir().join(format!("planet-det-fixture-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    let baked = bake_region(&world, &spec, &base, &climate, &dir);
    let hashed = baked.and_then(|_| hash_tree(&dir));
    let _ = std::fs::remove_dir_all(&dir);
    hashed
}

/// A 64-bit fold of every file under `dir` (relative path, length and bytes in 8-byte little-endian words, in path order).
fn hash_tree(dir: &Path) -> Result<u64, String> {
    use planet_core::hash::mix64;
    fn walk(at: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(at)
            .map_err(|e| format!("{}: {e}", at.display()))?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out)?;
            } else {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, &mut files)?;
    let mut h = mix64(files.len() as u64);
    for f in files {
        let rel = f.strip_prefix(dir).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        for chunk in rel.as_bytes().chunks(8) {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            h = mix64(h ^ u64::from_le_bytes(w));
        }
        let bytes = std::fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        h = mix64(h ^ bytes.len() as u64);
        for chunk in bytes.chunks(8) {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            h = mix64(h ^ u64::from_le_bytes(w));
        }
    }
    Ok(h)
}
