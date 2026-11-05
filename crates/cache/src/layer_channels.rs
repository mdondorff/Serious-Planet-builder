//! The erosion layer's two baked channels beside kinds 12 and 13 (GEN-110, GEN-111, STRM-072; m9-bake): the drainage area of pass 1 and the deposited depth of pass 2, kept for the viewer's
//! ground materials and detail.
//!
//! Record kind 14 ([`KIND_LAYER_AREA`]) is one record per cube face filed under the face's root tile: `codec (u8) | cells (u32 LE, pass 1's cells a face edge) | key (16 bytes) |` a brotli
//! stream (quality 5, window 22 bits) of the face's own cells as `u8` codes of the drainage area by multiple flow direction (`round(8 log2(area / cell area))`), row `j` outer, each row as its
//! first byte and the wrapping differences along the row. Record kind 15 ([`KIND_LAYER_DEPTH`]) is one record per tile filed like kind 13: `codec (u8) | tile (u32 LE) | key (16 bytes) |` a
//! brotli stream of the deposited plane of the core (`u8` square-root codes `round(12 sqrt(m))`, 0 to 451.6 m), each row as in kind 14. The depth removed is not stored: it is
//! `max(0, deposited - change)` of the tile's kind 13 ([`removed_depth_m`]). A decoder refuses an unknown codec and a stream
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
/// Version of the channels' layout and codes. History: 1 = the first layout; 2 = the fill-depth scale (GEN-120, STRM-076) joined the channels' key; 3 = the fill code is 16 bits at 4 steps a metre and the post-pass writes it (GEN-121, STRM-077); 4 = the deposited depth is a square-root code, `round(12 sqrt(m))`, 0 to 451.6 m (GEN-111).
pub const CHANNELS_VERSION: u32 = 4;
/// Version of the reader's sampling rule (the bilinear read of the area in codes, the scale of its log2, the depth's read across tile borders and its clamp to the home tile): part of the identity of a
/// [`LayerChannels`], so a change of how a point is read re-keys the tiles whose driver block holds it.
pub const CHANNEL_READER_VERSION: u32 = 2;
const AREA_ID: &str = "erosion-layer-area";
const DEPTH_ID: &str = "erosion-layer-depth";
const CODEC_VERSION: u8 = 1;
const BROTLI_QUALITY: u32 = 5;
const BROTLI_WINDOW_BITS: u32 = 22;
const BROTLI_BUFFER: usize = 64 * 1024;
pub(crate) const HEADER_LEN: usize = 1 + 4 + 16;
/// The scales of the codes, part of the keys: 8 steps of area a doubling; the deposited depth's code is a square root (`depth_code`, the steps a square-root metre in the key).
const AREA_STEPS: u32 = 8;
const DEPTH_SCALE: u32 = 2;

