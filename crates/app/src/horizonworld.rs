//! The chunk horizon in a world (M8-5, STRM-049): the baker that gives `streaming::chunks::HorizonJobs` its maps, and the world's methods to
//! enable the jobs, run a frame of them and read a map. Additive to the chunk wiring of `worldfield` (one field, no change to its methods).
//!
//! A chunk's horizon is baked from the world's own field (base, stamps and the changes of the chunks that are ready), sampled at the cell
//! directions of the chunk lattice over the chunk and a halo of `ceil(range / cell) + 2` cells, on the extended cube plane past a face edge.
//! The chunk's map is one byte per azimuth for each of its cells, row `j` outer (`HorizonMap` of GEN-044).

use std::path::Path;
use std::sync::{Arc, Mutex, Weak};

use planet_cache::global_store::HorizonSpec;
use planet_cache::horizon_store::{load_horizon, store_horizon};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, PlanetFixed, TileId};
use planet_generators::chunkfield::{ChunkKey, ChunkLattice, ChunkResult};
use planet_generators::coarsehorizon::{coarse_horizon, CoarseParams};
use planet_generators::global::LatticeSource;
use planet_generators::horizon::{bake_horizon, halo_texels, HeightRaster, HorizonParams};
use planet_generators::stamp::STAGE_HALO;
use planet_streaming::chunks::{HorizonBaker, HorizonConfig, HorizonJobs, HorizonStats, HorizonUpdate};
use planet_streaming::{Clock, Spawner};
use planet_world_def::WorldDefinition;

use crate::worldfield::WorldTerrain;

/// Version of the world's horizon bake (what it samples and how); part of the march's hash, so a change re-bakes every map.
pub const BAKE_VERSION: u32 = 1;

/// Bakes the horizon of a chunk from the world's field.
pub struct ChunkHorizonBaker {
    world: Weak<WorldTerrain>,
    lat: ChunkLattice,
    params: HorizonParams,
    threads: usize,
    /// The edit word of a chunk by the definition revision it was made for.
    words: Mutex<std::collections::HashMap<(ChunkKey, u64), (u64, u64)>>,
    /// The heights of the chunk-sized blocks of the raster, shared by the chunks that read them (STRM-059).
    tiles: Mutex<HeightTiles>,
    tile_cap: usize,
}

/// Most height tiles a baker keeps (STRM-059): 64 tiles of 614 x 614 f32 are 96.5 MB, the horizon's share of RAM (ADR 0083 point 4). The nine tiles
/// a bake reads, for the few bakes that run at once, are well inside it; the oldest-used tile goes first.
pub const HEIGHT_TILES_MAX: usize = 64;

/// What the height tiles did (the tests and the stats read it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileStats {
    /// Tiles evaluated from the field (cached or not).
    pub evaluated: u64,
    /// Tiles read from the cache.
    pub hits: u64,
    /// Tiles in the cache now.
    pub held: usize,
}

#[derive(Default)]
struct HeightTiles {
    /// By chunk: the key the heights were made under, the heights, and the use counter of the last read.
    held: std::collections::HashMap<ChunkKey, (u64, Arc<Vec<f32>>, u64)>,
    uses: u64,
    evaluated: u64,
    hits: u64,
}

impl ChunkHorizonBaker {
    pub fn new(world: &Arc<WorldTerrain>, lat: ChunkLattice, params: HorizonParams, threads: usize) -> ChunkHorizonBaker {
        ChunkHorizonBaker {
            world: Arc::downgrade(world),
            lat,
            params,
            threads: threads.max(1),
            words: Default::default(),
            tiles: Default::default(),
            tile_cap: HEIGHT_TILES_MAX,
        }
    }

    /// The hash of the items of every stamp layer that touch the chunk and `halo_cells` cells around it, 0 for none (the edit word of a horizon,
    /// and of a height tile with the stamp stage's own halo, STRM-059).
    fn items_word(&self, key: ChunkKey, halo_cells: f64) -> u64 {
        let Some(world) = self.world.upgrade() else { return 0 };
        let def = world.definition();
        let revision = def.revision();
        if let Some(&(r, w)) = self.words.lock().expect("edit words").get(&(key, halo_cells.to_bits())) {
            if r == revision {
                return w;
            }
        }
        let tile =
            TileId::new(Face(key.0), planet_generators::chunk::CHUNK_GRID_LEVEL, key.1, key.2).expect("a chunk of the grid is a tile");
        let halo = (halo_cells / f64::from(self.lat.cells_per_chunk())).clamp(0.0, 1.0);
        let cap = planet_world_def::footprint::tile_cap(&planet_core::TangentWarp, tile, halo);
        let mut items = std::collections::BTreeMap::new();
        for layer in def.stamp_layers(planet_world_def::BodyId::PLANET) {
            if let Ok(found) = def.footprint_items_cap(planet_world_def::BodyId::PLANET, &layer, cap) {
                for it in found {
                    items.entry((layer.clone(), it.id)).or_insert(it);
                }
            }
        }
        let word = if items.is_empty() {
            0
        } else {
            let all: Vec<planet_world_def::footprint::FootprintItem> = items.into_values().collect();
            planet_world_def::footprint_hash(&all) | 1
        };
        self.words.lock().expect("edit words").insert((key, halo_cells.to_bits()), (revision, word));
        word
    }

