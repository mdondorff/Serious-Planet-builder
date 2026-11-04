//! The erosion layer's two baked channels beside kinds 12 and 13 (GEN-110, GEN-111, STRM-072; m9-bake): the drainage area of pass 1 and the erosion and deposition depth of pass 2, kept for
//! the viewer's ground materials and detail.
//!
//! Record kind 14 ([`KIND_LAYER_AREA`]) is one record per cube face filed under the face's root tile: `codec (u8) | cells (u32 LE, pass 1's cells a face edge) | key (16 bytes) |` a brotli
//! stream (quality 5, window 22 bits) of the face's own cells as `u8` codes of the drainage area by multiple flow direction (`round(8 log2(area / cell area))`), row `j` outer, each row as its
//! first byte and the wrapping differences along the row. Record kind 15 ([`KIND_LAYER_DEPTH`]) is one record per tile filed like kind 13: `codec (u8) | tile (u32 LE) | key (16 bytes) |` a
//! brotli stream of the eroded plane then the deposited plane of the core (`u8` codes `round(16 sqrt(m))`, 0 to 254 m), each row as in kind 14. A decoder refuses an unknown codec and a stream
//! that does not decode to exactly the expected bytes, and never reserves more than that. Both keys carry the version of the channels, the scales of the codes and the key parameters of the layer
//! they were computed beside, so a channel of another layer is refused by name, never mixed.
//!
//! [`LayerChannels`] reads them at a direction (bilinear in metres, or in log2 cells) over the store, with bounded caches of the decoded face and tiles.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use planet_core::cube::{direction_to_face, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::face::area_cells;
use planet_generators::bake::tile::{depth_m, TileId as PassTile, TileResult};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_LAYER_CHANNELS, PARAM_RESOLUTION};
use crate::layer_store::{LayerSpec, LayerStoreError};
use crate::layer_tiles::{layer_tile_id, TileLayerSpec};
use crate::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};
use crate::relief_store::face_tile;

/// The kind byte of a face's drainage area.
pub const KIND_LAYER_AREA: u8 = Kind::LayerArea as u8;
/// The kind byte of a tile's erosion and deposition depth.
pub const KIND_LAYER_DEPTH: u8 = Kind::LayerDepth as u8;
/// Version of the channels' layout and codes. History: 1 = the first layout.
pub const CHANNELS_VERSION: u32 = 1;
const AREA_ID: &str = "erosion-layer-area";
const DEPTH_ID: &str = "erosion-layer-depth";
const CODEC_VERSION: u8 = 1;
const BROTLI_QUALITY: u32 = 5;
const BROTLI_WINDOW_BITS: u32 = 22;
const BROTLI_BUFFER: usize = 64 * 1024;
const HEADER_LEN: usize = 1 + 4 + 16;
/// The scales of the codes, part of the keys: 8 steps of area a doubling, 16 steps a square root of a metre.
const AREA_STEPS: u32 = 8;
const DEPTH_SCALE: u32 = 16;

fn channel_bytes(layer: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(12 + layer.len());
    b.extend(CHANNELS_VERSION.to_le_bytes());
    b.extend(AREA_STEPS.to_le_bytes());
    b.extend(DEPTH_SCALE.to_le_bytes());
    b.extend_from_slice(layer);
    b
}

/// The key of a face's drainage area under `spec`.
pub fn area_key(world: &GlobalWorld, res: u32, spec: &LayerSpec, face: Face) -> CacheKey {
    let mut inputs = world.base_key_inputs(Kind::LayerArea, face_tile(face), res, AREA_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, (spec.cells as u32).to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_LAYER_CHANNELS, channel_bytes(&spec.bytes()));
    cache_key(&inputs)
}

