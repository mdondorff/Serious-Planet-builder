//! The erosion layer's pass 2 in a store (STRM-065, GEN-101; m9-bake PR 5): one record per tile holding the core's change against the base field, quantised, compressed, with the tile's
//! bake statistics, checkpointed per tile so that a killed bake resumes from its last complete tile, and refused when it was made for another bake, other parameters or another pass 1.
//!
//! Record kind 13 ([`KIND_LAYER_TILE`]) filed under the tile's id at the level whose grid is the tile grid (`2^level` tiles a face edge). The key is the bake's stage key (the source), the
//! bake's resolution, and under tag 16 the layer version, the kernel version, the tile grid, the radius, the pass-2 parameters, `inputs_id`, whether the carve factor is applied
//! and a digest of the keys of the six pass-1 face records the tile was computed from (a changed pass 1 is another layer, ADR 0006). The payload is
//! `codec (u8) | tile (u32 LE, core cells a side) | stats (land u64, thermal passes u32, thermal still moving u64, eroded, deposited, exported f64 LE, clamped cells u64) | key (16 bytes) |`
//! a brotli stream (quality 5, 22-bit window; SPIKE-97, owner 2026-11-03) of the core's `i16` counts of 0.25 m (little-endian, row `j` outer, then `i`), each row stored as its first count and the wrapping
//! differences to the left neighbour (about 0.92 bytes a land cell, 20 GB for the planet against lz4's 34 GB). A decoder refuses a codec byte it does not know and a stream that does not
//! decode to exactly `tile x tile x 2` bytes, and never reserves more than that.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use planet_core::{Face, TileId, Vec3};
use planet_generators::bake::face::FaceChange;
use planet_generators::bake::tile::{dequantise, pass2_tile, quantise, Pass2Params, TileGrid, TileId as PassTile, TileResult, TileStats};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_RESOLUTION, PARAM_TILE_LAYER};
use crate::layer_store::{face_key, LayerSpec, LayerStoreError};
use crate::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};

/// The kind byte of a tile's pass-2 change.
pub const KIND_LAYER_TILE: u8 = Kind::LayerTile as u8;
/// Version of the tile record's layout and of the meaning of its key. History: 1 = counts in an lz4 block; 2 = row-delta counts in a brotli stream (codec 2).
pub const TILE_LAYER_VERSION: u32 = 2;
const IMPLEMENTATION_ID: &str = "erosion-layer-tile";
/// The codec byte of the payload: 2 = row-delta `i16` counts, brotli quality 5. (1, an lz4 block, was never a released layer.)
const CODEC_VERSION: u8 = 2;
const BROTLI_QUALITY: u32 = 5;
const BROTLI_WINDOW_BITS: u32 = 22;
const BROTLI_BUFFER: usize = 64 * 1024;
const HEADER_LEN: usize = 1 + 4 + 8 + 4 + 8 + 8 + 8 + 8 + 8 + 16;

/// What a tile layer is made of besides the bake it is beside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileLayerSpec {
    pub grid: TileGrid,
    pub params: Pass2Params,
    /// An identity of the inputs of the base field and the carve factor that the bake's key does not cover (as [`LayerSpec::inputs_id`]).
    pub inputs_id: u64,
    pub uses_factor: bool,
    /// A digest of the six pass-1 face keys ([`faces_digest`]): the tile reads the face change, so the pass 1 it was made from is part of its key.
    pub faces: [u8; 16],
}