    /// Keep at most `cap` height tiles (0 turns the cache off: every bake evaluates its whole raster).
    pub fn with_tile_cap(mut self, cap: usize) -> ChunkHorizonBaker {
        self.tile_cap = cap;
        self
    }

    /// What the height tiles did so far.
    pub fn tile_stats(&self) -> TileStats {
        let t = self.tiles.lock().expect("height tiles");
        TileStats { evaluated: t.evaluated, hits: t.hits, held: t.held.len() }
    }

    /// What the heights of a block depend on besides its items and its chunk: the base of the definition (seed, radius, height field, stage order), the baked input, the code that makes the chunks and the field above them
    /// (`ANET_VERSION`, the chunk stage, this bake), the lattice, and the coupled lens, which the field chain does not hash into a tile key.
    fn world_word(&self, world: &WorldTerrain) -> u64 {
        let lens = world.coupled_lens().map_or(0, |l| 1 + l as u64);
        let ready = matches!(world.coupled_standing(), crate::coupledworld::Standing::Ready(..));
        let def = world.definition();
        let body = planet_world_def::BodyId::PLANET;
        let positions: Vec<u64> = planet_world_def::LAYER_STACK.iter().map(|l| def.chunk_stage(body, l) as u64).collect();
        let bake_key = world.bake_key();
        let key_words = [
            u64::from_le_bytes(bake_key[..8].try_into().expect("8 bytes")),
            u64::from_le_bytes(bake_key[8..].try_into().expect("8 bytes")),
        ];
        planet_core::hash::hash_u64s(
            0x4854_5753,
            &[
                u64::from(BAKE_VERSION),
                u64::from(planet_generators::anet::ANET_VERSION),
                u64::from(planet_generators::chunkfield::CHUNK_STAGE_VERSION),
                u64::from(self.lat.cells_per_chunk()),
                self.params.radius_m.to_bits(),
                lens,
                u64::from(ready),
                def.base_hash(),
                planet_core::hash::hash_u64s(2, &positions),
                key_words[0],
                key_words[1],
            ],
        )
    }

    /// The heights of the `side x side` cells from `(x0, y0)` on `face`'s plane (past a face edge on the extended plane), from the field.
    fn evaluate(&self, field: &dyn planet_generators::height::HeightField, face: u8, x0: i64, y0: i64, side: usize) -> Vec<f32> {
        let mut dirs = Vec::with_capacity(side * side);
        for j in 0..side {
            for i in 0..side {
                dirs.push(self.lat.direction_ext(face, x0 + i as i64, y0 + j as i64));
            }
        }
        field.heights_threaded_until(&dirs, &|| false, self.threads).into_iter().map(|h| h as f32).collect()
    }

    /// The heights of the chunk-sized block `(bx, by)` of `face`'s plane: from the cache under `tile_key`, else from the field (and kept under
    /// it). A block without a key (past the face edge, or a chunk that changed while the field was built) is evaluated and not kept.
    fn block_heights(
        &self,
        field: &dyn planet_generators::height::HeightField,
        face: u8,
        (bx, by): (i64, i64),
        n: usize,
        tile_key: Option<u64>,
    ) -> Arc<Vec<f32>> {
        let chunk = (face, bx as u32, by as u32);
        if let Some(k) = tile_key.filter(|_| self.tile_cap > 0) {
            let mut t = self.tiles.lock().expect("height tiles");
            t.uses += 1;
            let uses = t.uses;
            if let Some(e) = t.held.get_mut(&chunk).filter(|e| e.0 == k) {
                e.2 = uses;
                let h = e.1.clone();
                t.hits += 1;
                return h;
            }
        }
        let heights = Arc::new(self.evaluate(field, face, bx * n as i64, by * n as i64, n));
        let mut t = self.tiles.lock().expect("height tiles");
        t.evaluated += 1;
        if let Some(k) = tile_key.filter(|_| self.tile_cap > 0) {
            t.uses += 1;
            let uses = t.uses;
            t.held.insert(chunk, (k, heights.clone(), uses));
            while t.held.len() > self.tile_cap {
                let oldest = t.held.iter().min_by_key(|(c, e)| (e.2, **c)).map(|(c, _)| *c).expect("a tile");
                t.held.remove(&oldest);
            }
        }
        heights
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
        use planet_core::Side::{East, North, South, West};
        let map = &planet_core::cube::TangentWarp;
        let k_of = |t: TileId| (t.face().0, t.x(), t.y());
        let mut out = vec![key];
        for side in [East, West, North, South] {
            out.push(k_of(tile.neighbor(map, side)));
        }
        // The four diagonals: a side neighbour's neighbour in the other axis (not its own axis, which would reach two chunks away).
        for (a, b) in [(East, North), (East, South), (West, North), (West, South)] {
            let k = k_of(tile.neighbor(map, a).neighbor(map, b));
            if !out.contains(&k) {
                out.push(k);
            }
        }
        out
    }