pub(crate) fn channel_bytes(layer: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(12 + layer.len());
    b.extend(CHANNELS_VERSION.to_le_bytes());
    b.extend(AREA_STEPS.to_le_bytes());
    b.extend(DEPTH_SCALE.to_le_bytes());
    b.extend(planet_generators::bake::tile::DEPTH_CODE_K.to_bits().to_le_bytes());
    b.extend(planet_generators::bake::face::FILL_STEPS_PER_M.to_bits().to_le_bytes());
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

pub(crate) fn undelta_rows(raw: &mut [u8], w: usize) {
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
pub(crate) fn decompress(stream: &[u8], len: usize) -> Result<Vec<u8>, String> {
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

pub(crate) fn record(world: &GlobalWorld, kind: u8, tile: planet_core::TileId, key: CacheKey, first: u32, body: &[u8], w: usize) -> Record {
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

/// The record of a tile's deposited depth under `spec`; refused when the result holds none.
pub fn depth_record(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, r: &TileResult) -> Result<Record, LayerStoreError> {
    let t = spec.grid.tile;
    if r.deposited.len() != t * t {
        return Err(LayerStoreError(format!("tile {}/{}/{} holds no deposited depth of {t} x {t} cells", r.id.face.0, r.id.ti, r.id.tj)));
    }
    Ok(record(world, KIND_LAYER_DEPTH, layer_tile_id(&spec.grid, r.id)?, depth_key(world, res, spec, r.id)?, t as u32, &r.deposited, t))
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

pub(crate) fn checked<'r>(
    r: &'r Record,
    key: CacheKey,
    first: usize,
    what: &dyn Fn(String) -> LayerStoreError,
) -> Result<&'r [u8], LayerStoreError> {
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

/// DIAGNOSTIC, never for the viewer or the generation path (STRM-075): the codes of a channel record of `cells x cells` (kind 14: a face's area; kind 15: `cells` the tile's side) decoded **without the key
/// check**, for tools that inspect a store made under the keys of another build (a measurement, a probe). The header's codec and cell count are still checked and the stream must decode to exactly the
/// expected bytes. A test pins that nothing but the probes calls it; the runtime reads through [`LayerChannels`], which refuses a channel of another layer by name.
pub fn diagnostic_decode_unchecked(r: &Record, cells: usize) -> Result<Vec<u8>, String> {
    let p = &r.payload;
    if p.len() < HEADER_LEN || p[0] != CODEC_VERSION || u32::from_le_bytes(p[1..5].try_into().expect("four bytes")) as usize != cells {
        return Err(format!("a payload of {} bytes that is not {cells} cells in codec {CODEC_VERSION}", p.len()));
    }
    let mut raw = decompress(&p[HEADER_LEN..], cells * cells)?;
    undelta_rows(&mut raw, cells);
    Ok(raw)
}

/// A window of a face's decoded area plane: columns `x0..x0 + w` of rows `y0..y0 + h` (`cells x cells` in all), row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaWindow {
    pub x0: usize,
    pub y0: usize,
    pub w: usize,
    pub h: usize,
    pub codes: Vec<u8>,
}

impl AreaWindow {
    /// The code at cell `(i, j)` of the face, which the window must hold.
    pub fn at(&self, i: usize, j: usize) -> u8 {
        self.codes[(j - self.y0) * self.w + (i - self.x0)]
    }
}

/// The window of `face`'s area codes: rows `rows` and columns `cols`, streamed so that only the window is kept (the face is `cells x cells` bytes: 151 MB at 12,288 cells). The stream is read from its start
/// to the last row of the window (a brotli stream cannot be entered in the middle), so a cold window costs up to one face's decode time and no more memory than the window; `Ok(None)` when the store holds
/// no area for the face. The record's header and key are checked as in [`load_area`].
pub fn load_area_window(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &LayerSpec,
    face: Face,
    rows: std::ops::Range<usize>,
    cols: std::ops::Range<usize>,
) -> Result<Option<AreaWindow>, LayerStoreError> {
    let key = area_key(world, res, spec, face);
    let what = |why: String| LayerStoreError(format!("layer area of face {} (record kind {KIND_LAYER_AREA}) key {key}: {why}", face.0));
    if !has_area(store, world, res, spec, face)? {
        return Ok(None);
    }
    let Some(r) = store.get(KIND_LAYER_AREA, face_tile(face), key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let stream = checked(&r, key, spec.cells, &what)?;
    let codes = stream_window(stream, spec.cells, rows.clone(), cols.clone()).map_err(what)?;
    Ok(Some(AreaWindow { x0: cols.start, y0: rows.start, w: cols.len(), h: rows.len(), codes }))
}

/// The window of a delta-coded `cells x cells` plane in a raw brotli stream (no header or key check: the callers have made them), row by row: the rows before the window are read and dropped, the rows of the window are undone to codes and cut to the columns.
pub fn stream_window(stream: &[u8], cells: usize, rows: std::ops::Range<usize>, cols: std::ops::Range<usize>) -> Result<Vec<u8>, String> {
    if rows.end > cells || cols.end > cells || rows.is_empty() || cols.is_empty() {
        return Err(format!("the window rows {rows:?} columns {cols:?} is not inside {cells} cells"));
    }
    let mut r = brotli::Decompressor::new(stream, BROTLI_BUFFER);
    let mut row = vec![0u8; cells];
    let mut out = Vec::with_capacity(rows.len() * cols.len());
    for j in 0..rows.end {
        r.read_exact(&mut row).map_err(|e| format!("the stream does not decode to {cells} rows: {e}"))?;
        if j >= rows.start {
            for i in 1..cols.end {
                row[i] = row[i].wrapping_add(row[i - 1]);
            }
            out.extend_from_slice(&row[cols.clone()]);
        }
    }
    Ok(out)
}

/// A tile's decoded deposited depth, `tile x tile` codes of 1 m.
#[derive(Clone, Debug, PartialEq)]
pub struct TileDepth {
    pub tile: usize,
    pub deposited: Vec<u8>,
}

/// The caller chooses which change to pass. With keep-pits (GEN-120) the STORED change of kind 13 includes the pits, so a pit would count as removal: [`LayerChannels::erosion_depth_m`] passes the
/// stored change plus the pit depth (the runtime's filled change), so a pit is not erosion (docs/learnings.md, 2026-11-05).
/// The depth removed at a cell in metres: `max(0, deposited - change)`, the deposited depth and the layer's change (kind 13) at the cell. Exact where pass 1 did not raise the cell; where it
/// did (uplift, a filled pit: 15 % of mountain land and 35 % of hills in seed 14's samples) it is short by the raise, and where the deposited depth is at the code's end (451.6 m) it is clipped.
pub fn removed_depth_m(deposited_m: f32, change_m: f32) -> f32 {
    (deposited_m - change_m).max(0.0)
}

/// The deposited depth of tile `id`: `Ok(None)` when the store holds none.
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
    let mut raw = decompress(stream, t * t).map_err(what)?;
    undelta_rows(&mut raw, t);
    Ok(Some(TileDepth { tile: t, deposited: raw }))
}

/// A least-recently-used cache of decoded values bounded by their bytes: it evicts from the cold end while the total is over the cap, but always keeps the entry just put (a face larger than the cap is
/// held alone, never refused).
struct Lru<K: std::hash::Hash + Eq + Copy, V> {
    cap_bytes: usize,
    used: usize,
    weigh: fn(&V) -> usize,
    map: HashMap<K, Arc<V>>,
    order: VecDeque<K>,
}

impl<K: std::hash::Hash + Eq + Copy, V> Lru<K, V> {
    fn new(cap_bytes: usize, weigh: fn(&V) -> usize) -> Self {
        Lru { cap_bytes, used: 0, weigh, map: HashMap::new(), order: VecDeque::new() }
    }

    fn get(&mut self, k: K) -> Option<Arc<V>> {
        let v = self.map.get(&k).cloned()?;
        self.order.retain(|x| *x != k);
        self.order.push_back(k);
        Some(v)
    }

    fn put(&mut self, k: K, v: Arc<V>) {
        self.used += (self.weigh)(&v);
        match self.map.insert(k, v) {
            None => self.order.push_back(k),
            Some(old) => {
                self.used -= (self.weigh)(&old);
                self.order.retain(|x| *x != k);
                self.order.push_back(k);
            }
        }
        // Cold end first; the entry just put is last in the order, so it is never the one evicted while another remains.
        while self.used > self.cap_bytes && self.order.len() > 1 {
            let Some(old) = self.order.pop_front() else { break };
            if let Some(v) = self.map.remove(&old) {
                self.used -= (self.weigh)(&v);
            }
        }
    }

    fn bytes(&self) -> usize {
        self.used
    }
}

/// The caches of [`LayerChannels`]: the windows of kind 14 (a window of `area_window + 1` cells a side, cut from a face of 151 MB at 12,288 cells) and the tiles of kind 15 (`tile^2` bytes each), each under a byte cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelCaches {
    /// The most bytes of decoded area windows held.
    pub area_bytes: usize,
    /// The cells a side that a window advances (a window holds one more row and column, so a bilinear read never leaves it).
    pub area_window: usize,
    /// The most bytes of decoded depth tiles held. The pit depth and the change come from the layer's own decoded tiles (shared with the heights), not from a cache here.
    pub tile_bytes: usize,
}

impl ChannelCaches {
    /// 256-cell windows (about 208 km of the 12,288-cell face, 66 KB each) under 16 MB, and 256 MB of depth tiles.
    pub const DEFAULT: ChannelCaches = ChannelCaches { area_bytes: 16 << 20, area_window: 256, tile_bytes: 256 << 20 };
}

/// What a [`LayerChannels`] has done so far: the reads served from its caches, the windows and tiles it decoded, and the time those decodes took (the measurement of ground-channels-2b PR 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChannelStats {
    pub window_hits: u64,
    pub window_decodes: u64,
    pub window_decode_us: u64,
    pub tile_hits: u64,
    pub tile_decodes: u64,
    pub tile_decode_us: u64,
}

/// The read API of the channels over a store: the drainage area and the erosion and deposition depth at a direction. It owns its handles (the store and the world are shared, so its lifetime is
/// the viewer's world's) and caches windows of the decoded face areas (a whole face of 12,288 cells would be 151 MB) and tiles (`tile x tile` bytes) under byte caps; safe to share between threads. A miss
/// decodes under a lock of its own, so concurrent workers asking for one window decode it once.
pub struct LayerChannels {
    store: Arc<TileStore>,
    world: Arc<GlobalWorld>,
    /// The resolution of the keys of the areas (pass 1's cells a face edge in `bake_layer`) and of the depths (pass 2's cells a face edge).
    area_res: u32,
    depth_res: u32,
    layer: LayerSpec,
    tiles: TileLayerSpec,
    area_window: usize,
    areas: Mutex<Lru<(u8, u32, u32), AreaWindow>>,
    depths: Mutex<Lru<(u8, u32, u32), TileDepth>>,
    /// The layer's decoded tiles (the viewer's own, shared with the heights): the change with the pit depth added back and the pit codes of kind 17.
    tile_source: Arc<dyn planet_generators::erosion_layer::LayerSource>,
    area_load: Mutex<()>,
    depth_load: Mutex<()>,
    stats: Mutex<ChannelStats>,
    identity: u64,
}

impl LayerChannels {
    /// A reader over `store` for the layer `layer` (pass 1) and `tiles` (pass 2); the decoded caches hold at most `caches` bytes (one face or tile is always kept).
    #[allow(clippy::too_many_arguments)] // the two key resolutions, the two specs, the layer's tile source and the caps are all the reader's inputs
    pub fn new(
        store: Arc<TileStore>,
        world: Arc<GlobalWorld>,
        area_res: u32,
        depth_res: u32,
        layer: LayerSpec,
        tiles: TileLayerSpec,
        tile_source: Arc<dyn planet_generators::erosion_layer::LayerSource>,
        caches: ChannelCaches,
    ) -> LayerChannels {
        // The identity of the channels: the keys of a face's area and of a tile's depth under these specs, which carry the layer's parameters, the channels' version and their scales.
        let mut parts: Vec<u64> = area_key(&world, area_res, &layer, Face(0))
            .0
            .chunks(8)
            .map(|c| u64::from_le_bytes(c.try_into().expect("eight bytes")))
            .collect();
        // (A tile spec whose first tile has no key is a spec the reader cannot serve anyway: its area keys alone then name the channels.)
        match depth_key(&world, depth_res, &tiles, PassTile { face: Face(0), ti: 0, tj: 0 }) {
            Ok(k) => parts.extend(k.0.chunks(8).map(|c| u64::from_le_bytes(c.try_into().expect("eight bytes")))),
            Err(_) => parts.push(0x6e6f_2064_6570_7468),
        }
        // The fill's key carries the fill scale (GEN-120: the code is not decided), so another scale is another identity.
        match crate::layer_fill::fill_key(&world, depth_res, &tiles, PassTile { face: Face(0), ti: 0, tj: 0 }) {
            Ok(k) => parts.extend(k.0.chunks(8).map(|c| u64::from_le_bytes(c.try_into().expect("eight bytes")))),
            Err(_) => parts.push(0x6e6f_2066_696c_6c00),
        }
        parts.push(u64::from(CHANNEL_READER_VERSION));
        let identity = planet_core::hash::hash_u64s(0x6964_656e, &parts);
        LayerChannels {
            identity,
            store,
            world,
            area_res,
            depth_res,
            layer,
            tiles,
            area_window: caches.area_window.max(1),
            areas: Mutex::new(Lru::new(caches.area_bytes, |v: &AreaWindow| v.codes.len())),
            depths: Mutex::new(Lru::new(caches.tile_bytes, |v: &TileDepth| v.deposited.len())),
            tile_source,
            area_load: Mutex::new(()),
            depth_load: Mutex::new(()),
            stats: Mutex::new(ChannelStats::default()),
        }
    }

    /// The faces whose drainage area the store holds under the current keys (an error when it holds one under other keys): empty for a layer made before the channels or under other keys, which
    /// the viewer's `--channels on` refuses.
    pub fn faces_with_area(&self) -> Result<Vec<Face>, LayerStoreError> {
        let mut out = Vec::new();
        for f in 0..6 {
            if has_area(&self.store, &self.world, self.area_res, &self.layer, Face(f))? {
                out.push(Face(f));
            }
        }
        Ok(out)
    }

    /// What the reader has done so far, and the bytes its two caches hold now (area windows, depth tiles).
    pub fn stats(&self) -> (ChannelStats, usize, usize) {
        let s = *self.stats.lock().expect("stats");
        (s, self.areas.lock().expect("area cache").bytes(), self.depths.lock().expect("depth cache").bytes())
    }

    /// The window `(wi, wj)` of `face`'s area plane through the cache (`None` where the store holds no area for the face).
    fn area_window_of(&self, face: Face, wi: u32, wj: u32) -> Result<Option<Arc<AreaWindow>>, LayerStoreError> {
        let key = (face.0, wi, wj);
        if let Some(a) = self.areas.lock().expect("area cache").get(key) {
            self.stats.lock().expect("stats").window_hits += 1;
            return Ok(Some(a));
        }
        let _one = self.area_load.lock().expect("area load");
        // Another worker may have decoded it while this one waited.
        if let Some(a) = self.areas.lock().expect("area cache").get(key) {
            self.stats.lock().expect("stats").window_hits += 1;
            return Ok(Some(a));
        }
        let (n, step) = (self.layer.cells, self.area_window);
        let (x0, y0) = (wi as usize * step, wj as usize * step);
        let (cols, rows) = (x0..(x0 + step + 1).min(n), y0..(y0 + step + 1).min(n));
        let started = std::time::Instant::now();
        let Some(a) = load_area_window(&self.store, &self.world, self.area_res, &self.layer, face, rows, cols)? else { return Ok(None) };
        let a = Arc::new(a);
        self.areas.lock().expect("area cache").put(key, a.clone());
        let mut s = self.stats.lock().expect("stats");
        s.window_decodes += 1;
        s.window_decode_us += started.elapsed().as_micros() as u64;
        Ok(Some(a))
    }

    /// The drainage area at `d` as log2 of cells of pass 1's grid (the codes' bilinear mean over the four cells around the point, in codes), or `None` where the store holds no area for
    /// the face.
    pub fn area_log2_cells(&self, d: Vec3) -> Result<Option<f32>, LayerStoreError> {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let n = self.layer.cells;
        let (x, y) =
            (((s + 1.0) * 0.5 * n as f64 - 0.5).clamp(0.0, (n - 1) as f64), ((t + 1.0) * 0.5 * n as f64 - 0.5).clamp(0.0, (n - 1) as f64));
        let (i, j) = ((x.floor() as usize).min(n - 2), (y.floor() as usize).min(n - 2));
        let (fx, fy) = (x - i as f64, y - j as f64);
        // The four cells around the point lie in one window (a window holds the cell past its step), whatever the face's size.
        let Some(win) = self.area_window_of(face, (i / self.area_window) as u32, (j / self.area_window) as u32)? else { return Ok(None) };
        let c = |a: usize, b: usize| f64::from(win.at(a, b));
        let v = (1.0 - fy) * ((1.0 - fx) * c(i, j) + fx * c(i + 1, j)) + fy * ((1.0 - fx) * c(i, j + 1) + fx * c(i + 1, j + 1));
        Ok(Some((v / f64::from(AREA_STEPS)) as f32))
    }

    /// The drainage area at `d` in cells of pass 1's grid (exp2 of [`Self::area_log2_cells`]; for the cell codes alone see [`area_cells`]).
    pub fn area_cells(&self, d: Vec3) -> Result<Option<f64>, LayerStoreError> {
        Ok(self.area_log2_cells(d)?.map(|l| area_cells((f64::from(l) * f64::from(AREA_STEPS)).round() as u8)))
    }

    /// The deposited depth in metres at `d`: bilinear in metres between the cell centres around the point, across tile borders (a neighbour tile without a record is read as
    /// the home tile's nearest cell), or `None` where the store holds no depth for the tile that holds the point.
    pub fn depth_m(&self, d: Vec3) -> Result<Option<f32>, LayerStoreError> {
        self.tiled_bilinear(d, &|face, ti, tj, px, py| {
            Ok(self.depth_tile(face, ti, tj)?.map(|t| f64::from(depth_m(t.deposited[py * self.tiles.grid.tile + px]))))
        })
    }

    /// The pit depth in metres at `d` (kind 17: how far the ground lies below the flood of the final surface, GEN-120), read like [`Self::depth_m`]; `None` where the store holds no fill for the tile.
    pub fn fill_depth_m(&self, d: Vec3) -> Result<Option<f32>, LayerStoreError> {
        self.tiled_bilinear(d, &|face, ti, tj, px, py| {
            let Some(lt) = self.tile_source.tile(face, ti, tj).map_err(LayerStoreError)? else { return Ok(None) };
            // A layer tile made before keep-pits holds no pit codes: none there.
            Ok(lt.result().fill.get(py * self.tiles.grid.tile + px).map(|c| planet_generators::bake::face::fill_m(*c)))
        })
    }

    /// The erosion depth in metres at `d`: per cell `max(0, deposited - (change + fill))` (the deposited depth of kind 15, the change of kind 13 and the pit depth of kind 17, a tile without a fill
    /// counting 0), read bilinearly in metres like [`Self::depth_m`]; `None` where the store holds no depth or no layer tile for the tile that holds the point.
    pub fn erosion_depth_m(&self, d: Vec3) -> Result<Option<f32>, LayerStoreError> {
        self.tiled_bilinear(d, &|face, ti, tj, px, py| {
            let p = py * self.tiles.grid.tile + px;
            let (Some(dep), Some(lt)) = (self.depth_tile(face, ti, tj)?, self.tile_source.tile(face, ti, tj).map_err(LayerStoreError)?)
            else {
                return Ok(None);
            };
            // The layer's cell is the change with the pit depth added back (the filled look the viewer draws), so a pit is not counted as removal.
            Ok(Some(f64::from(removed_depth_m(depth_m(dep.deposited[p]), lt.cell(px, py)))))
        })
    }

    /// The bilinear read, in metres, of a per-cell value of the pass-2 grid at `d`: `at(face, ti, tj, px, py)` gives the value of core cell `(px, py)` of tile `(ti, tj)`, or `Ok(None)` where the store holds no
    /// record of that tile. The four cells around the point are read across tile borders (a neighbour tile without a record is read as the home tile's nearest cell), and `None` is returned where the
    /// tile that holds the point has no record.
    fn tiled_bilinear(
        &self,
        d: Vec3,
        at: &dyn Fn(Face, u32, u32, usize, usize) -> Result<Option<f64>, LayerStoreError>,
    ) -> Result<Option<f32>, LayerStoreError> {
        let (face, s, t) = direction_to_face(&TangentWarp, d);
        let (n, tile) = (self.tiles.grid.cells, self.tiles.grid.tile);
        let (x, y) = ((s + 1.0) * 0.5 * n as f64 - 0.5, (t + 1.0) * 0.5 * n as f64 - 0.5);
        let (i, j) = (x.floor().clamp(0.0, (n - 1) as f64) as usize, y.floor().clamp(0.0, (n - 1) as f64) as usize);
        let (fx, fy) = ((x - i as f64).clamp(0.0, 1.0), (y - j as f64).clamp(0.0, 1.0));
        let (hi, hj) = (i / tile, j / tile);
        // The home tile's record decides: without it there is nothing to read.
        if at(face, hi as u32, hj as u32, 0, 0)?.is_none() {
            return Ok(None);
        }
        // The four cells around the point: one that lies in the next tile is read from it, so no seam shows at a tile border; a cell past the face's edge, or in a tile without a
        // record, is read as the nearest cell of the home tile.
        let mut cells = [0.0f64; 4];
        for (k, (a, b)) in [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)].into_iter().enumerate() {
            let (a, b) = (a.min(n - 1), b.min(n - 1));
            let (ta, tb) = (a / tile, b / tile);
            let other = if (ta, tb) == (hi, hj) { None } else { at(face, ta as u32, tb as u32, a - ta * tile, b - tb * tile)? };
            cells[k] = match other {
                Some(v) => v,
                None => at(
                    face,
                    hi as u32,
                    hj as u32,
                    a.clamp(hi * tile, hi * tile + tile - 1) - hi * tile,
                    b.clamp(hj * tile, hj * tile + tile - 1) - hj * tile,
                )?
                .expect("the home tile holds a record"),
            };
        }
        let v = (1.0 - fy) * ((1.0 - fx) * cells[0] + fx * cells[1]) + fy * ((1.0 - fx) * cells[2] + fx * cells[3]);
        Ok(Some(v as f32))
    }

    /// The decoded depth channels of a tile of `face` through the cache (`None` where the store holds none).
    fn depth_tile(&self, face: Face, ti: u32, tj: u32) -> Result<Option<Arc<TileDepth>>, LayerStoreError> {
        let key = (face.0, ti, tj);
        if let Some(a) = self.depths.lock().expect("depth cache").get(key) {
            self.stats.lock().expect("stats").tile_hits += 1;
            return Ok(Some(a));
        }
        let _one = self.depth_load.lock().expect("depth load");
        if let Some(a) = self.depths.lock().expect("depth cache").get(key) {
            self.stats.lock().expect("stats").tile_hits += 1;
            return Ok(Some(a));
        }
        let started = std::time::Instant::now();
        let Some(a) = load_depth(&self.store, &self.world, self.depth_res, &self.tiles, PassTile { face, ti, tj })? else {
            return Ok(None);
        };
        let a = Arc::new(a);
        self.depths.lock().expect("depth cache").put(key, a.clone());
        let mut s = self.stats.lock().expect("stats");
        s.tile_decodes += 1;
        s.tile_decode_us += started.elapsed().as_micros() as u64;
        Ok(Some(a))
    }
}

impl planet_generators::channels::ChannelSource for LayerChannels {
    fn identity(&self) -> u64 {
        self.identity
    }

    fn area_log2_cells(&self, d: Vec3) -> Result<Option<f32>, String> {
        LayerChannels::area_log2_cells(self, d).map_err(|e| e.0)
    }

    fn deposited_depth_m(&self, d: Vec3) -> Result<Option<f32>, String> {
        self.depth_m(d).map_err(|e| e.0)
    }

    fn fill_depth_m(&self, d: Vec3) -> Result<Option<f32>, String> {
        LayerChannels::fill_depth_m(self, d).map_err(|e| e.0)
    }

    fn erosion_depth_m(&self, d: Vec3) -> Result<Option<f32>, String> {
        LayerChannels::erosion_depth_m(self, d).map_err(|e| e.0)
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
