//! The post-pass of the erosion layer's fill depth (GEN-121, STRM-077; m9 keep-pits-seams PR 2): the planet-wide spill level of the final surface, tile by tile, written as kind 17.
//!
//! Pass 2 leaves each tile's final core heights in a scratch store (kind 18, f32). The post-pass reads only those:
//! 1. stage 1, per tile: the labelled flood of the core ([`planet_generators::bake::spill::flood_labelled`], the cells at or below the sea level are outlets) gives the territories' edges and, for each
//!    border cell, its seed and its height;
//! 2. the graph: the tiles' edges and every pair of neighbouring border cells of two tiles of the run ([`planet_generators::bake::fillgeom::border_neighbours`]: a tile edge, a cube edge, a
//!    corner), `w = max(z, z)`; a border cell whose neighbour lies in a tile that is not in the run drains to the sea there (at its own height); the planet-wide level of every seed is solved;
//! 3. stage 3, per tile: the flood again (deterministic, so nothing but the heights is kept between the stages), the level of every cell `max(level, G(label))`, and the depth
//!    `level - z` as 16-bit codes ([`planet_generators::bake::face::fill_code`]) into the tile store.
//!
//! A tile that holds a fill record under the current key is not written again. A run over a region treats the tiles it does not hold as sea: the region must include whole basins (its
//! margin is the caller's choice) or the basins at its border are filled only up to the region's edge.

use std::path::Path;

use planet_core::par::{self, Par};
use planet_generators::bake::face::fill_code;
use planet_generators::bake::fillgeom::border_neighbours;
use planet_generators::bake::spill::{
    final_level, flood_labelled, perimeter_cell, perimeter_index, perimeter_len, SpillGraph, TileFlood, SEA,
};
use planet_generators::bake::tile::TileId as PassTile;

use crate::global_store::GlobalWorld;
use crate::layer_fill::{fill_record, has_fill, load_heights};
use crate::layer_store::LayerStoreError;
use crate::layer_tiles::TileLayerSpec;
use crate::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};

/// What a run did.
#[derive(Clone, Debug, PartialEq)]
pub struct FillPassReport {
    /// Tiles whose fill record was written.
    pub written: Vec<PassTile>,
    /// Tiles that already held one.
    pub skipped: Vec<PassTile>,
    /// Core cells with a depth above 0 in the written tiles.
    pub pit_cells: u64,
    /// The deepest depth written, metres.
    pub deepest_m: f64,
    /// Cells whose level no path from the sea reaches (a run without any outlet in reach): their depth is written as 0.
    pub unreached_cells: u64,
}

/// A tile's stage-3 result: its index, the depth codes, the pit cells, the deepest depth and the cells no path reaches.
type TileDepth = (usize, Vec<u16>, u64, f64, u64);

/// What stage 1 keeps of a tile.
struct Info {
    edges: Vec<(u32, u32, f64)>,
    /// The height of each border cell, along the perimeter.
    border_z: Vec<f64>,
    /// The seed of each border cell ([`SEA`] for a cell that drains to an outlet of its tile).
    border_seed: Vec<u32>,
}

fn flood(heights: &[f32], t: usize, sea_level: f64) -> (Vec<f64>, TileFlood) {
    let z: Vec<f64> = heights.iter().map(|h| f64::from(*h) + 0.0).collect();
    let outlet: Vec<bool> = z.iter().map(|v| *v <= sea_level).collect();
    let f = flood_labelled(t, t, &z, &outlet);
    (z, f)
}

