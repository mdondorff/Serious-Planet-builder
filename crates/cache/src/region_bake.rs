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
use planet_generators::bake::k7::{K7Params, RegionField};
use planet_generators::bake::lakes::lakes_from_hydro;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::tile::{TileGrid, TileId};
use planet_generators::bake::Grid;

use crate::basin_table::BasinSpec;
use crate::global_store::GlobalWorld;
use crate::graph_store::GraphSpec;
use crate::l_writer::{bake_l, Commit, GraphInput, LBake, MemoryFeed};
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
    /// Blocks of the graph and basin tables a face edge (a power of two that divides `regional`).
    pub blocks_per_edge: u32,
    /// Rounds of the regional solve.
    pub rounds: usize,
    pub threads: usize,
    /// The resolution the keys are made with, and the identities of the base and the other inputs (the caller's).
    pub res: u32,
    pub base_id: u64,
    pub inputs_id: u64,
}

impl RegionSpec {
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

    fn check(&self) -> Result<(), String> {
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
        if self.blocks_per_edge == 0
            || !self.blocks_per_edge.is_power_of_two()
            || !self.regional.is_multiple_of(self.blocks_per_edge as usize)
        {
            return Err(format!(
                "{} blocks a face edge are not a power of two that divides {} regional cells",
                self.blocks_per_edge, self.regional
            ));
        }
        if !self.local().is_multiple_of(self.blocks_per_edge as usize) || !self.local().is_multiple_of(self.tile) {
            return Err(format!("the local grid of {} cells is not a whole number of blocks and of tiles of {}", self.local(), self.tile));
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
    let (i0, i1, j0, j1) = spec.region;
    let (ri0, rj0) = (i0 - spec.margin, j0 - spec.margin);
    let (rw, rh) = (i1 + spec.margin - ri0 + 1, j1 + spec.margin - rj0 + 1);
    let n = rw * rh;
    let face_spec = FaceSpec { face: spec.face, cells: spec.regional, radius_m: spec.radius_m, sea_level: 0.0, seed: spec.seed, halo: 0 };
    let dir_of = |c: usize| face_spec.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);

    // The regional solve from B read at the regional cells' centres.
    let zb: Vec<f64> = scoped_map(n, spec.threads, |c| base(dir_of(c)));
    let grid = Grid::new(rw, rh, face_spec.dx()).with_origin((ri0 as u32, rj0 as u32)).with_metric(Metric::cube_face(
        spec.regional,
        spec.radius_m,
        (ri0 as i64, rj0 as i64),
        rw,
        rh,
    ));
    let outlet = grid.outlets(&zb, 0.0);
    let carve = CarveParams {
        steps: 5 * spec.rounds,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        round_f32: true,
        pre_breach: Some(planet_generators::bake::breach::PreBreach::PROFILE),
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let stream = face_spec.stream();
    let mut zs = zb.clone();
    solve_rounds(&grid, &mut zs, &outlet, &carve, spec.rounds, &spec.threads, stream);
    let change: Vec<f32> = zs.iter().zip(&zb).map(|(s, b)| (s - b) as f32).collect();
    let regional_field = RegionField::new(spec.face, spec.regional, (ri0 as i64, rj0 as i64), rw, rh, change);
    let regional = |d: Vec3| regional_field.at(d);

    // H1 on the solved surface: the runoff in m/s (without it Q is an area), the weight of the discharge.
    let (mut weight, mut runoff, mut evap) = (vec![1.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
    for c in (0..n).filter(|&c| zs[c] > 0.0) {
        let (ro, ev, w) = climate(dir_of(c));
        runoff[c] = ro as f32;
        evap[c] = ev as f32;
        weight[c] = w as f32;
    }
    let routing = route_rules(&grid, &zs, &outlet, 1e-3, stream, 0.0, Some(&weight), RoundRules::ROUND);
    let filled = priority_flood_with(&grid, &zs, &outlet, 1e-3, true);
    let hydro = hydrology(&grid, &zs, &filled, &routing, &outlet, Some(&runoff), Some(&evap), &HydroParams::default());
    let lakes = lakes_from_hydro(&grid, &routing, &hydro, &zs);
    let graph = ChannelGraph::from_hydro(&grid, &routing, &hydro, &zs);
    let segment_ids: Vec<u64> = hydro.segments.iter().map(|s| s.id).collect();
    let to_local = |c: u32| {
        let c = c as usize;
        (
            (LOCAL_PER_REGIONAL * (ri0 + c % rw)) as f64 + (LOCAL_PER_REGIONAL as f64 - 1.0) / 2.0,
            (LOCAL_PER_REGIONAL * (rj0 + c / rw)) as f64 + (LOCAL_PER_REGIONAL as f64 - 1.0) / 2.0,
        )
    };
    let feed = MemoryFeed { face: spec.face, channels: channels_from_graph(&graph, &grid, &to_local), discharge: true };

    let local = spec.local();
    let basin = BasinSpec {
        hydro: HydroParams::default(),
        tile_cells: (local / spec.blocks_per_edge as usize) as u32,
        tiles_per_edge: spec.blocks_per_edge,
        base_id: spec.base_id,
        inputs_id: spec.inputs_id,
    };
    let l_spec = LTileSpec {
        grid: TileGrid { cells: local, tile: spec.tile, halo: spec.halo, radius_m: spec.radius_m },
        k7: K7Params::standard(spec.seed, 0.0),
        base_id: spec.base_id,
        inputs_id: spec.inputs_id,
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
    let tiles: Vec<TileId> = spec.tiles();
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
        channel_cells: hydro.stats.channel_cells,
        segments: hydro.stats.segments,
        lakes: lakes.lakes.len(),
        tiles: tiles.len(),
    };
    Ok((commit, report))
}

/// `f(c)` for every `c` in `0..n`, over scoped threads in contiguous chunks (a pure function of `c`: the result is the same at any thread count).
fn scoped_map<T: Send + Default + Clone>(n: usize, threads: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let mut out = vec![T::default(); n];
    let chunk = n.div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (k, part) in out.chunks_mut(chunk).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (c, v) in part.iter_mut().enumerate() {
                    *v = f(k * chunk + c);
                }
            });
        }
    });
    out
}

/// The direction of a lattice vertex `(i, j)` of a face of `nodes` nodes an edge (the store lattice is vertex-based: vertex `k` lies at `2k/nodes - 1`).
pub fn lattice_direction(face: Face, nodes: usize, i: usize, j: usize) -> Vec3 {
    face_to_direction(&TangentWarp, face, 2.0 * i as f64 / nodes as f64 - 1.0, 2.0 * j as f64 / nodes as f64 - 1.0)
}