/// The key of a tile's depth channels under `spec`.
pub fn depth_key(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<CacheKey, LayerStoreError> {
    let tile = layer_tile_id(&spec.grid, id)?;
    let mut inputs = world.base_key_inputs(Kind::LayerDepth, tile, res, DEPTH_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, (spec.grid.cells as u32).to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_LAYER_CHANNELS, channel_bytes(&spec.bytes()));
    Ok(cache_key(&inputs))
}

/// Each row of `w` bytes as its first byte and the wrapping differences along the row.
fn delta_rows(plane: &[u8], w: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(plane.len());
    for row in plane.chunks_exact(w) {
        out.push(row[0]);
        out.extend(row.windows(2).map(|p| p[1].wrapping_sub(p[0])));
    }
    out
}

fn undelta_rows(raw: &mut [u8], w: usize) {
    for row in raw.chunks_exact_mut(w) {
        for i in 1..w {
            row[i] = row[i].wrapping_add(row[i - 1]);
        }
    }
}

fn compress(raw: &[u8], payload: &mut Vec<u8>) {
    let mut w = brotli::CompressorWriter::new(payload, BROTLI_BUFFER, BROTLI_QUALITY, BROTLI_WINDOW_BITS);
    w.write_all(raw).expect("writing to a vector");
}

/// The stream as exactly `len` bytes: a stream that holds fewer or more is refused, and no more than `len` bytes (and one to detect a longer stream) is ever read.
fn decompress(stream: &[u8], len: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(len);
    let mut r = brotli::Decompressor::new(stream, BROTLI_BUFFER).take(len as u64 + 1);
    r.read_to_end(&mut out).map_err(|e| format!("the stream does not decode: {e}"))?;
    if out.len() != len {
        return Err(format!(
            "the stream decodes to {}{} bytes, not {len}",
            if out.len() > len { "more than " } else { "" },
            out.len().min(len)
        ));
    }
    Ok(out)
}

fn record(world: &GlobalWorld, kind: u8, tile: planet_core::TileId, key: CacheKey, first: u32, body: &[u8], w: usize) -> Record {
    let mut payload = Vec::with_capacity(HEADER_LEN + body.len() / 4);
    payload.push(CODEC_VERSION);
    payload.extend(first.to_le_bytes());
    payload.extend(key.0);
    compress(&delta_rows(body, w), &mut payload);
    Record {
        key,
        kind,
        tile,
        generator_version: planet_generators::global::GLOBAL_VERSION,
        hash_version: planet_core::hash::HASH_VERSION,
        node_set_id: world.node_set_id(),
        payload,
    }
}

/// The record of a face's drainage-area codes (`cells x cells`, `j` outer) under `spec`.
pub fn area_record(world: &GlobalWorld, res: u32, spec: &LayerSpec, face: Face, codes: &[u8]) -> Record {
    assert_eq!(codes.len(), spec.cells * spec.cells, "a face's area holds cells x cells codes");
    record(world, KIND_LAYER_AREA, face_tile(face), area_key(world, res, spec, face), spec.cells as u32, codes, spec.cells)
}

/// The record of a tile's depth channels under `spec`; refused when the result holds none.
pub fn depth_record(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, r: &TileResult) -> Result<Record, LayerStoreError> {
    let t = spec.grid.tile;
    if r.eroded.len() != t * t || r.deposited.len() != t * t {
        return Err(LayerStoreError(format!("tile {}/{}/{} holds no depth channels of {t} x {t} cells", r.id.face.0, r.id.ti, r.id.tj)));
    }
    let mut both = Vec::with_capacity(2 * t * t);
    both.extend_from_slice(&r.eroded);
    both.extend_from_slice(&r.deposited);
    Ok(record(world, KIND_LAYER_DEPTH, layer_tile_id(&spec.grid, r.id)?, depth_key(world, res, spec, r.id)?, t as u32, &both, t))
}

/// Whether the store holds the face's area under the current key: an error naming both keys when it holds the face under another.
pub fn has_area(store: &TileStore, world: &GlobalWorld, res: u32, spec: &LayerSpec, face: Face) -> Result<bool, LayerStoreError> {
    let key = area_key(world, res, spec, face);
    let keys = store.keys_for(KIND_LAYER_AREA, face_tile(face));
    match keys.first() {
        None => Ok(false),
        Some(_) if keys.contains(&key) => Ok(true),
        Some(other) => Err(LayerStoreError(format!(
            "layer area of face {} (record kind {KIND_LAYER_AREA}) key {key}: the store holds this face under key {other}, made for another layer: refused, not mixed",
            face.0
        ))),
    }
}

/// Whether the store holds the tile's depth under the current key (see [`has_area`]).
pub fn has_depth(store: &TileStore, world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<bool, LayerStoreError> {
    let key = depth_key(world, res, spec, id)?;
    let tile = layer_tile_id(&spec.grid, id)?;
    let keys = store.keys_for(KIND_LAYER_DEPTH, tile);
    match keys.first() {
        None => Ok(false),
        Some(_) if keys.contains(&key) => Ok(true),
        Some(other) => Err(LayerStoreError(format!(
            "layer depth {}/{}/{} (record kind {KIND_LAYER_DEPTH}) key {key}: the store holds this tile under key {other}, made for another layer: refused, not mixed",
            id.face.0, id.ti, id.tj
        ))),
    }
}

fn checked<'r>(r: &'r Record, key: CacheKey, first: usize, what: &dyn Fn(String) -> LayerStoreError) -> Result<&'r [u8], LayerStoreError> {
    let p = &r.payload;
    if p.len() < HEADER_LEN || p[0] != CODEC_VERSION || u32::from_le_bytes(p[1..5].try_into().expect("four bytes")) as usize != first {
        return Err(what(format!("a payload of {} bytes that is not {first} cells in codec {CODEC_VERSION}", p.len())));
    }
    if p[5..21] != key.0 {
        return Err(what("its payload names another key than its record".into()));
    }
    Ok(&p[HEADER_LEN..])
}