    /// The map of `key`; a world that is gone or a field that cannot be built gives a map of zeros of the right length (no shadow), never a
    /// short one, so a consumer indexing by cell and azimuth stays inside it.
    fn bake(&self, key: ChunkKey, _ready: &[(ChunkKey, Arc<ChunkResult>)]) -> Option<Vec<u8>> {
        let n = self.lat.cells_per_chunk() as usize;
        let none = || Some(vec![0u8; n * n * self.params.azimuths as usize]);
        // The edit word the map is made against: a change while it is made drops it (STRM-060), the job bakes it again.
        let word0 = self.edit_word(key);
        let Some(world) = self.world.upgrade() else { return none() };
        let halo = self.halo_cells();
        let side = n + 2 * halo;
        let manager = world.chunk_manager();
        let per_face = 1i64 << planet_generators::chunk::CHUNK_GRID_LEVEL;
        let in_face = |b: i64| (0..per_face).contains(&b);
        // The state of a block: the results of the chunks its heights read (its own and the cells of its neighbours across the edge, GEN-039).
        let state = |bx: i64, by: i64| -> u64 {
            let Some(m) = manager.as_ref().filter(|_| in_face(bx) && in_face(by)) else { return 0 };
            self.reads((key.0, bx as u32, by as u32))
                .into_iter()
                .fold(0x4854_5354, |h, k| planet_core::hash::mix64(h ^ m.result_hash(k).unwrap_or(0)))
        };
        let own_hashes = |blocks: &[(i64, i64)]| -> Vec<u64> { blocks.iter().map(|&(bx, by)| state(bx, by)).collect() };
        let blocks: Vec<(i64, i64)> =
            (-1..=1i64).flat_map(|dy| (-1..=1i64).map(move |dx| (i64::from(key.1) + dx, i64::from(key.2) + dy))).collect();
        // The result each block holds now, read before the field is built: a tile is kept only if it is still that result after (the field takes
        // its own snapshot of the ready chunks in between).
        let before = own_hashes(&blocks);
        let Ok(field) = world.field() else { return none() };
        let after = own_hashes(&blocks);
        let world_word = self.world_word(&world);
        let (x0, y0) = (i64::from(key.1) * n as i64 - halo as i64, i64::from(key.2) * n as i64 - halo as i64);
        let mut heights = vec![0.0f32; side * side];
        if halo > n {
            // A halo wider than a chunk edge reaches past the nine blocks: the whole raster from the field, as before the tiles.
            heights = self.evaluate(&*field, key.0, x0, y0, side);
        } else {
            for (idx, &(bx, by)) in blocks.iter().enumerate() {
                if self.edit_word(key) != word0 {
                    return None;
                }
                let tile_key = (in_face(bx) && in_face(by) && before[idx] == after[idx]).then(|| {
                    let block = (key.0, bx as u32, by as u32);
                    planet_core::hash::hash_u64s(
                        0x4854_494c,
                        &[world_word, self.items_word(block, STAGE_HALO * f64::from(self.lat.cells_per_chunk())), before[idx]],
                    )
                });
                let tile = self.block_heights(&*field, key.0, (bx, by), n, tile_key);
                // The rows of the block inside the raster: the block's cells are [bx n, bx n + n), the raster's [x0, x0 + side).
                let (cx, cy) = (bx * n as i64 - x0, by * n as i64 - y0);
                let (i0, i1) = (cx.max(0), (cx + n as i64).min(side as i64));
                let (j0, j1) = (cy.max(0), (cy + n as i64).min(side as i64));
                for j in j0..j1 {
                    let src = ((j - cy) as usize) * n + (i0 - cx) as usize;
                    let dst = (j as usize) * side + i0 as usize;
                    let len = (i1 - i0) as usize;
                    heights[dst..dst + len].copy_from_slice(&tile[src..src + len]);
                }
            }
        }
        if self.edit_word(key) != word0 {
            return None;
        }
        // Chunk jobs are queued or running: the march takes one thread, so a bake never holds cores the visible work needs (STRM-060).
        let threads = if manager.as_ref().is_some_and(|m| m.in_flight() > 0) { 1 } else { self.threads };
        let raster = HeightRaster { n: side, texel_m: self.lat.cell_m(self.params.radius_m), heights: &heights };
        bake_horizon(&raster, (halo, halo, n, n), &self.params, threads).map_or_else(|_| none(), |m| Some(m.data))
    }

