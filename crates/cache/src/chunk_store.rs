//! The disk cache of chunk results (STRM-041, ADR 0077 D7): a landed chunk is stored in the pack store under its stage key and verified on read
//! (CON-05); a damaged or missing entry is a miss and the chunk is recomputed.

use crate::key::{body_params_hash128, cache_key, CacheKey, KeyInputs, Kind, PARAM_CHUNK};
use crate::pack::{PackError, PackWriter, Record, StoreParams, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::hash::HASH_VERSION;
use planet_core::TileId;
use planet_world_def::BodyId;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The kind byte of a chunk record.
pub const KIND_CHUNK: u8 = Kind::Chunk as u8;
/// Version of the stored form of a chunk result (`ChunkResult::to_bytes`) and of the key's layout.
pub const CHUNK_STORE_VERSION: u32 = 1;
/// The implementation id of the chunk results' keys.
pub const CHUNK_IMPLEMENTATION_ID: &str = "chunk-result-v1";

/// Everything a chunk result depends on besides its tile (STRM-011): the body, the process (its hash covers its seed, parameters and version),
/// the grid (cells per chunk edge), the pyramid's top level and the key of the base the process reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkSpec {
    pub body: BodyId,
    pub seed: u64,
    pub radius_m: f64,
    /// `ChunkStage::definition_hash`.
    pub definition: u64,
    pub cells_per_chunk: u32,
    pub top_level: u8,
    /// The key of the field below the chunk stage over this chunk (what the process reads), or zeros for a process that reads none.
    pub base: [u8; 16],
}

impl ChunkSpec {
    fn bytes(&self, version: u64) -> Vec<u8> {
        let mut v = Vec::with_capacity(29);
        v.extend_from_slice(&self.definition.to_le_bytes());
        v.extend_from_slice(&self.cells_per_chunk.to_le_bytes());
        v.push(self.top_level);
        v.extend_from_slice(&self.radius_m.to_bits().to_le_bytes());
        // The version of the chunk (GEN-082): nothing for the first (an unedited chunk keeps the key it always had), eight bytes otherwise.
        if version != 0 {
            v.extend_from_slice(&version.to_le_bytes());
        }
        v
    }

    /// The inputs of the content key of the result of `tile` (a tile of the chunk grid).
    pub fn key_inputs(&self, tile: TileId) -> KeyInputs {
        self.key_inputs_versioned(tile, 0)
    }

    /// [`ChunkSpec::key_inputs`] of the chunk at `version` (0: the chunk no edit has changed).
    pub fn key_inputs_versioned(&self, tile: TileId, version: u64) -> KeyInputs {
        let mut output_params = BTreeMap::new();
        output_params.insert(PARAM_CHUNK, self.bytes(version));
        KeyInputs {
            kind: Kind::Chunk,
            hash_version: HASH_VERSION,
            generator_version: CHUNK_STORE_VERSION,
            implementation_id: CHUNK_IMPLEMENTATION_ID.to_string(),
            face_mapping_id: TangentWarp.id().to_string(),
            body: self.body,
            body_params: body_params_hash128(self.body, self.seed, self.radius_m, planet_world_def::SPHERE_SURFACE_ID),
            tile,
            source: Some(CacheKey(self.base)),
            output_params,
            reads: Vec::new(),
        }
    }

    /// The content key of the result of `tile`.
    pub fn key(&self, tile: TileId) -> CacheKey {
        self.key_versioned(tile, 0)
    }

    /// The content key of the result of `tile` at `version`.
    pub fn key_versioned(&self, tile: TileId, version: u64) -> CacheKey {
        cache_key(&self.key_inputs_versioned(tile, version))
    }

    /// What identifies the store of this body.
    pub fn store_params(&self) -> StoreParams {
        StoreParams { hash_version: HASH_VERSION, body: self.body.0 }
    }
}

/// The pack store of chunk results in a directory.
pub struct PackChunkDisk {
    dir: PathBuf,
    spec: ChunkSpec,
}

impl PackChunkDisk {
    pub fn new(dir: PathBuf, spec: ChunkSpec) -> PackChunkDisk {
        PackChunkDisk { dir, spec }
    }

    pub fn spec(&self) -> &ChunkSpec {
        &self.spec
    }

    /// The stored payload of `tile`: `Ok(None)` when none is stored under the current key, an error naming the damage when the entry fails
    /// its check (the caller treats both as a miss and recomputes, and logs the error).
    pub fn get(&self, tile: TileId) -> Result<Option<Vec<u8>>, String> {
        self.get_versioned(tile, 0)
    }

    /// [`PackChunkDisk::get`] of the chunk at `version`.
    pub fn get_versioned(&self, tile: TileId, version: u64) -> Result<Option<Vec<u8>>, String> {
        let store = TileStore::open(&self.dir, self.spec.store_params()).map_err(|e| e.to_string())?;
        match store.get(KIND_CHUNK, tile, self.spec.key_versioned(tile, version)) {
            Ok(Some(r)) => Ok(Some(r.payload)),
            // A miss on a store with damage names the damage: the entry may be the one it hit.
            Ok(None) if !store.damaged().is_empty() || !store.problems().is_empty() => Err(format!(
                "chunk {:#018x} not found in a damaged store: {}; {} damaged records",
                tile.raw(),
                store.problems().join("; "),
                store.damaged().len()
            )),
            Ok(None) => Ok(None),
            Err(e @ PackError::Corrupt { .. }) => Err(format!("chunk {:#018x}: {e}", tile.raw())),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Store `payload` as the result of `tile`.
    pub fn put(&self, tile: TileId, payload: Vec<u8>) -> Result<(), String> {
        self.put_versioned(tile, 0, payload)
    }

    /// [`PackChunkDisk::put`] of the chunk at `version`.
    pub fn put_versioned(&self, tile: TileId, version: u64, payload: Vec<u8>) -> Result<(), String> {
        let mut writer = PackWriter::open(&self.dir, self.spec.store_params(), DEFAULT_MAX_PACK_BYTES).map_err(|e| e.to_string())?;
        let record = Record {
            key: self.spec.key_versioned(tile, version),
            kind: KIND_CHUNK,
            tile,
            generator_version: CHUNK_STORE_VERSION,
            hash_version: HASH_VERSION,
            node_set_id: [0; 16],
            payload,
        };
        writer.append(&record).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())
    }
}
