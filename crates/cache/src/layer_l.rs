//! The local change L of the new chain as kind 13 records (STRM-080 family, GEN-156, architect D16): the tile record of layer version 3, codec 3.
//!
//! The same kind and the same counts stream as the previous layer (0.25 m `i16` counts, row deltas, brotli quality 5, the encoding item 14 measured), a different header and a different key:
//! - the stats block is K7's: land cells, the cells the burn-in lowered, the cells stored clamped, and the pits left after the pass (cells, p50, p99, maximum);
//! - the key carries the base's identity (`base_id`: a changed base is another layer, A3 section 3.2) and the identity of the other inputs (`inputs_id`: the regional change, the channels), the
//!   tile grid, K7's parameters and the versions of the kernels; the implementation id differs from the previous layer's, so the two never share a key;
//! - there is no fill channel: the surface is `B + L` and a basin is a lake, not a depth added back.
//!
//! The payload is `codec (u8) | tile (u32 LE, core cells a side) | stats (land, burned, clamped, pit cells u64 LE; pit p50, p99, max f64 LE) | key (16 bytes) | water section length (u32 LE) | water section |` the brotli stream.
//! The water section (architect D17/D18) is optional: empty for a tile that touches no lake, and for every record until the bake writer fills it with the tile's lake water mask (runs of wet cells labelled with an
//! index into the tile's basin-id list). This version reserves it and carries it as opaque bytes; its inner layout is the writer's to define without a version bump.
//! A record of the previous version (codec 2) is refused naming both versions. A store written by other parameters, another base or another layer version is refused, never mixed.

use std::io::Write;

use planet_generators::bake::k7::K7Params;
use planet_generators::bake::tile::{dequantise, quantise, TileGrid, TileId as PassTile, QUANT_STEP_M};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_RESOLUTION, PARAM_TILE_LAYER};
use crate::layer_store::LayerStoreError;
use crate::layer_tiles::{layer_tile_id, pack_counts, unpack_counts, TileLayerSpec, KIND_LAYER_TILE, TILE_LAYER_VERSION};
use crate::pack::{Record, TileStore};

/// The codec byte of an L record.
pub const L_CODEC_VERSION: u8 = 3;
/// The codec byte of the previous layer's records (kept to name it in a refusal).
const PREVIOUS_CODEC_VERSION: u8 = 2;
const IMPLEMENTATION_ID: &str = "layer-l-tile";
const BROTLI_QUALITY: u32 = 5;
const BROTLI_WINDOW_BITS: u32 = 22;
const BROTLI_BUFFER: usize = 64 * 1024;
/// The fixed part: codec, tile, stats, key and the length of the water section.
const HEADER_LEN: usize = 1 + 4 + 7 * 8 + 16 + 4;
/// The most a water section may hold: a run list of a tile of 1,024 cells a side is far below it; a larger length is a damaged record, not a request to reserve memory.
pub const MAX_WATER_SECTION: usize = 16 << 20;

/// What an L layer is made of besides the bake it is beside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LTileSpec {
    pub grid: TileGrid,
    /// K7's parameters (pass 2's parameters and the flood it runs).
    pub k7: K7Params,
    /// The identity of the base `B` the change is against.
    pub base_id: u64,
    /// The identity of the other inputs of K7 (the regional change, the channels, the world seed's streams).
    pub inputs_id: u64,
}

impl LTileSpec {
    fn bytes(&self) -> Vec<u8> {
        // The pass-2 parameters and the grid are written as the previous layer wrote them (the same `TileLayerSpec::bytes`, with the version of this layer in front), then what is new.
        let as_tile = TileLayerSpec {
            grid: self.grid,
            params: self.k7.pass2,
            inputs_id: self.inputs_id,
            uses_factor: false,
            rock: 0,
            faces: [0; 16],
        };
        let mut b = as_tile.bytes();
        b.push(u8::from(self.k7.canonical_flood));
        // The river width law of the burn-in (GEN-165): a and b are bake parameters, so another law is another layer.
        b.extend(self.k7.width.a.to_bits().to_le_bytes());
        b.extend(self.k7.width.b.to_bits().to_le_bytes());
        b.extend(self.base_id.to_le_bytes());
        b
    }
}