/// The area codes of `face` (`cells x cells`): `Ok(None)` when the store holds none.
pub fn load_area(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &LayerSpec,
    face: Face,
) -> Result<Option<Vec<u8>>, LayerStoreError> {
    let key = area_key(world, res, spec, face);
    let what = |why: String| LayerStoreError(format!("layer area of face {} (record kind {KIND_LAYER_AREA}) key {key}: {why}", face.0));
    if !has_area(store, world, res, spec, face)? {
        return Ok(None);
    }
    let Some(r) = store.get(KIND_LAYER_AREA, face_tile(face), key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let stream = checked(&r, key, spec.cells, &what)?;
    let mut raw = decompress(stream, spec.cells * spec.cells).map_err(what)?;
    undelta_rows(&mut raw, spec.cells);
    Ok(Some(raw))
}

/// A tile's decoded depth channels, `tile x tile` codes each.
#[derive(Clone, Debug, PartialEq)]
pub struct TileDepth {
    pub tile: usize,
    pub eroded: Vec<u8>,
    pub deposited: Vec<u8>,
}

/// The depth channels of tile `id`: `Ok(None)` when the store holds none.
pub fn load_depth(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    id: PassTile,
) -> Result<Option<TileDepth>, LayerStoreError> {
    let key = depth_key(world, res, spec, id)?;
    let tile = layer_tile_id(&spec.grid, id)?;
    let what = |why: String| {
        LayerStoreError(format!("layer depth {}/{}/{} (record kind {KIND_LAYER_DEPTH}) key {key}: {why}", id.face.0, id.ti, id.tj))
    };
    if !has_depth(store, world, res, spec, id)? {
        return Ok(None);
    }
    let Some(r) = store.get(KIND_LAYER_DEPTH, tile, key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let t = spec.grid.tile;
    let stream = checked(&r, key, t, &what)?;
    let mut raw = decompress(stream, 2 * t * t).map_err(what)?;
    undelta_rows(&mut raw, t);
    let deposited = raw.split_off(t * t);
    Ok(Some(TileDepth { tile: t, eroded: raw, deposited }))
}

/// A bounded least-recently-used cache of decoded values.
struct Lru<K: std::hash::Hash + Eq + Copy, V> {
    capacity: usize,
    map: HashMap<K, Arc<V>>,
    order: VecDeque<K>,
}

impl<K: std::hash::Hash + Eq + Copy, V> Lru<K, V> {
    fn new(capacity: usize) -> Self {
        Lru { capacity: capacity.max(1), map: HashMap::new(), order: VecDeque::new() }
    }

    fn get(&mut self, k: K) -> Option<Arc<V>> {
        let v = self.map.get(&k).cloned()?;
        self.order.retain(|x| *x != k);
        self.order.push_back(k);
        Some(v)
    }

    fn put(&mut self, k: K, v: Arc<V>) {
        if self.map.insert(k, v).is_none() {
            self.order.push_back(k);
        }
        while self.map.len() > self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }
}

/// The read API of the channels over a store: the drainage area and the erosion and deposition depth at a direction. Caches the decoded face areas (a face of 12,288 cells is 151 MB) and
/// tiles (`2 x tile x tile` bytes); safe to share between threads.
pub struct LayerChannels<'a> {
    store: &'a TileStore,
    world: &'a GlobalWorld,
    res: u32,
    layer: LayerSpec,
    tiles: TileLayerSpec,
    areas: Mutex<Lru<u8, Vec<u8>>>,
    depths: Mutex<Lru<(u8, u32, u32), TileDepth>>,
}

impl<'a> LayerChannels<'a> {
    /// A reader over `store` for the layer `layer` (pass 1) and `tiles` (pass 2); the caches hold `faces` faces and `tiles_cached` tiles.
    pub fn new(
        store: &'a TileStore,
        world: &'a GlobalWorld,
        res: u32,
        layer: LayerSpec,
        tiles: TileLayerSpec,
        faces: usize,
        tiles_cached: usize,
    ) -> LayerChannels<'a> {
        LayerChannels { store, world, res, layer, tiles, areas: Mutex::new(Lru::new(faces)), depths: Mutex::new(Lru::new(tiles_cached)) }
    }

    /// The drainage area at `d` as log2 of cells of pass 1's grid (the codes' bilinear mean over the four cells around the point, in codes), or `None` where the store holds no area for
    /// the face.
    pub fn area_log2_cells(&self, d: Vec3) -> Result<Option<f32>, LayerStoreError> {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let n = self.layer.cells;
        let cached = self.areas.lock().expect("area cache").get(face.0);
        let face_codes = {
            if let Some(a) = cached {
                a
            } else {
                let Some(a) = load_area(self.store, self.world, self.res, &self.layer, face)? else { return Ok(None) };
                let a = Arc::new(a);
                self.areas.lock().expect("area cache").put(face.0, a.clone());
                a
            }
        };
        let (x, y) =
            (((s + 1.0) * 0.5 * n as f64 - 0.5).clamp(0.0, (n - 1) as f64), ((t + 1.0) * 0.5 * n as f64 - 0.5).clamp(0.0, (n - 1) as f64));
        let (i, j) = ((x.floor() as usize).min(n - 2), (y.floor() as usize).min(n - 2));
        let (fx, fy) = (x - i as f64, y - j as f64);
        let c = |a: usize, b: usize| f64::from(face_codes[b * n + a]);
        let v = (1.0 - fy) * ((1.0 - fx) * c(i, j) + fx * c(i + 1, j)) + fy * ((1.0 - fx) * c(i, j + 1) + fx * c(i + 1, j + 1));
        Ok(Some((v / f64::from(AREA_STEPS)) as f32))
    }

    /// The drainage area at `d` in cells of pass 1's grid (exp2 of [`Self::area_log2_cells`]; for the cell codes alone see [`area_cells`]).
    pub fn area_cells(&self, d: Vec3) -> Result<Option<f64>, LayerStoreError> {
        Ok(self.area_log2_cells(d)?.map(|l| area_cells((f64::from(l) * f64::from(AREA_STEPS)).round() as u8)))
    }

    /// The erosion and deposition depth in metres at `d`: bilinear in metres between the cell centres of the tile that holds the point (indices clamped to that tile), or `None` where the
    /// store holds no depth for the tile.
    pub fn depth_m(&self, d: Vec3) -> Result<Option<(f32, f32)>, LayerStoreError> {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let (n, tile) = (self.tiles.grid.cells, self.tiles.grid.tile);
        let (x, y) = ((s + 1.0) * 0.5 * n as f64 - 0.5, (t + 1.0) * 0.5 * n as f64 - 0.5);
        let (i, j) = (x.floor().clamp(0.0, (n - 1) as f64) as usize, y.floor().clamp(0.0, (n - 1) as f64) as usize);
        let (ti, tj) = ((i / tile) as u32, (j / tile) as u32);
        let depth = {
            let key = (face.0, ti, tj);
            let cached = self.depths.lock().expect("depth cache").get(key);
            if let Some(a) = cached {
                a
            } else {
                let id = PassTile { face, ti, tj };
                let Some(a) = load_depth(self.store, self.world, self.res, &self.tiles, id)? else { return Ok(None) };
                let a = Arc::new(a);
                self.depths.lock().expect("depth cache").put(key, a.clone());
                a
            }
        };
        let (i0, j0) = (ti as usize * tile, tj as usize * tile);
        let (fx, fy) = ((x - i as f64).clamp(0.0, 1.0), (y - j as f64).clamp(0.0, 1.0));
        let at =
            |plane: &[u8], a: usize, b: usize| f64::from(depth_m(plane[(b.min(j0 + tile - 1) - j0) * tile + (a.min(i0 + tile - 1) - i0)]));
        let bil = |plane: &[u8]| {
            (1.0 - fy) * ((1.0 - fx) * at(plane, i, j) + fx * at(plane, i + 1, j))
                + fy * ((1.0 - fx) * at(plane, i, j + 1) + fx * at(plane, i + 1, j + 1))
        };
        Ok(Some((bil(&depth.eroded) as f32, bil(&depth.deposited) as f32)))
    }
}

