//! The erosion layer's pass 2 in a store (STRM-065, GEN-101; m9-bake PR 5): one record per tile holding the core's change against the base field, quantised, compressed, with the tile's
//! bake statistics, checkpointed per tile so that a killed bake resumes from its last complete tile, and refused when it was made for another bake, other parameters or another pass 1.
//!
//! Record kind 13 ([`KIND_LAYER_TILE`]) filed under the tile's id at the level whose grid is the tile grid (`2^level` tiles a face edge). The key is the bake's stage key (the source), the
//! bake's resolution, and under tag 16 the layer version, the kernel version, the tile grid, the radius, the pass-2 parameters, `inputs_id`, whether the carve factor is applied
//! and a digest of the keys of the six pass-1 face records the tile was computed from (a changed pass 1 is another layer, ADR 0006). The payload is
//! `codec (u8) | tile (u32 LE, core cells a side) | stats (land u64, thermal passes u32, thermal still moving u64, eroded, deposited, exported f64 LE, clamped cells u64) | key (16 bytes) |`
//! an lz4 block of the core's `i16` counts of 0.25 m (little-endian, row `j` outer, then `i`).

use std::path::Path;

use planet_core::{Face, TileId, Vec3};
use planet_generators::bake::face::FaceChange;
use planet_generators::bake::tile::{dequantise, pass2_tile, quantise, Pass2Params, TileGrid, TileId as PassTile, TileResult, TileStats};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_RESOLUTION, PARAM_TILE_LAYER};
use crate::layer_store::{face_key, LayerSpec, LayerStoreError};
use crate::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};

/// The kind byte of a tile's pass-2 change.
pub const KIND_LAYER_TILE: u8 = Kind::LayerTile as u8;
/// Version of the tile record's layout and of the meaning of its key. History: 1 = this layout.
pub const TILE_LAYER_VERSION: u32 = 1;
const IMPLEMENTATION_ID: &str = "erosion-layer-tile";
const CODEC_VERSION: u8 = 1;
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
    fn bytes(&self) -> Vec<u8> {
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
    let mut raw = Vec::with_capacity(counts.len() * 2);
    for c in &counts {
        raw.extend(c.to_le_bytes());
    }
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
    payload.extend(lz4_flex::compress(&raw));
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
    let raw = lz4_flex::decompress(&p[HEADER_LEN..], t * t * 2).map_err(|e| what(format!("its counts do not decompress: {e}")))?;
    if raw.len() != t * t * 2 {
        return Err(what(format!("{} bytes of counts, expected {}", raw.len(), t * t * 2)));
    }
    let counts: Vec<i16> = raw.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c)).collect();
    let stats = TileStats {
        land_cells: u64_at(5) as usize,
        thermal_passes: u32::from_le_bytes(word(13, 4).try_into().expect("four bytes")) as usize,
        thermal_moving_at_cap: u64_at(17) as usize,
        eroded_m3: f64_at(25),
        deposited_m3: f64_at(33),
        exported_m3: f64_at(41),
        clamped_cells: u64_at(49) as usize,
    };
    Ok(Some(TileResult { id, core: dequantise(&counts), stats }))
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
        if load_tile(&store, world, res, spec, id)?.is_some() {
            report.skipped.push(id);
            continue;
        }
        let r = pass2_tile(&spec.grid, id, inputs.base, inputs.face_change, inputs.factor, &spec.params, threads);
        writer.append(&tile_record(world, res, spec, &r)?).map_err(err)?;
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
