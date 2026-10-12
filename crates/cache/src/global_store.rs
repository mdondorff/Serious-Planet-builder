//! The global-coarse bake in a tile store (STRM-012, ADR 0026 points 2 and 6, ADR 0019).
//!
//! A bake is stored as blocks: block `b` is the level-`L` tile `b` of a grid of `res = B << L` cells per face edge, holding
//! its `(B + 1)^2` samples of four `f32` channels as a pack record of kind 3. The bake has one stage key (STRM-011, the
//! `Scope::Global` output of the `global-coarse` field); each block's content key is the ADR 0018 key with kind 3, the block's
//! tile, the stage key as its source, `B` and `res` as output parameters and an empty read set. A reader lists the bake's one
//! stage key as its upstream key, never the blocks.
//!
//! Payload (kind 3): `codec version (u8) | cells per block edge B (u32 LE) | raw length (u32 LE) | LZ4 block`. The raw bytes
//! are the four channels in the order continentalness, elevation, temperature, precipitation, each a plane of `(B + 1)^2`
//! little-endian `f32` in row-major order (`k` fastest), each plane byte-shuffled as in the height codec, one plane after
//! the other. The decoder never allocates more than `B` allows.

use std::collections::BTreeMap;
use std::path::Path;

use planet_core::cube::TangentWarp;
use planet_core::hash::HASH_VERSION;
use planet_core::{FaceMapping, TileId};
use planet_generators::global::{
    bake_block, bake_blocks_where, BakeError, GlobalCoarseGenerator, GlobalSample, GLOBAL_VERSION, IMPLEMENTATION_ID, MAX_BLOCK_CELLS,
};
use planet_generators::stage::{FieldGraph, FieldSpec, Scope, Stage};
use planet_world_def::{body_radius_path, BodyId, GenContext, MissingNode, NodeValue, ReadRecord, WorldDefinition, SPHERE_SURFACE_ID};

use crate::codec::{shuffle, unshuffle};
use crate::key::{body_params_hash128, cache_key, read_entry, CacheKey, KeyInputs, Kind, ReadEntry, PARAM_GRID_RES, PARAM_RESOLUTION};
use crate::pack::{PackError, PackWriter, Record, StoreParams, TileStore};
use crate::stage_key::{stage_key, StageKeyContext};
use crate::tiles::{Mismatch, VerifyReport};

/// The kind byte of a block record of the global bake.
pub const KIND_GLOBAL_BLOCK: u8 = Kind::GlobalBlock as u8;

/// Version of the block payload layout. History: 1 = four shuffled `f32` planes, LZ4 block.
pub const BLOCK_CODEC_VERSION: u8 = 1;

const HEADER_LEN: usize = 9;

/// Why a block payload could not be encoded or decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockCodecError {
    Short,
    Version(u8),
    Cells(u32),
    RawLength {
        claimed: u32,
        expected: u64,
    },
    Truncated {
        got: usize,
        expected: u64,
    },
    Lz4(String),
    /// Encoding: the number of samples does not match the block's cells.
    Samples {
        got: usize,
        expected: u64,
    },
}

impl std::fmt::Display for BlockCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockCodecError::Short => write!(f, "the block payload is shorter than its header"),
            BlockCodecError::Version(v) => write!(f, "block codec version {v}, this build reads {BLOCK_CODEC_VERSION}"),
            BlockCodecError::Cells(c) => write!(f, "{c} cells per block edge is not a power of two up to {MAX_BLOCK_CELLS}"),
            BlockCodecError::RawLength { claimed, expected } => {
                write!(f, "the block payload claims {claimed} raw bytes, {expected} are needed for its cells")
            }
            BlockCodecError::Truncated { got, expected } => write!(f, "the LZ4 block gives {got} raw bytes, {expected} are needed"),
            BlockCodecError::Lz4(e) => write!(f, "the LZ4 block is damaged: {e}"),
            BlockCodecError::Samples { got, expected } => write!(f, "the block has {got} samples, {expected} are needed for its cells"),
        }
    }
}

impl std::error::Error for BlockCodecError {}

fn valid_cells(cells: u32) -> bool {
    cells.is_power_of_two() && cells <= MAX_BLOCK_CELLS
}

fn raw_len(cells: u32) -> u64 {
    16 * (u64::from(cells) + 1) * (u64::from(cells) + 1)
}