    /// The items of every stamp layer that touch the chunk and the halo the march reads around it (the horizon samples the whole field, so the layers
    /// above the chunk stage count too): the hash of those items, 0 for none. The cap is the chunk's own with a halo of the march's cells.
    fn edit_word(&self, key: ChunkKey) -> u64 {
        self.items_word(key, self.halo_cells() as f64)
    }

    /// The march, the lattice and the bake (the edits are in [`HorizonBaker::edit_word`], per chunk).
    fn spec_hash(&self) -> u64 {
        use planet_core::hash::mix64;
        let p = &self.params;
        let mut h = mix64(u64::from(BAKE_VERSION) ^ u64::from(p.azimuths) << 32);
        for v in [p.range_m, p.step_min_m, p.step_growth, p.radius_m] {
            h = mix64(h ^ v.to_bits());
        }
        h = mix64(h ^ u64::from(self.lat.cells_per_chunk()));
        h
    }
}

/// The longest range whose halo (`ceil(range / cell) + 2` cells) fits in one chunk edge: the 3 x 3 chunks a horizon reads then cover it.
pub fn max_range_m(lat: &ChunkLattice, radius_m: f64) -> f64 {
    (f64::from(lat.cells_per_chunk()) - 2.0).max(0.0) * lat.cell_m(radius_m)
}

/// The march of a world's chunk horizons: SPIKE-32's 16 azimuths and 5 km, the range cut to what a chunk edge allows (4.8 km at the 8 m lattice
/// of the A-net default; a 5 km range would need 627 cells of halo against a chunk of 614).
pub fn chunk_horizon_params(lat: &ChunkLattice, radius_m: f64) -> HorizonParams {
    let base = HorizonParams::chunk(radius_m);
    HorizonParams { range_m: base.range_m.min(max_range_m(lat, radius_m)), ..base }
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
        // The march must be one the bake accepts, and its halo must fit in the ring of neighbours the state word reads (a chunk edge): refused
        // here, not as a panic in a worker.
        let halo = baker.halo_cells();
        if halo > lat.cells_per_chunk() as usize {
            return Err(format!(
                "a range of {} m needs {halo} cells of halo, more than the {} cells of a chunk edge: use a range of at most {:.0} m (see `chunk_horizon_params`)",
                params.range_m,
                lat.cells_per_chunk(),
                max_range_m(&lat, params.radius_m)
            ));
        }
        let flat = vec![0.0f32; (2 * halo + 1) * (2 * halo + 1)];
        let trial = HeightRaster { n: 2 * halo + 1, texel_m: lat.cell_m(params.radius_m), heights: &flat };
        bake_horizon(&trial, (halo, halo, 1, 1), &params, 1).map_err(|e| format!("the horizon march is refused: {e}"))?;
        *self.horizon.write().expect("horizon jobs") = Some(Arc::new(HorizonJobs::new(chunks, baker, spawner, clock, None, cfg)));
        Ok(())
    }

    /// One frame of the horizon jobs: the chunks the plan's `tiles` read, nearest to `camera` first. Returns what landed and what was dropped
    /// (the tiles of a landed map want their horizon texture refreshed); without the jobs nothing happens. Never waits for a job.
    pub fn horizon_frame(&self, tiles: &[TileId], camera: PlanetFixed) -> HorizonUpdate {
        let (Some(jobs), Some(m)) = (crate::stall::read(&self.horizon, "horizon jobs lock (read)").clone(), self.chunk_manager()) else {
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
        crate::stall::read(&self.horizon, "horizon jobs lock (read)").as_ref()?.horizon(key)
    }

    /// The horizon jobs in flight, 0 without them.
    pub fn horizon_in_flight(&self) -> usize {
        crate::stall::read(&self.horizon, "horizon jobs lock (read)").as_ref().map_or(0, |j| j.in_flight())
    }

    /// What the horizon jobs did so far, `None` without them.
    pub fn horizon_stats(&self) -> Option<HorizonStats> {
        crate::stall::read(&self.horizon, "horizon jobs lock (read)").as_ref().map(|j| j.stats())
    }
}

/// The coarse horizon of a world as the renderer reads it (GEN-045, STRM-045): `6 m^2` observer cells of `azimuths` bytes, faces in order, rows
/// `j` outer, then `i`, then the azimuth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoarseHorizon {
    pub m: u32,
    pub azimuths: u32,
    pub bytes: Arc<Vec<u8>>,
    /// The map packed as the GPU reads it (`pack_coarse`), made on the loading thread so no frame pays for it.
    pub packed: Arc<Vec<u8>>,
}

