//! The class weights and the relief channels as stored channels of the global bake (STRM-067, STRM-068; ADR 0088 point 1, ADR 0089 decisions 1 and 8).
//!
//! Two record kinds, one record per cube face each, filed under the face's level-0 tile:
//! - [`KIND_GLOBAL_CLASS_WEIGHTS`] (11): the class-weights lattice (GEN-093), `u8` per class. Its key does not carry the class-list version, so a store of another
//!   version is found and refused naming both versions (STRM-067); the lattice's content hash is in every record's header.
//! - [`KIND_GLOBAL_RELIEF`] (10): the `k` and `m` planes of one face with the block ranges' source values (GEN-099). Its key carries everything the bytes depend on:
//!   the bake's stage key, the channels' version, the class-list and class-source versions, the hash of the class lattice, the step limit `max_step`, the ring and the
//!   taper. A changed lattice, version or parameter is another key: the channel reads as absent, never as wrong (STRM-068).
//!
//! A store that holds neither kind opens and reads as before; the loaders return `Ok(None)` for what is not there.

use planet_core::{Face, TileId};
use planet_generators::classes::{ClassLattice, CLASS_COUNT};
use planet_generators::relief_channels::{ReliefChannels, BLOCK_CELLS, RELIEF_CHANNELS_VERSION, RING_RADIUS_M, TAPER_FULL_M};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, KeyInputs, Kind, PARAM_CLASS_WEIGHTS, PARAM_RELIEF, PARAM_RESOLUTION};
use crate::pack::{Record, TileStore};

/// The kind byte of a record of the relief channels.
pub const KIND_GLOBAL_RELIEF: u8 = Kind::GlobalRelief as u8;
/// The kind byte of a record of the class weights.
pub const KIND_GLOBAL_CLASS_WEIGHTS: u8 = Kind::GlobalClassWeights as u8;

/// Version of the payload layouts below. History: 1 = the layouts of this file.
pub const RELIEF_CODEC_VERSION: u8 = 1;

const RELIEF_IMPLEMENTATION_ID: &str = "relief-channels";
const CLASS_WEIGHTS_IMPLEMENTATION_ID: &str = "class-weights";

/// Why a stored channel could not be used. Every message names the record and what was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReliefStoreError(pub String);

impl std::fmt::Display for ReliefStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What the relief channels of a bake depend on besides the bake itself (everything is in the key).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReliefSpec {
    pub class_list_version: u32,
    pub class_source_version: u32,
    /// [`lattice_hash`] of the class lattice the channels were computed from.
    pub lattice_hash: [u8; 16],
    /// The step limit of `k` the bake runs with (GEN-097; a bake parameter, so part of the key).
    pub max_step: f64,
}

impl ReliefSpec {
    fn bytes(&self) -> Vec<u8> {
        assert!(self.max_step.is_finite() && self.max_step > 0.0, "the step limit of k is finite and positive, not {}", self.max_step);
        let mut b = Vec::with_capacity(52);
        b.extend(RELIEF_CHANNELS_VERSION.to_le_bytes());
        b.extend(self.class_list_version.to_le_bytes());
        b.extend(self.class_source_version.to_le_bytes());
        b.extend(self.lattice_hash);
        b.extend(self.max_step.to_bits().to_le_bytes());
        b.extend(RING_RADIUS_M.to_bits().to_le_bytes());
        b.extend(TAPER_FULL_M.to_bits().to_le_bytes());
        b
    }
}

/// The content hash of a class lattice (its bytes and cells): what ties the relief channels to the weights they were computed from.
pub fn lattice_hash(lattice: &ClassLattice) -> [u8; 16] {
    let mut m = (lattice.cells() as u64).to_le_bytes().to_vec();
    m.extend(lattice.to_bytes());
    let full = blake3::derive_key("planet-builder class lattice v1", &m);
    full[..16].try_into().expect("16 of 32 bytes")
}

/// The tile a face's record is filed under: the root tile of the face.
pub fn face_tile(face: Face) -> TileId {
    TileId::new(face, 0, 0, 0).expect("the root tile of a face")
}

fn relief_key(world: &GlobalWorld, res: u32, spec: &ReliefSpec, cells: u32, face: Face) -> CacheKey {
    let mut inputs = world.base_key_inputs(Kind::GlobalRelief, face_tile(face), res, RELIEF_IMPLEMENTATION_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, cells.to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_RELIEF, spec.bytes());
    cache_key(&inputs)
}

