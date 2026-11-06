//! The pit depth of the erosion layer (GEN-120, GEN-121, STRM-076, STRM-077; m9 keep-pits and keep-pits-seams): the layer keeps the pits of the carved surface in its change and records how deep each
//! lies below the planet-wide spill level of the final surface, so that the runtime adds the depth back (the filled look) until lake water and a pit-tolerance rule exist.
//!
//! Record kind 17 ([`KIND_LAYER_FILL`]) holds a tile's depth, one 16-bit code a core cell, `round(m * FILL_STEPS_PER_M)` (0.25 m steps), filed beside kind 15; the post-pass of
//! [`crate::layer_fillpass`] writes it. Record kind 18 ([`KIND_LAYER_HEIGHTS`]) is bake scratch: pass 2's final core heights as f32, in a store of their own that is deleted once the
//! post-pass has run. Both are coded `codec | first | key |` a brotli stream (kind 17: the rows' wrapping differences of the codes, as the other channels).

use std::io::{Read, Write};

use planet_generators::bake::tile::{TileId as PassTile, TileResult};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_LAYER_CHANNELS, PARAM_RESOLUTION};
use crate::layer_channels::channel_bytes;
use crate::layer_store::LayerStoreError;
use crate::layer_tiles::{layer_tile_id, TileLayerSpec};
use crate::pack::{Record, TileStore};

/// The kind byte of a tile's pit depth.
pub const KIND_LAYER_FILL: u8 = Kind::LayerFill as u8;
/// The kind byte of a tile's final heights (bake scratch).
pub const KIND_LAYER_HEIGHTS: u8 = Kind::LayerHeights as u8;
/// Version of the post-pass kernels (the spill flood and graph, the border geometry, the quantisation of the depth): part of the fill and scratch keys, bumped with any change that moves a bit of kind 17. History: 1 = keep-pits-seams PR 2; 2 = the depth is taken against the exact base plus the stored change, clamped at 0 (PR 3); 3 = the outlets are the cells at or below the sea level that the ocean connectivity calls ocean.
pub const FILL_KERNEL_VERSION: u32 = 3;
const FILL_ID: &str = "erosion-layer-fill";
const HEIGHTS_ID: &str = "erosion-layer-heights";
/// Codec 2: the 16-bit codes (codec 1 held 8-bit codes, keep-pits #1192; those records are another key and never read).
const FILL_CODEC: u8 = 2;
const HEIGHTS_CODEC: u8 = 1;
const HEADER_LEN: usize = 1 + 4 + 16;
const BROTLI_BUFFER: usize = 64 * 1024;

/// The step of the codes a metre, as recorded in the key and the manifest.
pub fn steps_per_m() -> f64 {
    planet_generators::bake::face::FILL_STEPS_PER_M
}

