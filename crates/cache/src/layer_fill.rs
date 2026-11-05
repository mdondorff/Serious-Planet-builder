//! The pit depth of the erosion layer (GEN-120, STRM-076; m9 keep-pits): the layer keeps the pits of the carved surface in its change and records how deep each lies below its flood, so that
//! the runtime adds the depth back (the filled look) until lake water and a pit-tolerance rule exist.
//!
//! Record kind 16 ([`KIND_LAYER_FACE_FILL`]) holds a face's pass-1 depth (one byte a cell of the face's own cells, filed beside kind 14) and kind 17 ([`KIND_LAYER_FILL`]) a tile's depth
//! (one byte a core cell, filed beside kind 15), both coded as the channels are (`codec | first | key |` a brotli stream of the rows' deltas). A code is `round(m * FILL_STEPS_PER_M)`;
//! the step is part of the channels' key (the code is not decided, `layer_channels::CHANNELS_VERSION` 2).

use planet_core::Face;
use planet_generators::bake::face::{FaceFill, FILL_STEPS_PER_M};
use planet_generators::bake::tile::{TileId as PassTile, TileResult};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_LAYER_CHANNELS, PARAM_RESOLUTION};
use crate::layer_channels::{channel_bytes, checked, decompress, record, undelta_rows};
use crate::layer_store::{LayerSpec, LayerStoreError};
use crate::layer_tiles::{layer_tile_id, TileLayerSpec};
use crate::pack::{Record, TileStore};
use crate::relief_store::face_tile;

/// The kind byte of a face's pit depth.
pub const KIND_LAYER_FACE_FILL: u8 = Kind::LayerFaceFill as u8;
/// The kind byte of a tile's pit depth.
pub const KIND_LAYER_FILL: u8 = Kind::LayerFill as u8;
const FACE_FILL_ID: &str = "erosion-layer-face-fill";
const FILL_ID: &str = "erosion-layer-fill";

/// The step of the codes a metre, as recorded in the key and the manifest.
pub fn steps_per_m() -> f64 {
    FILL_STEPS_PER_M
}

/// The key of a face's pit depth under `spec`.
pub fn face_fill_key(world: &GlobalWorld, res: u32, spec: &LayerSpec, face: Face) -> CacheKey {
    let mut inputs = world.base_key_inputs(Kind::LayerFaceFill, face_tile(face), res, FACE_FILL_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, (spec.cells as u32).to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_LAYER_CHANNELS, channel_bytes(&spec.bytes()));
    cache_key(&inputs)
}

/// The key of a tile's pit depth under `spec`.
pub fn fill_key(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<CacheKey, LayerStoreError> {
    let tile = layer_tile_id(&spec.grid, id)?;
    let mut inputs = world.base_key_inputs(Kind::LayerFill, tile, res, FILL_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, (spec.grid.cells as u32).to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_LAYER_CHANNELS, channel_bytes(&spec.bytes()));
    Ok(cache_key(&inputs))
}

/// The record of a face's pit depth codes (`cells x cells`, `j` outer).
pub fn face_fill_record(world: &GlobalWorld, res: u32, spec: &LayerSpec, face: Face, codes: &[u8]) -> Record {
    assert_eq!(codes.len(), spec.cells * spec.cells, "a face's pit depth holds cells x cells codes");
    record(world, KIND_LAYER_FACE_FILL, face_tile(face), face_fill_key(world, res, spec, face), spec.cells as u32, codes, spec.cells)
}

/// The record of a tile's pit depth; refused when the result holds none.
pub fn fill_record(world: &GlobalWorld, res: u32, spec: &TileLayerSpec, r: &TileResult) -> Result<Record, LayerStoreError> {
    let t = spec.grid.tile;
    if r.fill.len() != t * t {
        return Err(LayerStoreError(format!("tile {}/{}/{} holds no pit depth of {t} x {t} cells", r.id.face.0, r.id.ti, r.id.tj)));
    }
    Ok(record(world, KIND_LAYER_FILL, layer_tile_id(&spec.grid, r.id)?, fill_key(world, res, spec, r.id)?, t as u32, &r.fill, t))
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

/// Whether the store holds the face's pit depth under the current key; an error naming both keys when it holds the face under another.
pub fn has_face_fill(store: &TileStore, world: &GlobalWorld, res: u32, spec: &LayerSpec, face: Face) -> Result<bool, LayerStoreError> {
    present(
        store,
        KIND_LAYER_FACE_FILL,
        face_tile(face),
        face_fill_key(world, res, spec, face),
        &format!("layer face fill of face {}", face.0),
    )
}

/// Whether the store holds the tile's pit depth under the current key (see [`has_face_fill`]).
pub fn has_fill(store: &TileStore, world: &GlobalWorld, res: u32, spec: &TileLayerSpec, id: PassTile) -> Result<bool, LayerStoreError> {
    let key = fill_key(world, res, spec, id)?;
    present(store, KIND_LAYER_FILL, layer_tile_id(&spec.grid, id)?, key, &format!("layer fill {}/{}/{}", id.face.0, id.ti, id.tj))
}

/// The pit depth of `face` (`cells x cells` codes): `Ok(None)` when the store holds none.
pub fn load_face_fill(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &LayerSpec,
    face: Face,
) -> Result<Option<FaceFill>, LayerStoreError> {
    let key = face_fill_key(world, res, spec, face);
    let what =
        |why: String| LayerStoreError(format!("layer face fill of face {} (record kind {KIND_LAYER_FACE_FILL}) key {key}: {why}", face.0));
    if !has_face_fill(store, world, res, spec, face)? {
        return Ok(None);
    }
    let Some(r) = store.get(KIND_LAYER_FACE_FILL, face_tile(face), key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let stream = checked(&r, key, spec.cells, &what)?;
    let mut codes = decompress(stream, spec.cells * spec.cells).map_err(what)?;
    undelta_rows(&mut codes, spec.cells);
    Ok(Some(FaceFill { face, cells: spec.cells, codes }))
}

/// The pit depth codes of tile `id` (`tile x tile`): `Ok(None)` when the store holds none.
pub fn load_fill(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &TileLayerSpec,
    id: PassTile,
) -> Result<Option<Vec<u8>>, LayerStoreError> {
    let key = fill_key(world, res, spec, id)?;
    let tile = layer_tile_id(&spec.grid, id)?;
    let what = |why: String| {
        LayerStoreError(format!("layer fill {}/{}/{} (record kind {KIND_LAYER_FILL}) key {key}: {why}", id.face.0, id.ti, id.tj))
    };
    if !has_fill(store, world, res, spec, id)? {
        return Ok(None);
    }
    let Some(r) = store.get(KIND_LAYER_FILL, tile, key).map_err(|e| what(e.to_string()))? else { return Ok(None) };
    let t = spec.grid.tile;
    let stream = checked(&r, key, t, &what)?;
    let mut codes = decompress(stream, t * t).map_err(what)?;
    undelta_rows(&mut codes, t);
    Ok(Some(codes))
}