/// Encode a block of `cells` cells per edge. A block the decoder would refuse is an error here.
pub fn encode_block(cells: u32, samples: &[GlobalSample]) -> Result<Vec<u8>, BlockCodecError> {
    if !valid_cells(cells) {
        return Err(BlockCodecError::Cells(cells));
    }
    let n = (u64::from(cells) + 1) * (u64::from(cells) + 1);
    if samples.len() as u64 != n {
        return Err(BlockCodecError::Samples { got: samples.len(), expected: n });
    }
    let plane = |f: fn(&GlobalSample) -> f32| shuffle(&samples.iter().map(f).collect::<Vec<f32>>());
    let mut raw = plane(|s| s.continent);
    raw.extend(plane(|s| s.elevation_m));
    raw.extend(plane(|s| s.temperature_c));
    raw.extend(plane(|s| s.precipitation_mm));
    let mut out = Vec::with_capacity(HEADER_LEN + raw.len() / 2);
    out.push(BLOCK_CODEC_VERSION);
    out.extend_from_slice(&cells.to_le_bytes());
    out.extend_from_slice(&u32::try_from(raw.len()).expect("at most 16 * 1025^2 bytes").to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress(&raw));
    Ok(out)
}

/// The cells per block edge a payload claims, if its header is well formed.
pub fn block_cells(bytes: &[u8]) -> Result<u32, BlockCodecError> {
    if bytes.len() < HEADER_LEN {
        return Err(BlockCodecError::Short);
    }
    if bytes[0] != BLOCK_CODEC_VERSION {
        return Err(BlockCodecError::Version(bytes[0]));
    }
    let cells = u32::from_le_bytes(bytes[1..5].try_into().expect("4 bytes"));
    if !valid_cells(cells) {
        return Err(BlockCodecError::Cells(cells));
    }
    Ok(cells)
}

/// Decode a block payload into its cells per edge and its `(cells + 1)^2` samples.
pub fn decode_block(bytes: &[u8]) -> Result<(u32, Vec<GlobalSample>), BlockCodecError> {
    let cells = block_cells(bytes)?;
    let claimed = u32::from_le_bytes(bytes[5..9].try_into().expect("4 bytes"));
    let expected = raw_len(cells);
    if u64::from(claimed) != expected {
        return Err(BlockCodecError::RawLength { claimed, expected });
    }
    let raw = lz4_flex::decompress(&bytes[HEADER_LEN..], claimed as usize).map_err(|e| BlockCodecError::Lz4(e.to_string()))?;
    if raw.len() as u64 != expected {
        return Err(BlockCodecError::Truncated { got: raw.len(), expected });
    }
    let plane_bytes = raw.len() / 4;
    let planes: Vec<Vec<f32>> = (0..4).map(|p| unshuffle(&raw[p * plane_bytes..(p + 1) * plane_bytes])).collect();
    let samples = (0..planes[0].len())
        .map(|i| GlobalSample {
            continent: planes[0][i],
            elevation_m: planes[1][i],
            temperature_c: planes[2][i],
            precipitation_mm: planes[3][i],
        })
        .collect();
    Ok((cells, samples))
}

/// The global-coarse bake of one body of a definition as the store sees it: its seed (through the recording context), radius,
/// the entries of its read set and what identifies its store and its blocks.
pub struct GlobalWorld {
    seed: u64,
    body: BodyId,
    radius_m: f64,
    reads: Vec<ReadEntry>,
    node_set_id: [u8; 16],
    graph: FieldGraph,
}