/// Copies the drainage areas of `faces` from the pass-1 store `from` to the layer store `to` (the pass-1 store is temporary, STRM-065; the area is part of the layer): a face whose area `to`
/// holds under the current key is skipped. Returns the faces copied. A face without an area in `from` is an error.
pub fn copy_areas(
    from: &Path,
    to: &Path,
    world: &GlobalWorld,
    res: u32,
    spec: &LayerSpec,
    faces: &[Face],
) -> Result<Vec<Face>, LayerStoreError> {
    let err = |e: crate::pack::PackError| LayerStoreError(format!("the layer store {}: {e}", to.display()));
    let source = TileStore::open(from, world.params()).map_err(|e| LayerStoreError(format!("the pass-1 store {}: {e}", from.display())))?;
    let mut writer = PackWriter::open(to, world.params(), DEFAULT_MAX_PACK_BYTES).map_err(err)?;
    let target = TileStore::open(to, world.params()).map_err(err)?;
    let mut copied = Vec::new();
    for &face in faces {
        if copied.contains(&face) || has_area(&target, world, res, spec, face)? {
            continue;
        }
        let codes = load_area(&source, world, res, spec, face)?
            .ok_or_else(|| LayerStoreError(format!("the pass-1 store {} holds no area for face {}", from.display(), face.0)))?;
        writer.append(&area_record(world, res, spec, face, &codes)).map_err(err)?;
        writer.write_index().map_err(err)?;
        copied.push(face);
    }
    writer.finish().map_err(err)?;
    Ok(copied)
}