/// Where the coarse horizon of a world stands.
#[derive(Clone, Debug)]
pub enum CoarseStanding {
    /// Not asked for.
    Idle,
    /// The thread is reading it.
    Working,
    Ready(CoarseHorizon),
    /// The store holds none (or there is no store): the world shades with the chunk horizon alone; the text says how to make it.
    Absent(String),
    /// The store holds one that cannot be used.
    Failed(String),
}

/// The slot a world keeps its coarse horizon in: asked for once, read on a thread, read by whoever draws (CON-22: no frame waits for it).
#[derive(Clone)]
pub struct CoarseSlot {
    inner: Arc<Mutex<CoarseStanding>>,
}

impl Default for CoarseSlot {
    fn default() -> Self {
        CoarseSlot { inner: Arc::new(Mutex::new(CoarseStanding::Idle)) }
    }
}

impl CoarseSlot {
    pub fn standing(&self) -> CoarseStanding {
        crate::stall::lock(&self.inner, "coarse horizon slot").clone()
    }

    /// Run `job` on a thread, once: a later call while it works or after it ended does nothing.
    pub fn request(&self, job: impl FnOnce() -> CoarseStanding + Send + 'static) {
        {
            let mut s = crate::stall::lock(&self.inner, "coarse horizon slot");
            if !matches!(*s, CoarseStanding::Idle) {
                return;
            }
            *s = CoarseStanding::Working;
        }
        let inner = Arc::clone(&self.inner);
        std::thread::Builder::new()
            .name("coarse-horizon".into())
            .spawn(move || {
                let result = job();
                *inner.lock().expect("coarse slot") = result;
            })
            .expect("spawn the coarse horizon's thread");
    }
}

/// The observer lattice and march the world's coarse horizon is stored under: 512 observer cells per face edge (or the run's, if smaller) and
/// the march of `CoarseParams::for_lattice` over the coupled run of `n` cells per face edge.
pub fn coarse_spec(n: u32, radius_m: f64) -> HorizonSpec {
    let params = CoarseParams::for_lattice(n as usize, radius_m);
    // A small run (a test body) has cells wider than the range: one step then, never a step past the range the march refuses.
    HorizonSpec { m: n.min(512), params: CoarseParams { step_m: params.step_m.min(params.range_m), ..params } }
}

/// What a world without a stored coarse horizon says: the far-field shadows need the explicit step that makes it from the stored coupled run.
fn absent_text(why: &str) -> String {
    format!("no coarse horizon for the far-field shadows ({why}): shading with the chunk horizon alone; make it with `planet generate --coarse-horizon <store dir> --global-res <N>` from the coupled run stored beside the bake")
}

/// The stored coarse horizon beside the store `dir` for the run of `def` on `n` cells per face edge from the bake `src`: read, never made (the
/// march costs minutes at the run's size). `Absent` when there is no store or it holds none for this run and march.
pub fn read_coarse(def: &WorldDefinition, dir: Option<&Path>, src: &dyn LatticeSource, n: u32, steps: u32) -> CoarseStanding {
    let Some(dir) = dir else { return CoarseStanding::Absent(absent_text("the world has no store directory")) };
    let radius_m = match planet_world_def::GenContext::new(def).body_radius_m(planet_world_def::BodyId::PLANET) {
        Ok(r) => r,
        Err(e) => return CoarseStanding::Failed(format!("the definition cannot be generated: {e}")),
    };
    let world = match crate::coupledworld::keyed_world(def, src, n, steps) {
        Ok(w) => w.with_horizon(coarse_spec(n, radius_m)),
        Err(e) => return CoarseStanding::Absent(absent_text(&e)),
    };
    let store = match TileStore::open(dir, world.params()) {
        Ok(s) => s,
        Err(e) => return CoarseStanding::Absent(absent_text(&format!("{}: {e}", dir.display()))),
    };
    match load_horizon(&store, &world) {
        Ok(h) if h.azimuths == planet_frame::horizonview::MAX_SLOT_AZIMUTHS as u32 && h.m >= 2 => {
            match planet_frame::horizonview::pack_coarse(&h.bytes, h.m as usize, h.azimuths as usize) {
                Some(packed) => CoarseStanding::Ready(CoarseHorizon {
                    m: h.m,
                    azimuths: h.azimuths,
                    bytes: Arc::new(h.bytes),
                    packed: Arc::new(packed),
                }),
                None => CoarseStanding::Failed("the stored coarse horizon does not pack for the GPU".to_string()),
            }
        }
        Ok(h) => CoarseStanding::Failed(format!(
            "the stored coarse horizon has {} azimuths over {} cells: the shader reads 16 over at least 2",
            h.azimuths, h.m
        )),
        Err(e) => CoarseStanding::Absent(absent_text(&e.to_string())),
    }
}