fn class_weights_key(world: &GlobalWorld, res: u32, cells: u32, face: Face) -> CacheKey {
    let mut inputs: KeyInputs = world.base_key_inputs(Kind::GlobalClassWeights, face_tile(face), res, CLASS_WEIGHTS_IMPLEMENTATION_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, cells.to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_CLASS_WEIGHTS, CLASS_COUNT.to_le_bytes().to_vec());
    cache_key(&inputs)
}

fn record(world: &GlobalWorld, key: CacheKey, kind: u8, face: Face, payload: Vec<u8>) -> Record {
    Record {
        key,
        kind,
        tile: face_tile(face),
        generator_version: planet_generators::global::GLOBAL_VERSION,
        hash_version: planet_core::hash::HASH_VERSION,
        node_set_id: world.node_set_id(),
        payload,
    }
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes());
}

fn get_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().expect("four bytes"))
}

// ---- class weights ----

const CW_HEADER: usize = 1 + 4 + 4 + 4 + 16;

/// The six records of a class lattice: `codec (u8) | class-list version | class-source version | cells (u32 LE) | lattice hash (16) |` the face's weights.
pub fn class_weights_records(
    world: &GlobalWorld,
    res: u32,
    lattice: &ClassLattice,
    class_list_version: u32,
    class_source_version: u32,
) -> Vec<Record> {
    let cells = lattice.cells() as u32;
    let hash = lattice_hash(lattice);
    let bytes = lattice.to_bytes();
    let per_face = bytes.len() / 6;
    (0..6u8)
        .map(|f| {
            let mut p = Vec::with_capacity(CW_HEADER + per_face);
            p.push(RELIEF_CODEC_VERSION);
            put_u32(&mut p, class_list_version);
            put_u32(&mut p, class_source_version);
            put_u32(&mut p, cells);
            p.extend(hash);
            p.extend_from_slice(&bytes[f as usize * per_face..(f as usize + 1) * per_face]);
            record(world, class_weights_key(world, res, cells, Face(f)), KIND_GLOBAL_CLASS_WEIGHTS, Face(f), p)
        })
        .collect()
}

/// The stored class lattice of `cells` cells per face edge and the class-source version it was made with (the caller compares it with the generator's: the key does not carry
/// it, so a repaint under an unchanged class list writes the same key, and the newest copy wins). Load the class weights BEFORE the relief channels: a changed class-list
/// version is refused here, but reads as absent for the relief channels: `Ok(None)` when the store holds none (a store from before the channel); an error naming both versions when the
/// stored class-list version is not `class_list_version` (STRM-067), and when a record is damaged or the faces disagree.
pub fn load_class_weights(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    cells: u32,
    class_list_version: u32,
) -> Result<Option<(ClassLattice, u32)>, ReliefStoreError> {
    let mut faces: Vec<u8> = Vec::new();
    let mut header: Option<(u32, [u8; 16])> = None;
    for f in 0..6u8 {
        let key = class_weights_key(world, res, cells, Face(f));
        let what =
            |why: String| ReliefStoreError(format!("class weights (record kind {KIND_GLOBAL_CLASS_WEIGHTS}) face {f} key {key}: {why}"));
        let Some(rec) = store.get(KIND_GLOBAL_CLASS_WEIGHTS, face_tile(Face(f)), key).map_err(|e| what(e.to_string()))? else {
            if f == 0 {
                return Ok(None);
            }
            return Err(what("not stored, though face 0 is: the channel is incomplete (re-run the bake's channel step)".into()));
        };
        let p = &rec.payload;
        if p.len() < CW_HEADER || p[0] != RELIEF_CODEC_VERSION {
            return Err(what(format!("a payload of {} bytes that is not codec version {RELIEF_CODEC_VERSION}", p.len())));
        }
        let (stored_list, source, stored_cells) = (get_u32(p, 1), get_u32(p, 5), get_u32(p, 9));
        if stored_list != class_list_version {
            return Err(what(format!("the store holds class-list version {stored_list}, the generator has {class_list_version}")));
        }
        if stored_cells != cells {
            return Err(what(format!("it has {stored_cells} cells per face edge, the layout says {cells}")));
        }
        let hash: [u8; 16] = p[13..29].try_into().expect("16 bytes");
        match header {
            None => header = Some((source, hash)),
            Some(h) if h != (source, hash) => return Err(what("its class-source version or lattice hash differs from face 0's".into())),
            Some(_) => {}
        }
        faces.extend_from_slice(&p[CW_HEADER..]);
    }
    let (source, hash) = header.expect("six faces were read");
    let lattice = ClassLattice::from_bytes(cells as usize, &faces).ok_or_else(|| {
        ReliefStoreError(format!(
            "class weights (record kind {KIND_GLOBAL_CLASS_WEIGHTS}) faces 0 to 5: the stored weights do not fit {cells} cells or do not sum to 255"
        ))
    })?;
    if lattice_hash(&lattice) != hash {
        return Err(ReliefStoreError(format!(
            "class weights (record kind {KIND_GLOBAL_CLASS_WEIGHTS}) faces 0 to 5: the stored weights do not match their lattice hash"
        )));
    }
    Ok(Some((lattice, source)))
}

