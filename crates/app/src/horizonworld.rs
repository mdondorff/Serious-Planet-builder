//! The chunk horizon in a world (M8-5, STRM-049): the baker that gives `streaming::chunks::HorizonJobs` its maps, and the world's methods to
//! enable the jobs, run a frame of them and read a map. Additive to the chunk wiring of `worldfield` (one field, no change to its methods).
//!
//! A chunk's horizon is baked from the world's own field (base, stamps and the changes of the chunks that are ready), sampled at the cell
//! directions of the chunk lattice over the chunk and a halo of `ceil(range / cell) + 2` cells, on the extended cube plane past a face edge.
//! The chunk's map is one byte per azimuth for each of its cells, row `j` outer (`HorizonMap` of GEN-044).

use std::sync::{Arc, Weak};

use planet_core::{Face, PlanetFixed, TileId};
use planet_generators::chunkfield::{ChunkKey, ChunkLattice, ChunkResult};
use planet_generators::horizon::{bake_horizon, halo_texels, HeightRaster, HorizonParams};
use planet_streaming::chunks::{HorizonBaker, HorizonConfig, HorizonJobs, HorizonStats, HorizonUpdate};
use planet_streaming::{Clock, Spawner};

use crate::worldfield::WorldTerrain;

/// Version of the world's horizon bake (what it samples and how); part of the march's hash, so a change re-bakes every map.
pub const BAKE_VERSION: u32 = 1;

/// Bakes the horizon of a chunk from the world's field.
pub struct ChunkHorizonBaker {
    world: Weak<WorldTerrain>,
    lat: ChunkLattice,
    params: HorizonParams,
    threads: usize,
}

impl ChunkHorizonBaker {
    pub fn new(world: &Arc<WorldTerrain>, lat: ChunkLattice, params: HorizonParams, threads: usize) -> ChunkHorizonBaker {
        ChunkHorizonBaker { world: Arc::downgrade(world), lat, params, threads: threads.max(1) }
    }

    /// Cells of halo the march of this baker needs on every side of a chunk.
    pub fn halo_cells(&self) -> usize {
        halo_texels(&self.params, self.lat.cell_m(self.params.radius_m))
    }
}