/// Make the coarse horizon of the stored coupled run beside the store `dir` and store it there (the explicit step; minutes at 2048 cells).
/// Errors when the store holds no coupled run for `n` and `steps` (made by the viewer or the coupled scene), naming it. Returns the records written.
pub fn make_coarse_horizon(
    def: &WorldDefinition,
    dir: &Path,
    src: &dyn LatticeSource,
    n: u32,
    steps: u32,
    threads: usize,
) -> Result<usize, String> {
    let run = crate::coupledworld::load_stored(def, Some(dir), src, n, steps)?
        .ok_or_else(|| format!("{}: no stored coupled run of {n} cells and {steps} steps to take the horizon from", dir.display()))?;
    let radius_m = planet_world_def::GenContext::new(def)
        .body_radius_m(planet_world_def::BodyId::PLANET)
        .map_err(|e| format!("the definition cannot be generated: {e}"))?;
    let spec = coarse_spec(n, radius_m);
    let world = crate::coupledworld::keyed_world(def, src, n, steps)?.with_horizon(spec);
    let lat = run.maps.lattice();
    let z: Vec<f32> = (0..lat.len()).map(|c| run.maps.z(c)).collect();
    let map = coarse_horizon(lat, &z, spec.m as usize, &spec.params, threads.max(1))?;
    store_horizon(dir, &world, &map, DEFAULT_MAX_PACK_BYTES).map_err(|e| format!("{}: {e}", dir.display()))
}

/// `generate --coarse-horizon DIR --global-res N [--world F | --seed S] [--threads T]`: make the coarse horizon of the coupled run stored beside the
/// bake in `DIR` and store it there; prints a progress line before the march and the wall time after it.
pub fn generate_coarse_horizon(o: &crate::Options, out: &mut dyn std::io::Write) -> Result<(), String> {
    use crate::err;
    let dir = o.coarse_horizon.as_ref().expect("checked by the caller");
    let others = [
        "--plan-check",
        "--compact-cache",
        "--prefill-dir",
        "--global-dir",
        "--verify-global",
        "--global-products",
        "--cache-dir",
        "--verify-cache",
        "--manifest",
        "--tile",
        "--out",
        "--region",
    ];
    let clash: Vec<&str> = others.into_iter().filter(|f| o.given.contains(*f)).collect();
    if !clash.is_empty() {
        return Err(format!("--coarse-horizon takes only --global-res, --world or --seed and --threads, not {}", clash.join(", ")));
    }
    if !o.given.contains("--global-res") {
        return Err(
            "--coarse-horizon needs --global-res N, the resolution the bake was made at (a store holds one bake per resolution)".into()
        );
    }
    if o.given.contains("--world") && o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    if !dir.is_dir() {
        return Err(format!("{}: no such store directory (a bake made by `generate --global-dir`)", dir.display()));
    }
    let def = match &o.world {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
            planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
    };
    let world =
        planet_cache::global_store::GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?;
    let store = TileStore::open(dir, world.params()).map_err(|e| e.to_string())?;
    let src = planet_cache::global_store::StoreSource::new(&store, &world, o.global_res).map_err(|e| e.to_string())?;
    let n = crate::coupledworld::run_cells(o.global_res);
    let radius_m = planet_world_def::GenContext::new(&def)
        .body_radius_m(planet_world_def::BodyId::PLANET)
        .map_err(|e| format!("the definition cannot be generated: {e}"))?;
    let spec = coarse_spec(n, radius_m);
    let threads = if o.threads == 0 { std::thread::available_parallelism().map_or(1, |t| t.get()) } else { o.threads };
    writeln!(
        out,
        "coarse horizon: marching {} azimuths to {:.0} km from {} observer cells per face edge over the coupled run of {n} cells ({} steps), {threads} threads",
        spec.params.azimuths,
        spec.params.range_m / 1000.0,
        spec.m,
        crate::coupledworld::COUPLED_STEPS
    )
    .map_err(err)?;
    let started = std::time::Instant::now();
    let records = make_coarse_horizon(&def, dir, &src, n, crate::coupledworld::COUPLED_STEPS, threads)?;
    writeln!(out, "coarse horizon: {records} records stored in {} in {:.1} s", dir.display(), started.elapsed().as_secs_f64()).map_err(err)
}

impl WorldTerrain {
    /// Start reading the stored coarse horizon on a thread, once (STRM-045): from the store beside the bake, for the coupled run the viewer shows.
    /// No frame waits for it (CON-22); until it is `Ready` the shadows come from the chunk horizon alone.
    pub fn request_coarse_horizon(self: &Arc<Self>) {
        let world = Arc::clone(self);
        self.coarse.request(move || world.read_coarse_now());
    }

    /// Where the coarse horizon stands.
    pub fn coarse_standing(&self) -> CoarseStanding {
        self.coarse.standing()
    }
}