impl GlobalWorld {
    /// Build the world of body 0. The seed is read through a fresh [`GenContext`] (CON-06); the radius is part of the body
    /// parameter hash of every key (ADR 0018) but is not an input of the field, so it is read from the definition directly
    /// and does not enter the read set.
    pub fn from_definition(def: &WorldDefinition) -> Result<GlobalWorld, MissingNode> {
        let body = BodyId::PLANET;
        let ctx = GenContext::new(def);
        let field = GlobalCoarseGenerator.field(&ctx, body)?;
        let radius_m = match def.get(&body_radius_path(body)) {
            Some(NodeValue::F64(r)) if r.is_finite() && r > 0.0 => r,
            _ => return Err(MissingNode(body_radius_path(body))),
        };
        let records = field.read_records();
        let reads: Vec<ReadEntry> = records.iter().map(read_entry).collect();
        // ADR 0025 point 3 for the global field: the paths of its reads (the world seed and the body's derived seed), no
        // radius node, deduplicated, sorted, length-prefixed.
        let mut paths: Vec<String> = records.iter().map(ReadRecord::path).collect();
        paths.sort();
        paths.dedup();
        let mut material = Vec::new();
        for p in &paths {
            material.extend_from_slice(&u32::try_from(p.len()).expect("a node path is under 4 GiB").to_le_bytes());
            material.extend_from_slice(p.as_bytes());
        }
        let node_set_id = blake3::derive_key("planet-builder node set v1", &material)[..16].try_into().expect("16 bytes");
        let graph = FieldGraph::new(&[FieldSpec {
            id: "global-coarse",
            stage: Stage::GlobalCoarse,
            reads: &[],
            version: GLOBAL_VERSION,
            implementation_id: IMPLEMENTATION_ID,
            halo_fraction: 0.0,
        }])
        .expect("one global field is a valid graph");
        Ok(GlobalWorld { seed: field.seed(), body, radius_m, reads, node_set_id, graph })
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// What identifies the store of this world: the hash version and the body.
    pub fn params(&self) -> StoreParams {
        StoreParams { hash_version: HASH_VERSION, body: self.body.0 }
    }

    /// The node set id of every block record (STRM-010 construction over the seed reads).
    pub fn node_set_id(&self) -> [u8; 16] {
        self.node_set_id
    }

    fn body_params(&self) -> [u8; 16] {
        body_params_hash128(self.body, self.seed, self.radius_m, SPHERE_SURFACE_ID)
    }

    /// The one stage key of the bake of `res` cells per face edge (`Scope::Global`).
    pub fn stage_key(&self, res: u32) -> CacheKey {
        let ctx = StageKeyContext {
            body: self.body,
            body_params: self.body_params(),
            face_mapping_id: TangentWarp.id(),
            resolution: res,
            reads: &self.reads,
        };
        stage_key(&self.graph, "global-coarse", Scope::Global, &ctx, &BTreeMap::new())
    }

    /// The inputs of the content key of one block of the bake of `res` cells per face edge in blocks of `cells`.
    pub fn block_key_inputs(&self, tile: TileId, cells: u32, res: u32) -> KeyInputs {
        let mut output_params = BTreeMap::new();
        output_params.insert(PARAM_RESOLUTION, cells.to_le_bytes().to_vec());
        output_params.insert(PARAM_GRID_RES, res.to_le_bytes().to_vec());
        KeyInputs {
            kind: Kind::GlobalBlock,
            hash_version: HASH_VERSION,
            generator_version: GLOBAL_VERSION,
            implementation_id: IMPLEMENTATION_ID.to_string(),
            face_mapping_id: TangentWarp.id().to_string(),
            body: self.body,
            body_params: self.body_params(),
            tile,
            source: Some(self.stage_key(res)),
            output_params,
            reads: Vec::new(),
        }
    }

    pub fn block_key(&self, tile: TileId, cells: u32, res: u32) -> CacheKey {
        cache_key(&self.block_key_inputs(tile, cells, res))
    }

    /// The record of a block of samples.
    pub fn block_record(&self, tile: TileId, cells: u32, samples: &[GlobalSample]) -> Result<Record, BlockCodecError> {
        let res = cells << tile.level();
        Ok(Record {
            key: self.block_key(tile, cells, res),
            kind: KIND_GLOBAL_BLOCK,
            tile,
            generator_version: GLOBAL_VERSION,
            hash_version: HASH_VERSION,
            node_set_id: self.node_set_id,
            payload: encode_block(cells, samples)?,
        })
    }
}

/// Why a build failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobalBuildError {
    Bake(BakeError),
    Codec(BlockCodecError),
    Pack(PackError),
}

impl std::fmt::Display for GlobalBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GlobalBuildError::Bake(e) => write!(f, "{e}"),
            GlobalBuildError::Codec(e) => write!(f, "{e}"),
            GlobalBuildError::Pack(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for GlobalBuildError {}

impl From<BakeError> for GlobalBuildError {
    fn from(e: BakeError) -> Self {
        GlobalBuildError::Bake(e)
    }
}
impl From<BlockCodecError> for GlobalBuildError {
    fn from(e: BlockCodecError) -> Self {
        GlobalBuildError::Codec(e)
    }
}
impl From<PackError> for GlobalBuildError {
    fn from(e: PackError) -> Self {
        GlobalBuildError::Pack(e)
    }
}

/// What a build did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalBuildReport {
    pub written: usize,
    pub reused: usize,
    /// The most blocks the bake held in memory at once.
    pub peak_resident_blocks: usize,
}