// ---- relief channels ----

const RC_HEADER: usize = 1 + 4;

/// The six records of the relief channels: `codec (u8) | cells (u32 LE) |` the `k` plane then the `m` plane of the face (f32 LE, row `j` outer, then `i`). The block
/// ranges are not stored: they are recomputed from the planes on load, so a stored range cannot disagree with its block (STRM-068).
pub fn relief_records(world: &GlobalWorld, res: u32, spec: &ReliefSpec, channels: &ReliefChannels) -> Vec<Record> {
    let cells = channels.cells() as u32;
    (0..6u8)
        .map(|f| {
            let planes = channels.face_planes(Face(f));
            let mut p = Vec::with_capacity(RC_HEADER + 8 * planes.k.len());
            p.push(RELIEF_CODEC_VERSION);
            put_u32(&mut p, cells);
            for v in planes.k.iter().chain(planes.m) {
                p.extend(v.to_le_bytes());
            }
            record(world, relief_key(world, res, spec, cells, Face(f)), KIND_GLOBAL_RELIEF, Face(f), p)
        })
        .collect()
}

/// The stored relief channels of `cells` cells per face edge under `spec`: `Ok(None)` when the store holds them under no such key (a store from before the channel, or one
/// computed from another class lattice, version or step limit); an error when a record is damaged or the faces are not all there.
pub fn load_relief(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &ReliefSpec,
    cells: u32,
) -> Result<Option<ReliefChannels>, ReliefStoreError> {
    let mut faces: Vec<(Vec<f32>, Vec<f32>)> = Vec::with_capacity(6);
    let plane = ((cells as usize) + 1) * ((cells as usize) + 1);
    for f in 0..6u8 {
        let key = relief_key(world, res, spec, cells, Face(f));
        let what = |why: String| ReliefStoreError(format!("relief channels (record kind {KIND_GLOBAL_RELIEF}) face {f} key {key}: {why}"));
        let Some(rec) = store.get(KIND_GLOBAL_RELIEF, face_tile(Face(f)), key).map_err(|e| what(e.to_string()))? else {
            if f == 0 {
                return Ok(None);
            }
            return Err(what("not stored, though face 0 is: the channel is incomplete (re-run the bake's channel step)".into()));
        };
        let p = &rec.payload;
        if p.len() != RC_HEADER + 8 * plane || p[0] != RELIEF_CODEC_VERSION || get_u32(p, 1) != cells {
            return Err(what(format!(
                "a payload of {} bytes that is not {cells} cells in codec version {RELIEF_CODEC_VERSION} ({} bytes)",
                p.len(),
                RC_HEADER + 8 * plane
            )));
        }
        let floats: Vec<f32> = p[RC_HEADER..].as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
        let (k, m) = floats.split_at(plane);
        faces.push((k.to_vec(), m.to_vec()));
    }
    ReliefChannels::from_planes(cells as usize, faces)
        .map(Some)
        .ok_or_else(|| ReliefStoreError(format!("relief channels (record kind {KIND_GLOBAL_RELIEF}): the stored planes are not finite or do not fit {cells} cells (blocks of {BLOCK_CELLS})")))
}

const _: () = assert!(KIND_GLOBAL_RELIEF != KIND_GLOBAL_CLASS_WEIGHTS);