impl HorizonBaker for ChunkHorizonBaker {
    /// The chunk and its eight neighbours on the chunk grid (the halo of a chunk is no larger than one chunk edge for the default march; a chunk
    /// two rings away contributes its base only to a horizon past that).
    fn reads(&self, key: ChunkKey) -> Vec<ChunkKey> {
        let tile =
            TileId::new(Face(key.0), planet_generators::chunk::CHUNK_GRID_LEVEL, key.1, key.2).expect("a chunk of the grid is a tile");
        let mut out = vec![key];
        for side in planet_core::Side::ALL {
            let n = tile.neighbor(&planet_core::cube::TangentWarp, side);
            out.push((n.face().0, n.x(), n.y()));
        }
        // The diagonals: the neighbours of the side neighbours that are not already in.
        for side in planet_core::Side::ALL {
            let n = tile.neighbor(&planet_core::cube::TangentWarp, side);
            for other in planet_core::Side::ALL {
                let d = n.neighbor(&planet_core::cube::TangentWarp, other);
                let k = (d.face().0, d.x(), d.y());
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
        out
    }

    fn bake(&self, key: ChunkKey, _ready: &[(ChunkKey, Arc<ChunkResult>)]) -> Vec<u8> {
        let Some(world) = self.world.upgrade() else { return Vec::new() };
        let Ok(field) = world.field() else { return Vec::new() };
        let n = self.lat.cells_per_chunk() as usize;
        let halo = self.halo_cells();
        let side = n + 2 * halo;
        let (x0, y0) = (i64::from(key.1) * n as i64 - halo as i64, i64::from(key.2) * n as i64 - halo as i64);
        let mut dirs = Vec::with_capacity(side * side);
        for j in 0..side {
            for i in 0..side {
                dirs.push(self.lat.direction_ext(key.0, x0 + i as i64, y0 + j as i64));
            }
        }
        let heights: Vec<f32> = field.heights_threaded_until(&dirs, &|| false, self.threads).into_iter().map(|h| h as f32).collect();
        let raster = HeightRaster { n: side, texel_m: self.lat.cell_m(self.params.radius_m), heights: &heights };
        bake_horizon(&raster, (halo, halo, n, n), &self.params, self.threads).expect("the halo is the march's own").data
    }

    /// The march, the lattice, the bake and the world's edits: an edit changes every map's word, so every map is baked again once the edit has
    /// settled (a superset of the chunks whose halo it touches; the precise set is M8-7's).
    fn spec_hash(&self) -> u64 {
        use planet_core::hash::mix64;
        let p = &self.params;
        let mut h = mix64(u64::from(BAKE_VERSION) ^ u64::from(p.azimuths) << 32);
        for v in [p.range_m, p.step_min_m, p.step_growth, p.radius_m] {
            h = mix64(h ^ v.to_bits());
        }
        h = mix64(h ^ u64::from(self.lat.cells_per_chunk()));
        if let Some(world) = self.world.upgrade() {
            h = mix64(h ^ world.definition().revision());
        }
        h
    }
}

impl WorldTerrain {
    /// Give a world that has a chunk stage the horizon jobs (STRM-049): a second job per ready chunk on `spawner`, `clock` for the settle window,
    /// `params` the march (its cell is the chunk lattice's). Errors without a chunk stage. A second call replaces the first.
    pub fn enable_horizon(
        self: &Arc<Self>,
        params: HorizonParams,
        spawner: Arc<dyn Spawner>,
        clock: Arc<dyn Clock>,
        cfg: HorizonConfig,
        threads: usize,
    ) -> Result<(), String> {
        let chunks = self.chunk_manager().ok_or("the horizon jobs need the world's chunk stage: enable_chunks first")?;
        let lat = self.chunk_lattice().ok_or("the world has no chunk lattice")?;
        let baker = Arc::new(ChunkHorizonBaker::new(self, lat, params, threads));
        *self.horizon.write().expect("horizon jobs") = Some(Arc::new(HorizonJobs::new(chunks, baker, spawner, clock, None, cfg)));
        Ok(())
    }

    /// One frame of the horizon jobs: the chunks the plan's `tiles` read, nearest to `camera` first. Returns what landed and what was dropped
    /// (the tiles of a landed map want their horizon texture refreshed); without the jobs nothing happens. Never waits for a job.
    pub fn horizon_frame(&self, tiles: &[TileId], camera: PlanetFixed) -> HorizonUpdate {
        let (Some(jobs), Some(m)) = (self.horizon.read().expect("horizon jobs").clone(), self.chunk_manager()) else {
            return Default::default();
        };
        let mut wanted: Vec<(ChunkKey, f64)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &t in tiles {
            for k in m.reads(t) {
                if seen.insert(k) {
                    let tile = TileId::new(Face(k.0), planet_generators::chunk::CHUNK_GRID_LEVEL, k.1, k.2)
                        .expect("a chunk of the grid is a tile");
                    let d = (tile.center_direction(&planet_core::cube::TangentWarp) * self.radius_m() - camera.0).length();
                    wanted.push((k, d));
                }
            }
        }
        jobs.update(&wanted)
    }

    /// The horizon map of a chunk, if one has landed (`None`: the tile reads the coarse horizon alone).
    pub fn horizon_map(&self, key: ChunkKey) -> Option<Arc<Vec<u8>>> {
        self.horizon.read().expect("horizon jobs").as_ref()?.horizon(key)
    }

    /// What the horizon jobs did so far, `None` without them.
    pub fn horizon_stats(&self) -> Option<HorizonStats> {
        self.horizon.read().expect("horizon jobs").as_ref().map(|j| j.stats())
    }
}