/// What the viewer keeps between frames to give the renderer the horizon shadows (REND-038, REND-041): the slots of the chunk maps, the version of
/// each map (a re-bake changes it), the uploads still to do (one a frame, so a landing map never stalls a frame, CON-22) and whether the coarse
/// map was handed over.
pub struct HorizonRig {
    slots: planet_frame::horizonview::HorizonSlots,
    versions: std::collections::HashMap<ChunkKey, u64>,
    pending: std::collections::VecDeque<(ChunkKey, u32)>,
    frame: u64,
    coarse_done: bool,
    warned_azimuths: bool,
}

/// Slots of the viewer array of chunk horizons (one MiB each).
pub const VIEWER_HORIZON_SLOTS: u32 = 64;

impl Default for HorizonRig {
    fn default() -> Self {
        HorizonRig {
            slots: planet_frame::horizonview::HorizonSlots::new(VIEWER_HORIZON_SLOTS),
            versions: Default::default(),
            pending: Default::default(),
            frame: 0,
            coarse_done: false,
            warned_azimuths: false,
        }
    }
}

impl HorizonRig {
    /// The decisions of a frame, GPU-free (tier A): the maps that `landed` bump their versions; the chunks of `tiles` that `has_map` takes slots; the
    /// table of the frame, in which a node whose map is not uploaded yet reads slot -1 (the stand-in is the coarse horizon, CON-22); and the one
    /// upload of this frame, `(chunk, slot)`: the oldest still valid (the chunk keeps that slot and its map is still there).
    pub fn decide(
        &mut self,
        tiles: &[TileId],
        landed: &[ChunkKey],
        has_map: &dyn Fn(ChunkKey) -> bool,
        sun: [f32; 3],
    ) -> (planet_frame::horizonview::HorizonFrame, Option<(ChunkKey, u32)>) {
        use planet_frame::horizonview::{tile_in_chunk, HorizonFrame};
        self.frame += 1;
        for key in landed {
            *self.versions.entry(*key).or_insert(0) += 1;
        }
        // The chunks of the plan tiles that have a map, in the plan order.
        let mut wanted: Vec<(ChunkKey, u64)> = Vec::new();
        for &t in tiles {
            let Some((key, _)) = tile_in_chunk(t) else { continue };
            if wanted.iter().all(|(k, _)| *k != key) && has_map(key) {
                wanted.push((key, self.versions.get(&key).copied().unwrap_or(0)));
            }
        }
        let slot_update = self.slots.update(&wanted, self.frame);
        self.pending.extend(slot_update.uploads.iter().copied());
        let mut upload = None;
        while let Some((key, slot)) = self.pending.pop_front() {
            if self.slots.slot_of(key) != Some(slot) || !has_map(key) {
                continue;
            }
            upload = Some((key, slot));
            break;
        }
        let sun3 = planet_core::Vec3::new(f64::from(sun[0]), f64::from(sun[1]), f64::from(sun[2]));
        let mut frame = HorizonFrame::build(&self.slots, slot_update, tiles, sun3, &planet_core::cube::TangentWarp);
        let waiting: std::collections::HashSet<ChunkKey> =
            self.pending.iter().filter(|(k, s)| self.slots.slot_of(*k) == Some(*s)).map(|(k, _)| *k).collect();
        for (node, &t) in frame.nodes.iter_mut().zip(tiles) {
            if tile_in_chunk(t).is_some_and(|(k, _)| waiting.contains(&k)) {
                node.slot = -1;
            }
        }
        (frame, upload)
    }

    /// One frame: with the horizon update of the chunk step (`frame_step` ran the jobs for the plan tiles `tiles`), hand the renderer the table of the frame, the next map to upload and
    /// (once) the coarse map. `sun` is the lit plan sun direction. Does nothing without the chunk stage of the world.
    pub fn frame(
        &mut self,
        world: &WorldTerrain,
        ctx: &planet_render::GpuContext,
        renderer: &mut planet_render::TerrainRenderer,
        tiles: &[TileId],
        update: &HorizonUpdate,
        sun: [f32; 3],
    ) {
        use planet_frame::horizonview::{pack_map, MAX_SLOT_AZIMUTHS};
        let (Some(lat), true) = (world.chunk_lattice(), world.chunk_manager().is_some()) else { return };
        let (frame, upload) = self.decide(tiles, &update.landed, &|k| world.horizon_map(k).is_some(), sun);
        let mut packed: Vec<(u32, Vec<u8>)> = Vec::new();
        if let Some((key, slot)) = upload {
            if let Some(map) = world.horizon_map(key) {
                let n = lat.cells_per_chunk() as usize;
                match pack_map(&map, n, map.len() / (n * n).max(1)) {
                    Some(bytes) => packed.push((slot, bytes)),
                    None => {
                        if !self.warned_azimuths {
                            self.warned_azimuths = true;
                            eprintln!(
                                "horizon: a chunk map of {} bytes over {n} cells is not {MAX_SLOT_AZIMUTHS} azimuths: no shadow from it",
                                map.len()
                            );
                        }
                    }
                }
            }
        }
        renderer.set_horizon(ctx, &frame, &packed);
        if !self.coarse_done {
            match world.coarse_standing() {
                CoarseStanding::Ready(h) => {
                    renderer.set_horizon_coarse(ctx, Some((h.m, &h.packed)));
                    self.coarse_done = true;
                }
                CoarseStanding::Absent(text) | CoarseStanding::Failed(text) => {
                    eprintln!("horizon: {text}");
                    self.coarse_done = true;
                }
                CoarseStanding::Idle | CoarseStanding::Working => {}
            }
        }
    }
}