/// Bake the grid of `cells << level` cells per face edge into the store in `dir` as blocks of `cells` cells. A block the store
/// already holds under the current key, and whose payload decodes, is reused and not computed, so a second run writes nothing.
pub fn build_global(
    dir: &Path,
    world: &GlobalWorld,
    level: u8,
    cells: u32,
    threads: usize,
    max_pack_bytes: u64,
) -> Result<GlobalBuildReport, GlobalBuildError> {
    let res = u32::try_from(u64::from(cells) << level.min(40)).map_err(|_| BakeError::BadLevel(level))?;
    let existing = TileStore::open(dir, world.params())?;
    let have = |tile: TileId| matches!(existing.get(KIND_GLOBAL_BLOCK, tile, world.block_key(tile, cells, res)), Ok(Some(r)) if decode_block(&r.payload).is_ok());
    let mut writer = PackWriter::open(dir, world.params(), max_pack_bytes)?;
    let (mut written, mut failed) = (0usize, None);
    let stats = bake_blocks_where(world.seed(), &TangentWarp, level, cells, threads, &|t| !have(t), &mut |tile, samples| {
        if failed.is_some() {
            return;
        }
        match world.block_record(tile, cells, &samples).map_err(GlobalBuildError::from).and_then(|r| writer.append(&r).map_err(Into::into))
        {
            Ok(()) => written += 1,
            Err(e) => failed = Some(e),
        }
    })?;
    if let Some(e) = failed {
        return Err(e);
    }
    writer.finish()?;
    let total = 6usize << (2 * u32::from(level));
    Ok(GlobalBuildReport { written, reused: total - stats.blocks, peak_resident_blocks: stats.peak_resident_blocks })
}

/// Verify `sample` evenly spaced block records of the store (all of them when `sample` is 0) against regeneration: the
/// same rules as for height tiles (STRM-004): every record is decoded first; a record under the key the world gives must carry
/// the world's metadata and equal its regeneration bit for bit; a record under another key is stale (explained when its
/// versions or node set id differ, without a known reason when they are all equal). Records of another kind are counted.
pub fn verify_global(store: &TileStore, world: &GlobalWorld, sample: usize) -> VerifyReport {
    let mut report = VerifyReport::default();
    report.unreadable.extend(store.damaged().iter().map(ToString::to_string));
    for item in store.sample_records(sample) {
        report.checked += 1;
        let stored = match item {
            Ok(s) => s,
            Err(e) => {
                report.unreadable.push(e.to_string());
                continue;
            }
        };
        let r = &stored.record;
        if r.kind != KIND_GLOBAL_BLOCK {
            report.unsupported += 1;
            continue;
        }
        let where_ = format!("{}: block {:#018x} key {} at offset {}", stored.pack, r.tile.raw(), r.key, stored.offset);
        let (cells, kept) = match decode_block(&r.payload) {
            Ok(b) => b,
            Err(e) => {
                report.unreadable.push(format!("{where_}: {e}"));
                continue;
            }
        };
        let res = cells << r.tile.level();
        let metadata_current =
            r.generator_version == GLOBAL_VERSION && r.hash_version == HASH_VERSION && r.node_set_id == world.node_set_id;
        if world.block_key(r.tile, cells, res) != r.key {
            if metadata_current {
                report.unexplained_stale += 1;
            } else {
                report.stale += 1;
            }
            continue;
        }
        let mismatch = |detail: String| Mismatch { tile: r.tile, key: r.key, pack: stored.pack.clone(), offset: stored.offset, detail };
        if !metadata_current {
            report.mismatches.push(mismatch(format!(
                "the key is the world's but the record disagrees with it: generator version {} (current {GLOBAL_VERSION}), hash version {} (current {HASH_VERSION}), node set id {} (current {}): the record was written wrongly",
                r.generator_version,
                r.hash_version,
                CacheKey(r.node_set_id),
                CacheKey(world.node_set_id)
            )));
            continue;
        }
        let fresh = bake_block(world.seed, &TangentWarp, r.tile, cells);
        let bits =
            |s: &GlobalSample| [s.continent.to_bits(), s.elevation_m.to_bits(), s.temperature_c.to_bits(), s.precipitation_mm.to_bits()];
        match (0..fresh.len()).position(|i| bits(&fresh[i]) != bits(&kept[i])) {
            None => report.matching += 1,
            Some(i) => {
                let count = (0..fresh.len()).filter(|i| bits(&fresh[*i]) != bits(&kept[*i])).count();
                let n = cells + 1;
                report.mismatches.push(mismatch(format!(
                    "sample ({}, {}): stored {:?}, regenerated {:?}; {count} of {} samples differ: an input of this block is missing from its key",
                    i as u32 % n,
                    i as u32 / n,
                    kept[i],
                    fresh[i],
                    fresh.len()
                )));
            }
        }
    }
    report
}