impl TileLayerSpec {
    pub(crate) fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(256);
        b.extend(TILE_LAYER_VERSION.to_le_bytes());
        b.extend(planet_generators::bake::BAKE_KERNEL_VERSION.to_le_bytes());
        b.extend(planet_generators::bake::PASS2_KERNEL_VERSION.to_le_bytes());
        for v in [self.grid.cells, self.grid.tile, self.grid.halo] {
            b.extend((v as u64).to_le_bytes());
        }
        b.extend(self.grid.radius_m.to_bits().to_le_bytes());
        let p = &self.params;
        for v in [
            p.carve.u_over_k,
            p.carve.k_dt,
            p.carve.epsilon,
            p.carve.breach_slope,
            p.carve.breach_max_m,
            p.carve.sea_level,
            p.carve.max_area,
        ] {
            b.extend(v.to_bits().to_le_bytes());
        }
        b.push(u8::from(p.carve.incise_only));
        b.extend((p.carve.steps as u64).to_le_bytes());
        b.extend((p.carve.reroute_every as u64).to_le_bytes());
        let t = &p.thermal;
        for v in [t.theta_deg, t.perturb, t.k, t.min_excess, t.sea_level] {
            b.extend(v.to_bits().to_le_bytes());
        }
        b.extend((t.passes as u64).to_le_bytes());
        for v in [p.kc, p.zeta, p.local_area_cells, p.sea_level] {
            b.extend(v.to_bits().to_le_bytes());
        }
        b.extend(p.seed.to_le_bytes());
        b.extend(self.inputs_id.to_le_bytes());
        b.push(u8::from(self.uses_factor));
        b.extend(self.faces);
        b
    }
}

/// The digest of the keys of the six pass-1 face records of `layer` (what a tile's face change comes from).
pub fn faces_digest(world: &GlobalWorld, res: u32, layer: &LayerSpec) -> [u8; 16] {
    let mut h = blake3::Hasher::new();
    for f in 0..6 {
        h.update(&face_key(world, res, layer, Face(f)).0);
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.finalize().as_bytes()[..16]);
    out
}

/// The tile a record is filed under: `2^level` tiles a face edge. Refused when the tiles a face edge are not a power of two.
pub fn layer_tile_id(grid: &TileGrid, id: PassTile) -> Result<TileId, LayerStoreError> {
    let n = grid.tiles_per_edge();
    if !n.is_power_of_two() {
        return Err(LayerStoreError(format!("{n} tiles a face edge is not a power of two: a tile cannot be filed under a quadtree id")));
    }
    TileId::new(id.face, n.trailing_zeros() as u8, id.ti, id.tj)
        .ok_or_else(|| LayerStoreError(format!("tile ({}, {}) of face {} is outside the {n} tiles a face edge", id.ti, id.tj, id.face.0)))
}