#[cfg(test)]
mod rig_tests {
    use super::*;
    use planet_generators::chunk::CHUNK_GRID_LEVEL;

    fn tile_of(x: u32) -> TileId {
        TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000 + x, 1000).unwrap()
    }

    fn slot_of(frame: &planet_frame::horizonview::HorizonFrame, i: usize) -> i32 {
        frame.nodes[i].slot
    }

    // spec: REND-038
    #[test]
    fn a_tile_drawn_before_its_map_has_uploaded_reads_slot_minus_one_and_after_the_upload_its_slot() {
        let mut rig = HorizonRig::default();
        let tiles = [tile_of(0), tile_of(1)];
        let key = |i: usize| (2u8, 1000 + i as u32, 1000u32);
        // Both maps landed this frame: one upload a frame, so the second chunk is still waiting and its node reads no map.
        let (f1, up1) = rig.decide(&tiles, &[key(0), key(1)], &|_| true, [0.0, 0.0, 1.0]);
        assert_eq!(up1.map(|u| u.0), Some(key(0)), "the first in the plan order uploads first");
        assert_eq!((slot_of(&f1, 0), slot_of(&f1, 1)), (0, -1), "the uploaded chunk reads its slot, the waiting one reads none");
        let (f2, up2) = rig.decide(&tiles, &[], &|_| true, [0.0, 0.0, 1.0]);
        assert_eq!(up2.map(|u| u.0), Some(key(1)));
        assert_eq!((slot_of(&f2, 0), slot_of(&f2, 1)), (0, 1));
        // Nothing left to upload.
        let (_, up3) = rig.decide(&tiles, &[], &|_| true, [0.0, 0.0, 1.0]);
        assert_eq!(up3, None);
        // A re-bake (landed again) uploads again under the same slot, in the frame it is drawn (the upload is written before the draw); a chunk
        // whose map is gone keeps its slot, and the texels in it, until the slot is needed.
        let (f4, up4) = rig.decide(&tiles, &[key(0)], &|k| k == key(0), [0.0, 0.0, 1.0]);
        assert_eq!(up4, Some((key(0), 0)));
        assert_eq!((slot_of(&f4, 0), slot_of(&f4, 1)), (0, 1));
    }

    // spec: REND-038
    #[test]
    fn with_every_slot_full_a_new_chunk_takes_the_slot_of_the_one_wanted_longest_ago() {
        let mut rig = HorizonRig::default();
        let n = VIEWER_HORIZON_SLOTS as usize;
        let tiles: Vec<TileId> = (0..n as u32).map(tile_of).collect();
        let key = |i: usize| (2u8, 1000 + i as u32, 1000u32);
        // Fill every slot: one upload a frame, so n frames and then every node reads its own slot.
        let mut landed: Vec<ChunkKey> = (0..n).map(key).collect();
        for _ in 0..n {
            rig.decide(&tiles, &landed, &|_| true, [0.0, 0.0, 1.0]);
            landed.clear();
        }
        let (full, none) = rig.decide(&tiles, &[], &|_| true, [0.0, 0.0, 1.0]);
        assert_eq!(none, None);
        let mut slots: Vec<i32> = full.nodes.iter().map(|x| x.slot).collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..n as i32).collect::<Vec<_>>(), "64 chunks, 64 distinct slots");
        // A 65th chunk is wanted and the first 32 are not: it takes a slot of the ones wanted longest ago and reads it after its upload.
        let fresh = tile_of(n as u32);
        let mut view: Vec<TileId> = tiles[n / 2..].to_vec();
        view.push(fresh);
        let (f, up) = rig.decide(&view, &[(2, 1000 + n as u32, 1000)], &|_| true, [0.0, 0.0, 1.0]);
        let (ukey, uslot) = up.expect("the new map uploads");
        assert_eq!(ukey, (2, 1000 + n as u32, 1000));
        assert_eq!(slot_of(&f, view.len() - 1), uslot as i32, "the new chunk reads the slot it uploaded to");
        assert!(rig.slots.slot_of(key(0)).is_none() || rig.slots.slot_of(key(n / 2)).is_some(), "a chunk still wanted keeps its slot");
        assert!((0..n / 2).any(|i| rig.slots.slot_of(key(i)).is_none()), "one of the chunks not wanted gave its slot up");
    }
}