/// K7's stats of a tile (what the old stats block held, replaced).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LTileStats {
    /// Land cells of the core.
    pub land_cells: u64,
    /// The cells the burn-in lowered (K7Result::burned).
    pub burned_cells: u64,
    /// The pits after the whole pass: cells at the pit limit or deeper, and the percentiles of their depth.
    pub pit_cells: u64,
    pub pit_p50_m: f64,
    pub pit_p99_m: f64,
    pub pit_max_m: f64,
}

/// A tile of L as read: the core's change in metres (on the 0.25 m grid), K7's stats and the cells that were stored clamped.
#[derive(Clone, Debug, PartialEq)]
pub struct LTile {
    pub id: PassTile,
    pub core: Vec<f32>,
    pub stats: LTileStats,
    pub clamped_cells: u64,
    /// The water section as stored: empty, or the tile's lake water mask (opaque here).
    pub water: Vec<u8>,
}

/// The key of a tile of L.
pub fn l_tile_key(world: &GlobalWorld, res: u32, spec: &LTileSpec, id: PassTile) -> Result<CacheKey, LayerStoreError> {
    let tile = layer_tile_id(&spec.grid, id)?;
    let mut inputs = world.base_key_inputs(Kind::LayerTile, tile, res, IMPLEMENTATION_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, (spec.grid.tile as u32).to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_TILE_LAYER, spec.bytes());
    Ok(cache_key(&inputs))
}

/// The record of a tile of L: `core` is the tile's change against `B` in metres (K7Result::change), `stats` K7's, `water` the water section (empty for a tile that touches no lake). Cells beyond the
/// quantisation range are stored clamped and counted.
pub fn l_tile_record(
    world: &GlobalWorld,
    res: u32,
    spec: &LTileSpec,
    id: PassTile,
    core: &[f32],
    stats: &LTileStats,
    water: &[u8],
) -> Result<Record, LayerStoreError> {
    assert!(water.len() <= MAX_WATER_SECTION, "the water section of a tile is at most {MAX_WATER_SECTION} bytes");
    let t = spec.grid.tile;
    assert_eq!(core.len(), t * t, "a tile of L holds tile x tile cells");
    assert!(core.iter().all(|v| v.is_finite()), "the change of a tile is finite");
    assert!(stats.pit_p50_m.is_finite() && stats.pit_p99_m.is_finite() && stats.pit_max_m.is_finite(), "the pit depths are finite");
    let key = l_tile_key(world, res, spec, id)?;
    let (lo, hi) = (f32::from(i16::MIN) * QUANT_STEP_M, f32::from(i16::MAX) * QUANT_STEP_M);
    let clamped = core.iter().filter(|&&v| v < lo - QUANT_STEP_M / 2.0 || v > hi + QUANT_STEP_M / 2.0).count() as u64;
    let raw = pack_counts(&quantise(core), t);
    let mut payload = Vec::with_capacity(HEADER_LEN + raw.len() / 2);
    payload.push(L_CODEC_VERSION);
    payload.extend((t as u32).to_le_bytes());
    for v in [stats.land_cells, stats.burned_cells, clamped, stats.pit_cells] {
        payload.extend(v.to_le_bytes());
    }
    for v in [stats.pit_p50_m, stats.pit_p99_m, stats.pit_max_m] {
        payload.extend(v.to_bits().to_le_bytes());
    }
    payload.extend(key.0);
    payload.extend((water.len() as u32).to_le_bytes());
    payload.extend_from_slice(water);
    {
        let mut w = brotli::CompressorWriter::new(&mut payload, BROTLI_BUFFER, BROTLI_QUALITY, BROTLI_WINDOW_BITS);
        w.write_all(&raw).expect("writing to a vector");
    }
    Ok(Record {
        key,
        kind: KIND_LAYER_TILE,
        tile: layer_tile_id(&spec.grid, id)?,
        generator_version: planet_generators::global::GLOBAL_VERSION,
        hash_version: planet_core::hash::HASH_VERSION,
        node_set_id: world.node_set_id(),
        payload,
    })
}