fn heights_of(scratch: &TileStore, world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<Vec<f32>, LayerStoreError> {
    load_heights(scratch, world, res, spec, id)?.ok_or_else(|| {
        LayerStoreError(format!(
            "the scratch store holds no final heights of tile {}/{}/{}: pass 2 was not run with a scratch store for it",
            id.face.0, id.ti, id.tj
        ))
    })
}

/// Runs the post-pass over `tiles` (every tile of the layer, or a region with a margin): the final heights come from the scratch store in `scratch_dir`, the fill depth is written to the tile
/// store in `tile_dir`. `threads` tiles are flooded at a time (about 400 MB each at 3,072 cells a side).
#[allow(clippy::too_many_arguments)]
pub fn run_fill_pass(
    tile_dir: &Path,
    scratch_dir: &Path,
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    tiles: &[PassTile],
    threads: usize,
    on_tile: &mut dyn FnMut(PassTile, u64),
) -> Result<FillPassReport, LayerStoreError> {
    spec.grid.check().map_err(LayerStoreError)?;
    let err = |e: crate::pack::PackError| LayerStoreError(format!("the layer store {}: {e}", tile_dir.display()));
    let scratch = TileStore::open(scratch_dir, world.params())
        .map_err(|e| LayerStoreError(format!("the scratch store {}: {e}", scratch_dir.display())))?;
    let store = TileStore::open(tile_dir, world.params()).map_err(err)?;
    let mut writer = PackWriter::open(tile_dir, world.params(), DEFAULT_MAX_PACK_BYTES).map_err(err)?;
    let mut order: Vec<PassTile> = tiles.to_vec();
    order.sort();
    order.dedup();
    let (t, g, sea) = (spec.grid.tile, spec.grid, spec.params.sea_level);
    let par: &dyn Par = &threads.max(1);
    let index: std::collections::HashMap<PassTile, usize> = order.iter().enumerate().map(|(k, id)| (*id, k)).collect();
    // Stage 1.
    let infos: Vec<Result<Info, LayerStoreError>> = par::map(par, order.len(), &|k| {
        let heights = heights_of(&scratch, world, res, spec, order[k])?;
        let (z, f) = flood(&heights, t, sea);
        let (mut border_z, mut border_seed) = (Vec::with_capacity(perimeter_len(t, t)), Vec::with_capacity(perimeter_len(t, t)));
        for s in 0..perimeter_len(t, t) as u32 {
            let (i, j) = perimeter_cell(t, t, s);
            border_z.push(z[j * t + i]);
            border_seed.push(f.label[j * t + i]);
        }
        Ok(Info { edges: f.edges, border_z, border_seed })
    });
    let infos: Vec<Info> = infos.into_iter().collect::<Result<_, _>>()?;
    // The graph.
    let mut graph = SpillGraph::new();
    let refs: Vec<_> = infos.iter().map(|i| graph.add_tile(perimeter_len(t, t), &i.edges)).collect();
    for (a, id) in order.iter().enumerate() {
        for ka in 0..perimeter_len(t, t) as u32 {
            let (seed_a, za) = (infos[a].border_seed[ka as usize], infos[a].border_z[ka as usize]);
            let (i, j) = perimeter_cell(t, t, ka);
            for (other, bi, bj) in border_neighbours(&g, *id, i, j) {
                match index.get(&other) {
                    Some(&b) => {
                        let Some(kb) = perimeter_index(t, t, bi, bj) else { continue };
                        // Inside a face the relation is symmetric: one edge a pair. Across a cube edge it can be one-sided: every pair is joined.
                        if id.face == other.face && (*id, ka) > (other, kb) {
                            continue;
                        }
                        let (seed_b, zb) = (infos[b].border_seed[kb as usize], infos[b].border_z[kb as usize]);
                        if seed_a == SEA && seed_b == SEA {
                            continue;
                        }
                        graph.connect(refs[a], seed_a, refs[b], seed_b, za.max(zb));
                    }
                    None => {
                        if seed_a != SEA {
                            graph.connect(refs[a], seed_a, refs[a], SEA, za);
                        }
                    }
                }
            }
        }
    }
    drop(infos);
    let levels = graph.solve();
    drop(graph);
    // Stage 3.
    let mut report = FillPassReport { written: Vec::new(), skipped: Vec::new(), pit_cells: 0, deepest_m: 0.0, unreached_cells: 0 };
    let mut todo: Vec<usize> = Vec::new();
    for (k, id) in order.iter().enumerate() {
        if has_fill(&store, world, res, spec, *id)? {
            report.skipped.push(*id);
        } else {
            todo.push(k);
        }
    }
    for batch in todo.chunks(threads.max(1)) {
        let done: Vec<Result<TileDepth, LayerStoreError>> = par::map(par, batch.len(), &|n| {
            let k = batch[n];
            let heights = heights_of(&scratch, world, res, spec, order[k])?;
            let (z, f) = flood(&heights, t, sea);
            let level = final_level(&f, &|s| levels.seed(refs[k], s));
            let (mut pits, mut deepest, mut unreached) = (0u64, 0.0f64, 0u64);
            let codes: Vec<u16> = level
                .iter()
                .zip(&z)
                .map(|(l, h)| {
                    if !l.is_finite() {
                        unreached += 1;
                        return 0;
                    }
                    let depth = (l - h).max(0.0);
                    let c = fill_code(depth);
                    if c > 0 {
                        pits += 1;
                        deepest = deepest.max(depth);
                    }
                    c
                })
                .collect();
            Ok((k, codes, pits, deepest, unreached))
        });
        for r in done {
            let (k, codes, pits, deepest, unreached) = r?;
            writer.append(&fill_record(world, res, spec, order[k], &codes)?).map_err(err)?;
            writer.write_index().map_err(err)?;
            report.written.push(order[k]);
            report.pit_cells += pits;
            report.deepest_m = report.deepest_m.max(deepest);
            report.unreached_cells += unreached;
            on_tile(order[k], pits);
        }
    }
    writer.finish().map_err(err)?;
    Ok(report)
}