/// The key of a tile's pit depth under `spec`.
pub fn fill_key(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<CacheKey, LayerStoreError> {
    key_of(world, res, spec, id, Kind::LayerFill, FILL_ID, FILL_KERNEL_VERSION)
}

/// The key of a tile's pit depth as the post-pass kernels of `version` would write it (a test of what a bump of [`FILL_KERNEL_VERSION`] re-keys).
pub fn fill_key_at_version(
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    id: PassTile,
    version: u32,
) -> Result<CacheKey, LayerStoreError> {
    key_of(world, res, spec, id, Kind::LayerFill, FILL_ID, version)
}

/// The key of a tile's scratch heights under `spec`.
pub fn heights_key(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<CacheKey, LayerStoreError> {
    key_of(world, res, spec, id, Kind::LayerHeights, HEIGHTS_ID, FILL_KERNEL_VERSION)
}

fn key_of(
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    id: PassTile,
    kind: Kind,
    name: &str,
    version: u32,
) -> Result<CacheKey, LayerStoreError> {
    let tile = layer_tile_id(&spec.grid, id)?;
    let mut inputs = world.base_key_inputs(kind, tile, res, name);
    inputs.output_params.insert(PARAM_RESOLUTION, (spec.grid.cells as u32).to_le_bytes().to_vec());
    let mut bytes = channel_bytes(&spec.bytes());
    bytes.extend(version.to_le_bytes());
    inputs.output_params.insert(PARAM_LAYER_CHANNELS, bytes);
    Ok(cache_key(&inputs))
}

fn compress(raw: &[u8], payload: &mut Vec<u8>) {
    let mut w = brotli::CompressorWriter::new(payload, BROTLI_BUFFER, 5, 22);
    w.write_all(raw).expect("writing to a vector");
}

fn decompress(stream: &[u8], len: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(len);
    brotli::Decompressor::new(stream, BROTLI_BUFFER)
        .take(len as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("the stream does not decode: {e}"))?;
    if out.len() != len {
        return Err(format!(
            "the stream decodes to {}{} bytes, not {len}",
            if out.len() > len { "more than " } else { "" },
            out.len().min(len)
        ));
    }
    Ok(out)
}

fn record(world: &GlobalWorld, kind: u8, codec: u8, tile: planet_core::TileId, key: CacheKey, first: u32, raw: &[u8]) -> Record {
    let mut payload = Vec::with_capacity(HEADER_LEN + raw.len() / 4);
    payload.push(codec);
    payload.extend(first.to_le_bytes());
    payload.extend(key.0);
    compress(raw, &mut payload);
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

fn body<'r>(
    r: &'r Record,
    key: CacheKey,
    codec: u8,
    first: usize,
    what: &dyn Fn(String) -> LayerStoreError,
) -> Result<&'r [u8], LayerStoreError> {
    let p = &r.payload;
    if p.len() < HEADER_LEN || p[0] != codec || u32::from_le_bytes(p[1..5].try_into().expect("four bytes")) as usize != first {
        return Err(what(format!("a payload of {} bytes that is not {first} cells in codec {codec}", p.len())));
    }
    if p[5..21] != key.0 {
        return Err(what("its payload names another key than its record".into()));
    }
    Ok(&p[HEADER_LEN..])
}

fn present(store: &TileStore, kind: u8, tile: planet_core::TileId, key: CacheKey, what: &str) -> Result<bool, LayerStoreError> {
    let keys = store.keys_for(kind, tile);
    match keys.first() {
        None => Ok(false),
        Some(_) if keys.contains(&key) => Ok(true),
        Some(other) => Err(LayerStoreError(format!(
            "{what} (record kind {kind}) key {key}: the store holds it under key {other}, made for another layer: refused, not mixed"
        ))),
    }
}

/// The record of a tile's pit depth codes (`tile x tile`, 16 bits each).
pub fn fill_record(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile, codes: &[u16]) -> Result<Record, LayerStoreError> {
    let t = spec.grid.tile;
    if codes.len() != t * t {
        return Err(LayerStoreError(format!("tile {}/{}/{} has {} fill codes, not {t} x {t}", id.face.0, id.ti, id.tj, codes.len())));
    }
    let mut raw = Vec::with_capacity(codes.len() * 2);
    for row in codes.chunks(t) {
        let mut prev = 0u16;
        for &c in row {
            raw.extend(c.wrapping_sub(prev).to_le_bytes());
            prev = c;
        }
    }
    Ok(record(world, KIND_LAYER_FILL, FILL_CODEC, layer_tile_id(&spec.grid, id)?, fill_key(world, res, spec, id)?, t as u32, &raw))
}

/// Whether the store holds the tile's pit depth under the current key; an error naming both keys when it holds the tile under another.
pub fn has_fill(store: &TileStore, world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<bool, LayerStoreError> {
    let key = fill_key(world, res, spec, id)?;
    present(store, KIND_LAYER_FILL, layer_tile_id(&spec.grid, id)?, key, &format!("layer fill {}/{}/{}", id.face.0, id.ti, id.tj))
}

/// The pit depth codes of tile `id` (`tile x tile`): `Ok(None)` when the store holds none.
pub fn load_fill(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    id: PassTile,
) -> Result<Option<Vec<u16>>, LayerStoreError> {
    let key = fill_key(world, res, spec, id)?;
    let what = |why: String| {
        LayerStoreError(format!("layer fill {}/{}/{} (record kind {KIND_LAYER_FILL}) key {key}: {why}", id.face.0, id.ti, id.tj))
    };
    if !has_fill(store, world, res, spec, id)? {
        return Ok(None);
    }
    let tile = layer_tile_id(&spec.grid, id)?;
    let Some(r) = store.get(KIND_LAYER_FILL, tile, key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let t = spec.grid.tile;
    let raw = decompress(body(&r, key, FILL_CODEC, t, &what)?, t * t * 2).map_err(what)?;
    let mut out = Vec::with_capacity(t * t);
    for row in raw.as_chunks::<2>().0.chunks(t) {
        let mut acc = 0u16;
        for d in row {
            acc = acc.wrapping_add(u16::from_le_bytes(*d));
            out.push(acc);
        }
    }
    Ok(Some(out))
}

/// The scratch record of a tile's final heights (f32 little-endian, `tile x tile`); refused when the result holds none.
pub fn heights_record(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, r: &TileResult) -> Result<Record, LayerStoreError> {
    let t = spec.grid.tile;
    if r.heights.len() != t * t {
        return Err(LayerStoreError(format!("tile {}/{}/{} holds no final heights of {t} x {t} cells", r.id.face.0, r.id.ti, r.id.tj)));
    }
    let raw: Vec<u8> = r.heights.iter().flat_map(|h| h.to_le_bytes()).collect();
    Ok(record(
        world,
        KIND_LAYER_HEIGHTS,
        HEIGHTS_CODEC,
        layer_tile_id(&spec.grid, r.id)?,
        heights_key(world, res, spec, r.id)?,
        t as u32,
        &raw,
    ))
}

/// A scratch record built from heights given as they are (a test or a tool without a pass 2).
pub fn heights_record_of(
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    id: PassTile,
    heights: &[f32],
) -> Result<Record, LayerStoreError> {
    let t = spec.grid.tile;
    assert_eq!(heights.len(), t * t, "a tile holds tile x tile heights");
    let raw: Vec<u8> = heights.iter().flat_map(|h| h.to_le_bytes()).collect();
    Ok(record(world, KIND_LAYER_HEIGHTS, HEIGHTS_CODEC, layer_tile_id(&spec.grid, id)?, heights_key(world, res, spec, id)?, t as u32, &raw))
}

/// Whether the scratch store holds the tile's heights under the current key.
pub fn has_heights(store: &TileStore, world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<bool, LayerStoreError> {
    let key = heights_key(world, res, spec, id)?;
    present(store, KIND_LAYER_HEIGHTS, layer_tile_id(&spec.grid, id)?, key, &format!("layer heights {}/{}/{}", id.face.0, id.ti, id.tj))
}

/// The scratch heights of tile `id`: `Ok(None)` when the store holds none.
pub fn load_heights(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    id: PassTile,
) -> Result<Option<Vec<f32>>, LayerStoreError> {
    let key = heights_key(world, res, spec, id)?;
    let what = |why: String| {
        LayerStoreError(format!("layer heights {}/{}/{} (record kind {KIND_LAYER_HEIGHTS}) key {key}: {why}", id.face.0, id.ti, id.tj))
    };
    if !has_heights(store, world, res, spec, id)? {
        return Ok(None);
    }
    let tile = layer_tile_id(&spec.grid, id)?;
    let Some(r) = store.get(KIND_LAYER_HEIGHTS, tile, key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let t = spec.grid.tile;
    let raw = decompress(body(&r, key, HEIGHTS_CODEC, t, &what)?, t * t * 4).map_err(what)?;
    Ok(Some(raw.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()))
}