/// The stored tile of L: `Ok(None)` when the store holds nothing for the tile; an error naming the record and both versions when the store holds a record of the previous layer version (codec 2) for it,
/// an error when it holds the tile under another key (another base, other parameters or kernels: refused, not mixed) or when the record is damaged.
pub fn load_l_tile(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &LTileSpec,
    id: PassTile,
) -> Result<Option<LTile>, LayerStoreError> {
    let key = l_tile_key(world, res, spec, id)?;
    let tile = layer_tile_id(&spec.grid, id)?;
    let what =
        |why: String| LayerStoreError(format!("L tile {}/{}/{} (record kind {KIND_LAYER_TILE}) key {key}: {why}", id.face.0, id.ti, id.tj));
    if let Some(t) = load_l_tile_keyed(store, spec, id, key)? {
        return Ok(Some(t));
    }
    {
        let Some(other) = store.keys_for(KIND_LAYER_TILE, tile).first().copied() else { return Ok(None) };
        let found = store.get(KIND_LAYER_TILE, tile, other).ok().flatten().and_then(|r| r.payload.first().copied());
        return Err(what(match found {
            Some(PREVIOUS_CODEC_VERSION) => format!(
                "the store holds this tile under key {other} as a record of the previous layer (codec {PREVIOUS_CODEC_VERSION}, layer version 2, with a fill channel); this build reads codec {L_CODEC_VERSION} (layer version {TILE_LAYER_VERSION}): refused, rebuild the layer"
            ),
            Some(c) => format!(
                "the store holds this tile under key {other} (codec {c}), made for another base, other parameters or another kernel version; this build reads codec {L_CODEC_VERSION} (layer version {TILE_LAYER_VERSION}): refused, not mixed"
            ),
            None => format!("the store holds this tile under key {other} that cannot be read: refused, not mixed"),
        }));
    }
}

/// The L tile record stored under `key` (no check that this build makes `key` for the tile): `Ok(None)` when the store holds no record under it. [`load_l_tile`] reads under the key this build makes;
/// a diagnostic that reads a store of another build (the river gate's `--no-key-check`, BUILD-055) reads under the key the store's commit names.
pub fn load_l_tile_keyed(store: &TileStore, spec: &LTileSpec, id: PassTile, key: CacheKey) -> Result<Option<LTile>, LayerStoreError> {
    let tile = layer_tile_id(&spec.grid, id)?;
    let what =
        |why: String| LayerStoreError(format!("L tile {}/{}/{} (record kind {KIND_LAYER_TILE}) key {key}: {why}", id.face.0, id.ti, id.tj));
    let Some(r) = store.get(KIND_LAYER_TILE, tile, key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let p = &r.payload;
    let t = spec.grid.tile;
    if p.len() < HEADER_LEN || p[0] != L_CODEC_VERSION {
        let found = p.first().map_or("an empty payload".to_string(), |c| format!("codec {c}"));
        return Err(what(format!(
            "a payload of {} bytes with {found}; this build reads codec {L_CODEC_VERSION} (layer version {TILE_LAYER_VERSION})",
            p.len()
        )));
    }
    let u32_at = |a: usize| u32::from_le_bytes(p[a..a + 4].try_into().expect("four bytes"));
    let u64_at = |a: usize| u64::from_le_bytes(p[a..a + 8].try_into().expect("eight bytes"));
    let f64_at = |a: usize| f64::from_bits(u64_at(a));
    if u32_at(1) as usize != t {
        return Err(what(format!("it holds a tile of {} cells a side, the layout says {t}", u32_at(1))));
    }
    if p[5 + 56..5 + 56 + 16] != key.0 {
        return Err(what("its payload names another key than its record".into()));
    }
    let stats = LTileStats {
        land_cells: u64_at(5),
        burned_cells: u64_at(13),
        pit_cells: u64_at(29),
        pit_p50_m: f64_at(37),
        pit_p99_m: f64_at(45),
        pit_max_m: f64_at(53),
    };
    let clamped_cells = u64_at(21);
    if !(stats.pit_p50_m.is_finite() && stats.pit_p99_m.is_finite() && stats.pit_max_m.is_finite())
        || clamped_cells > (t * t) as u64
        || stats.land_cells > (t * t) as u64
    {
        return Err(what("its stats block holds a number that is not finite or exceeds the tile's cells".into()));
    }
    let water_len = u32_at(HEADER_LEN - 4) as usize;
    if water_len > MAX_WATER_SECTION || water_len > p.len() - HEADER_LEN {
        return Err(what(format!(
            "its water section claims {water_len} bytes, the record has {} after the header (at most {MAX_WATER_SECTION})",
            p.len() - HEADER_LEN
        )));
    }
    let water = p[HEADER_LEN..HEADER_LEN + water_len].to_vec();
    let counts = unpack_counts(&p[HEADER_LEN + water_len..], t).map_err(what)?;
    Ok(Some(LTile { id, core: dequantise(&counts), stats, clamped_cells, water }))
}