fn tile_key(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<CacheKey, LayerStoreError> {
    let tile = layer_tile_id(&spec.grid, id)?;
    let mut inputs = world.base_key_inputs(Kind::LayerTile, tile, res, IMPLEMENTATION_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, (spec.grid.cells as u32).to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_TILE_LAYER, spec.bytes());
    Ok(cache_key(&inputs))
}

/// The record of a tile's result under `spec`.
pub fn tile_record(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, r: &TileResult) -> Result<Record, LayerStoreError> {
    assert_eq!(r.core.len(), spec.grid.tile * spec.grid.tile, "a tile result holds tile x tile cells");
    let key = tile_key(world, res, spec, r.id)?;
    let counts = quantise(&r.core);
    let raw = pack_counts(&counts, spec.grid.tile);
    let s = &r.stats;
    let mut payload = Vec::with_capacity(HEADER_LEN + raw.len() / 2);
    payload.push(CODEC_VERSION);
    payload.extend((spec.grid.tile as u32).to_le_bytes());
    payload.extend((s.land_cells as u64).to_le_bytes());
    payload.extend((s.thermal_passes as u32).to_le_bytes());
    payload.extend((s.thermal_moving_at_cap as u64).to_le_bytes());
    for v in [s.eroded_m3, s.deposited_m3, s.exported_m3] {
        payload.extend(v.to_bits().to_le_bytes());
    }
    payload.extend((s.clamped_cells as u64).to_le_bytes());
    payload.extend(key.0);
    {
        let mut w = brotli::CompressorWriter::new(&mut payload, BROTLI_BUFFER, BROTLI_QUALITY, BROTLI_WINDOW_BITS);
        w.write_all(&raw).expect("writing to a vector");
    }
    Ok(Record {
        key,
        kind: KIND_LAYER_TILE,
        tile: layer_tile_id(&spec.grid, r.id)?,
        generator_version: planet_generators::global::GLOBAL_VERSION,
        hash_version: planet_core::hash::HASH_VERSION,
        node_set_id: world.node_set_id(),
        payload,
    })
}

/// The stored result of tile `id` under `spec`, its core dequantised: `Ok(None)` when the store holds no record of the tile; an error naming both keys when it holds one made under
/// another key (another bake, parameters or pass 1); an error naming the tile when the record is damaged.
pub fn load_tile(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    id: PassTile,
) -> Result<Option<TileResult>, LayerStoreError> {
    let key = tile_key(world, res, spec, id)?;
    let tile = layer_tile_id(&spec.grid, id)?;
    let what = |why: String| {
        LayerStoreError(format!("layer tile {}/{}/{} (record kind {KIND_LAYER_TILE}) key {key}: {why}", id.face.0, id.ti, id.tj))
    };
    let Some(r) = store.get(KIND_LAYER_TILE, tile, key).map_err(|e| what(e.to_string()))? else {
        return match store.keys_for(KIND_LAYER_TILE, tile).first() {
            Some(other) => Err(what(format!("the store holds this tile under key {other}, made for another bake, other parameters or another pass 1: refused, not mixed"))),
            None => Ok(None),
        };
    };
    let p = &r.payload;
    let t = spec.grid.tile;
    if p.len() < HEADER_LEN || p[0] != CODEC_VERSION || u32::from_le_bytes(p[1..5].try_into().expect("four bytes")) as usize != t {
        return Err(what(format!("a payload of {} bytes that is not a tile of {t} cells in codec {CODEC_VERSION}", p.len())));
    }
    let word = |a: usize, n: usize| &p[a..a + n];
    let u64_at = |a: usize| u64::from_le_bytes(word(a, 8).try_into().expect("eight bytes"));
    let f64_at = |a: usize| f64::from_bits(u64_at(a));
    if word(5 + 8 + 4 + 8 + 24 + 8, 16) != key.0 {
        return Err(what("its payload names another key than its record".into()));
    }
    let counts = unpack_counts(&p[HEADER_LEN..], t).map_err(what)?;
    let stats = TileStats {
        land_cells: u64_at(5) as usize,
        thermal_passes: u32::from_le_bytes(word(13, 4).try_into().expect("four bytes")) as usize,
        thermal_moving_at_cap: u64_at(17) as usize,
        eroded_m3: f64_at(25),
        deposited_m3: f64_at(33),
        exported_m3: f64_at(41),
        clamped_cells: u64_at(49) as usize,
    };
    Ok(Some(TileResult { id, core: dequantise(&counts), deposited: Vec::new(), stats }))
}

/// The core's counts as bytes: little-endian, each row of `t` as its first count and the wrapping differences along the row.
fn pack_counts(counts: &[i16], t: usize) -> Vec<u8> {
    let mut raw = Vec::with_capacity(counts.len() * 2);
    for row in counts.chunks(t) {
        let mut prev = 0i16;
        for &c in row {
            raw.extend(c.wrapping_sub(prev).to_le_bytes());
            prev = c;
        }
    }
    raw
}

/// The counts of a brotli stream of `t x t` row-delta counts; an error text when the stream is damaged or does not hold exactly that many bytes (reading stops one byte past it).
fn unpack_counts(stream: &[u8], t: usize) -> Result<Vec<i16>, String> {
    let want = t * t * 2;
    let mut raw = Vec::with_capacity(want);
    brotli::Decompressor::new(stream, BROTLI_BUFFER)
        .take(want as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| format!("its counts do not decompress: {e}"))?;
    if raw.len() != want {
        return Err(format!("{} bytes of counts{}, expected {want}", raw.len(), if raw.len() > want { " or more" } else { "" }));
    }
    let mut counts = Vec::with_capacity(t * t);
    for row in raw.as_chunks::<2>().0.chunks(t) {
        let mut acc = 0i16;
        for d in row {
            acc = acc.wrapping_add(i16::from_le_bytes(*d));
            counts.push(acc);
        }
    }
    Ok(counts)
}

/// A bounded cache of decoded tiles for the read side (SPIKE-97: a decode per render tile would cost 3.1 times terrain's share, and a tile serves every render tile of its 312.7 km square).
/// Least recently used out first; `capacity` is in tiles (a decoded core is `tile x tile` f32). Safe to share between threads; two threads asking for the same absent tile may both decode it.
pub struct DecodedTileCache {
    capacity: usize,
    inner: Mutex<CacheInner>,
}

#[derive(Default)]
struct CacheInner {
    tiles: HashMap<PassTile, Arc<TileResult>>,
    /// Oldest use first.
    order: VecDeque<PassTile>,
    decodes: u64,
}

impl DecodedTileCache {
    /// A cache of at most `capacity` tiles (at least 1).
    pub fn new(capacity: usize) -> DecodedTileCache {
        DecodedTileCache { capacity: capacity.max(1), inner: Mutex::new(CacheInner::default()) }
    }

    /// The tile `id`, decoded once while it stays cached; `None` when the store holds none (not cached).
    pub fn get(
        &self,
        store: &TileStore,
        world: &GlobalWorld,
        res: u32,
        spec: &TileLayerSpec,
        id: PassTile,
    ) -> Result<Option<Arc<TileResult>>, LayerStoreError> {
        {
            let mut g = self.inner.lock().expect("cache lock");
            if let Some(t) = g.tiles.get(&id).cloned() {
                g.order.retain(|x| *x != id);
                g.order.push_back(id);
                return Ok(Some(t));
            }
        }
        let Some(r) = load_tile(store, world, res, spec, id)? else { return Ok(None) };
        let r = Arc::new(r);
        let mut g = self.inner.lock().expect("cache lock");
        g.decodes += 1;
        if g.tiles.insert(id, r.clone()).is_none() {
            g.order.push_back(id);
        }
        while g.tiles.len() > self.capacity {
            if let Some(old) = g.order.pop_front() {
                g.tiles.remove(&old);
            }
        }
        Ok(Some(r))
    }

    /// Tiles decoded so far (a hit is not a decode).
    pub fn decodes(&self) -> u64 {
        self.inner.lock().expect("cache lock").decodes
    }

    /// Tiles held now.
    pub fn len(&self) -> usize {
        self.inner.lock().expect("cache lock").tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The shape of a bake of the erosion layer: what `bake_layer` takes as options and the viewer must know to rebuild the layer's keys (until the layer's manifest records them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeShape {
    pub seed: u64,
    pub radius_m: f64,
    /// Cells a face edge of pass 1 (`--face-res`, GEN-100: 12,288).
    pub face_res: usize,
    /// Cells a face edge of pass 2 (`--planet-res`: 98,304).
    pub planet_res: usize,
    /// Core cells a tile edge (`--tile`; default the smaller of `planet_res` and 3,072).
    pub tile: Option<usize>,
    /// Halo cells around a tile (`--halo`; default a twenty-fourth of the tile).
    pub halo: Option<usize>,
}

impl BakeShape {
    /// The production bake of `seed`: pass 1 at 12,288 cells, pass 2 at 98,304.
    pub fn production(seed: u64, radius_m: f64) -> BakeShape {
        BakeShape { seed, radius_m, face_res: 12_288, planet_res: 98_304, tile: None, halo: None }
    }
}

/// The specs of pass 1 and pass 2 of a bake of `shape`, derived once for the command and the viewer so that they cannot drift: the key of every tile record follows them (an error from
/// [`TileGrid::check`] when the geometry does not fit).
pub fn bake_specs(world: &GlobalWorld, shape: &BakeShape) -> Result<(LayerSpec, TileLayerSpec), String> {
    let tile = shape.tile.unwrap_or(shape.planet_res.min(3072));
    let halo = shape.halo.unwrap_or(tile / 24);
    let grid = TileGrid { cells: shape.planet_res, tile, halo, radius_m: shape.radius_m };
    grid.check()?;
    if shape.face_res < 8 {
        return Err(format!("pass 1 has {} cells a face edge, at least 8 are needed", shape.face_res));
    }
    let face_halo = shape.face_res.div_ceil(128).max(4).min((shape.face_res - 1) / 2);
    let layer = LayerSpec {
        cells: shape.face_res,
        radius_m: shape.radius_m,
        sea_level: 0.0,
        seed: shape.seed,
        carve: planet_generators::bake::carve::CarveParams::default(),
        inputs_id: planet_generators::bake::inputs::BakeInputs::id(),
        uses_factor: true,
        halo: face_halo,
    };
    let spec = TileLayerSpec {
        grid,
        params: Pass2Params::standard(shape.seed, 0.0),
        inputs_id: planet_generators::bake::inputs::BakeInputs::id(),
        uses_factor: true,
        faces: faces_digest(world, shape.face_res as u32, &layer),
    };
    Ok((layer, spec))
}

/// The tiles of a [`StoreLayer`] whose pyramid is built, and the order they were last used (oldest first).
type BuiltTiles = (HashMap<PassTile, Arc<planet_generators::erosion_layer::LayerTile>>, VecDeque<PassTile>);

/// The pass-2 layer of a store as a [`planet_generators::erosion_layer::LayerSource`] (GEN-112): tiles decoded once while they stay in a bounded least-recently-used cache (SPIKE-97: one
/// decode serves every render tile of its 312.7 km square), each held with its min/max pyramid, built when the tile is decoded (STRM-066: built at decode, not stored). A tile the store
/// does not hold is a tile with no land: `None`. A record made under another key, or a damaged one, is an error naming the tile (never mixed, STRM-065); so is a tile whose change lies
/// outside the global range the layer was opened with (a range from a manifest that is too narrow would make the bounds unsound).
///
/// `capacity` should be at least [`planet_generators::erosion_layer::MAX_CAP_TILES`] + 4 tiles (a cap's bounds read that many, and a read a stencil's four): a decoded core is `tile x tile` f32
/// (37.7 MB at 3,072 cells). The cache holds the decoded core once, in the tile's own `Arc`.
pub struct StoreLayer {
    store: Arc<TileStore>,
    world: Arc<GlobalWorld>,
    res: u32,
    spec: TileLayerSpec,
    built: Mutex<BuiltTiles>,
    decodes: std::sync::atomic::AtomicU64,
    capacity: usize,
    range: (f64, f64),
    id: u64,
}

/// The range of the change when the layer's manifest gives none: the representable range of the stored counts (`i16` of [`planet_generators::bake::tile::QUANT_STEP_M`]).
pub fn default_change_range() -> (f64, f64) {
    let limit = f64::from(i16::MAX) * f64::from(planet_generators::bake::tile::QUANT_STEP_M);
    (-limit, limit)
}

impl StoreLayer {
    /// The layer of `spec` in `store` beside the bake `world` at `res`, with `capacity` decoded tiles cached and `range` the change's range over the planet (the layer's manifest, else
    /// [`default_change_range`]). An error when the key of the layer cannot be formed (the tile grid does not fit the store's levels).
    pub fn new(
        store: Arc<TileStore>,
        world: Arc<GlobalWorld>,
        res: u32,
        spec: TileLayerSpec,
        capacity: usize,
        range: (f64, f64),
    ) -> Result<StoreLayer, String> {
        // The layer's identity is the key of its first tile: the bake, its resolution, and every parameter of the layer and of the pass 1 it was made from.
        let k = tile_key(&world, res, &spec, PassTile { face: Face(0), ti: 0, tj: 0 }).map_err(|e| e.to_string())?;
        let word = |a: usize| u64::from_le_bytes(k.0[a..a + 8].try_into().expect("eight bytes"));
        let id = planet_core::hash::hash_u64s(0x7374_6c79, &[word(0), word(8)]);
        Ok(StoreLayer {
            store,
            world,
            res,
            spec,
            built: Mutex::new((HashMap::new(), VecDeque::new())),
            decodes: 0.into(),
            capacity: capacity.max(1),
            range,
            id,
        })
    }

    /// Opens the layer of `shape` in `dir` (a folder of tile packs, or a bake's output folder holding `tiles/`) beside the bake `world` and checks that it holds tiles under the keys of
    /// that shape: the first stored tile is decoded, so a layer made for another bake, other parameters or another pass 1 is refused here, naming both keys (STRM-065), and a store with no
    /// tile of this shape says so. `capacity` as [`StoreLayer::new`], `range` the layer's global change range ([`default_change_range`] when the manifest gives none).
    pub fn open(dir: &Path, world: Arc<GlobalWorld>, shape: &BakeShape, capacity: usize, range: (f64, f64)) -> Result<StoreLayer, String> {
        let tiles_dir = if dir.join("tiles").is_dir() { dir.join("tiles") } else { dir.to_path_buf() };
        if !tiles_dir.is_dir() {
            return Err(format!(
                "{}: no such folder (the erosion layer of `bake_layer`: its output folder or its tiles/ folder)",
                dir.display()
            ));
        }
        let (_, spec) = bake_specs(&world, shape)?;
        let store = TileStore::open(&tiles_dir, world.params()).map_err(|e| format!("{}: {e}", tiles_dir.display()))?;
        let layer = StoreLayer::new(Arc::new(store), world, shape.planet_res as u32, spec, capacity, range)?;
        let grid = layer.spec.grid;
        let first = grid
            .all_tiles()
            .into_iter()
            .find(|t| layer_tile_id(&grid, *t).is_ok_and(|id| !layer.store.keys_for(KIND_LAYER_TILE, id).is_empty()))
            .ok_or_else(|| {
                format!(
                    "{}: the folder holds no layer tile (record kind {KIND_LAYER_TILE}) of any level of a {}-cell grid",
                    tiles_dir.display(),
                    grid.cells
                )
            })?;
        planet_generators::erosion_layer::LayerSource::tile(&layer, first.face, first.ti, first.tj)?;
        Ok(layer)
    }

    /// Tiles decoded so far (a hit is not a decode; a tile that left the cache and comes back is).
    pub fn decodes(&self) -> u64 {
        self.decodes.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl planet_generators::erosion_layer::LayerSource for StoreLayer {
    fn grid(&self) -> &TileGrid {
        &self.spec.grid
    }

    fn tile(&self, face: Face, ti: u32, tj: u32) -> Result<Option<Arc<planet_generators::erosion_layer::LayerTile>>, String> {
        let id = PassTile { face, ti, tj };
        {
            let mut g = self.built.lock().expect("built lock");
            if let Some(t) = g.0.get(&id).cloned() {
                g.1.retain(|x| *x != id);
                g.1.push_back(id);
                return Ok(Some(t));
            }
        }
        let Some(result) = load_tile(&self.store, &self.world, self.res, &self.spec, id).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let tile = Arc::new(planet_generators::erosion_layer::LayerTile::new(Arc::new(result), self.spec.grid.tile)?);
        let [lo, hi] = tile.range();
        if f64::from(lo) < self.range.0 || f64::from(hi) > self.range.1 {
            return Err(format!(
                "layer tile {}/{ti}/{tj}: its change [{lo}, {hi}] m lies outside the layer's global range [{}, {}] m (a manifest range that is too narrow)",
                face.0, self.range.0, self.range.1
            ));
        }
        self.decodes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut g = self.built.lock().expect("built lock");
        if g.0.insert(id, tile.clone()).is_none() {
            g.1.push_back(id);
        }
        while g.0.len() > self.capacity {
            if let Some(old) = g.1.pop_front() {
                g.0.remove(&old);
            }
        }
        Ok(Some(tile))
    }

    fn global_range(&self) -> (f64, f64) {
        self.range
    }

    fn id(&self) -> u64 {
        self.id
    }
}

/// What a run did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass2Report {
    pub computed: Vec<PassTile>,
    pub skipped: Vec<PassTile>,
}

/// The inputs of a pass-2 run: the base field, the face carve's change (the blended reader of the six faces) and the optional carve factor, at a direction.
pub struct Pass2Inputs<'a> {
    pub base: &'a (dyn Fn(Vec3) -> f64 + Sync),
    pub face_change: &'a (dyn Fn(Vec3) -> f64 + Sync),
    pub factor: Option<&'a (dyn Fn(Vec3) -> f64 + Sync)>,
}

/// Pass 2 over `tiles`, one at a time, into the tile layer store in `dir`: a tile stored under the current key is skipped (a resume), the others are computed and appended with the
/// index written after each (the checkpoint). A tile stored under another key is an error. `on_tile` is called after each computed tile (progress).
#[allow(clippy::too_many_arguments)]
pub fn run_pass2(
    dir: &Path,
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    inputs: &Pass2Inputs,
    tiles: &[PassTile],
    threads: usize,
    on_tile: &mut dyn FnMut(&TileResult),
) -> Result<Pass2Report, LayerStoreError> {
    if spec.uses_factor != inputs.factor.is_some() {
        return Err(LayerStoreError(format!(
            "the tile layer spec says uses_factor = {} but the carve factor given is {}",
            spec.uses_factor,
            if inputs.factor.is_some() { "present" } else { "absent" }
        )));
    }
    spec.grid.check().map_err(LayerStoreError)?;
    let err = |e: crate::pack::PackError| LayerStoreError(format!("the tile layer store {}: {e}", dir.display()));
    let mut writer = PackWriter::open(dir, world.params(), DEFAULT_MAX_PACK_BYTES).map_err(err)?;
    let store = TileStore::open(dir, world.params()).map_err(err)?;
    let mut report = Pass2Report { computed: Vec::new(), skipped: Vec::new() };
    for &id in tiles {
        if report.computed.contains(&id) || report.skipped.contains(&id) {
            continue;
        }
        let have_tile = load_tile(&store, world, res, spec, id)?.is_some();
        let have_depth = crate::layer_channels::has_depth(&store, world, res, spec, id)?;
        if have_tile && have_depth {
            report.skipped.push(id);
            continue;
        }
        let r = pass2_tile(&spec.grid, id, inputs.base, inputs.face_change, inputs.factor, &spec.params, threads);
        // A tile stored before the channels existed is computed again for its depth only; what is stored is not written twice.
        if !have_tile {
            writer.append(&tile_record(world, res, spec, &r)?).map_err(err)?;
        }
        if !have_depth {
            writer.append(&crate::layer_channels::depth_record(world, res, spec, &r)?).map_err(err)?;
        }
        writer.write_index().map_err(err)?;
        on_tile(&r);
        report.computed.push(id);
    }
    writer.finish().map_err(err)?;
    Ok(report)
}

/// The six faces' change as a reader (the blended `change_at` of the faces).
pub fn face_change_reader(faces: &[FaceChange; 6]) -> impl Fn(Vec3) -> f64 + Sync + '_ {
    move |d| planet_generators::bake::map::change_at(faces, d)
}

/// Drops the pass-1 face records: removes the layer store directory `dir` (the final layer is complete and `--keep-pass1` was not given). Refuses a directory that does not hold a layer
/// store (no pack or index file), so that a wrong path cannot delete a folder.
pub fn drop_pass1(dir: &Path) -> Result<(), LayerStoreError> {
    let e = |m: String| LayerStoreError(format!("drop pass 1 at {}: {m}", dir.display()));
    let entries = std::fs::read_dir(dir).map_err(|x| e(x.to_string()))?;
    let names: Vec<String> = entries.filter_map(Result::ok).map(|x| x.file_name().to_string_lossy().into_owned()).collect();
    if names.is_empty() || !names.iter().all(|n| n.ends_with(".tpack") || n.ends_with(".tidx") || n.ends_with(".tmp") || n == "writer.lock")
    {
        return Err(e(format!("the folder holds files that are not a tile store's ({names:?}): refused")));
    }
    std::fs::remove_dir_all(dir).map_err(|x| e(x.to_string()))
}
