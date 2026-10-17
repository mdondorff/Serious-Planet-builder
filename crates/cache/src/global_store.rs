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

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use planet_core::cube::TangentWarp;
use planet_core::hash::HASH_VERSION;
use planet_core::{Face, FaceMapping, TileId};
use planet_generators::global::{
    bake_block, bake_blocks_with, check_tiled, BakeError, GlobalCoarseGenerator, GlobalSample, LatticeSource, ReadError, GLOBAL_VERSION,
    IMPLEMENTATION_ID, MAX_BATCH_BYTES, MAX_BLOCK_CELLS, MAX_TILED_RES,
};
use planet_generators::ocean::{
    analyse_block, ocean_bits, parallel_map, BlockAnalysis, OceanBits, OceanError, OceanLattice, OceanMerger, OceanTable,
    OCEAN_IMPLEMENTATION_ID,
};
use planet_generators::stage::{FieldGraph, FieldSpec, Scope, Stage};
use planet_generators::water_summary::{summarize_block, SummaryCell, SummaryError, WaterSummary, SUMMARY_IMPLEMENTATION_ID};
use planet_world_def::{
    body_radius_path, body_seed_path, BodyId, GenContext, MissingNode, NodeValue, SeaParams, WorldDefinition, SPHERE_SURFACE_ID,
};

use crate::codec::shuffle;
use crate::key::{
    body_params_hash128, cache_key, read_entry, CacheKey, KeyInputs, Kind, ReadEntry, PARAM_GRID_RES, PARAM_OCEAN_AREA_FRACTION,
    PARAM_RESOLUTION,
};
use crate::pack::{PackError, PackWriter, Record, StoreParams, TileStore};
use crate::stage_key::{stage_key, StageKeyContext};
use crate::tiles::{Mismatch, VerifyReport};

/// The kind byte of a block record of the global bake.
pub const KIND_GLOBAL_BLOCK: u8 = Kind::GlobalBlock as u8;

/// Version of the block payload layout. History: 1 = four shuffled `f32` planes, LZ4 block.
pub const BLOCK_CODEC_VERSION: u8 = 1;

/// The kind byte of a block of the ocean-connectivity channel (ADR 0041 point 2).
pub const KIND_GLOBAL_OCEAN: u8 = Kind::GlobalOcean as u8;

/// Version of the ocean block payload layout. History: 1 = `(B + 1)^2` bits, LSB first, as an LZ4 block.
pub const OCEAN_CODEC_VERSION: u8 = 1;

/// The kind byte of the water summary of the global bake (ADR 0058): the coarse level of the water field.
pub const KIND_GLOBAL_WATER_SUMMARY: u8 = Kind::GlobalWaterSummary as u8;

/// Version of the summary payload layout. History: 1 = one flag byte per cell, then the band elevations as a byte-shuffled `f32`
/// plane, LZ4 block.
pub const SUMMARY_CODEC_VERSION: u8 = 1;

const HEADER_LEN: usize = 9;
/// `codec version (u8) | resolution (u32 LE) | cells per block edge (u32 LE) | raw length (u32 LE)`.
const SUMMARY_HEADER_LEN: usize = 13;

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
    /// Keying or encoding: the level and cells make no grid the tiled bake accepts.
    Grid {
        level: u8,
        cells: u32,
    },
    /// Encoding: the number of samples does not match the block's cells.
    Samples {
        got: usize,
        expected: u64,
    },
    /// An ocean block whose bits do not fit its cells (a bit set beyond the last vertex).
    OceanBits,
    /// An ocean block of another codec version.
    OceanVersion(u8),
    /// A water summary of another codec version.
    SummaryVersion(u8),
    /// A water summary that makes no grid, or whose cells are not what this version writes.
    Summary(String),
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
            BlockCodecError::Grid { level, cells } => write!(f, "{cells} cells per block at level {level} is not a grid this build bakes"),
            BlockCodecError::OceanBits => write!(f, "the ocean block has a bit set beyond its last vertex"),
            BlockCodecError::OceanVersion(v) => write!(f, "ocean block codec version {v}, this build reads {OCEAN_CODEC_VERSION}"),
            BlockCodecError::SummaryVersion(v) => write!(f, "water summary codec version {v}, this build reads {SUMMARY_CODEC_VERSION}"),
            BlockCodecError::Summary(why) => write!(f, "the water summary is not valid: {why}"),
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

/// Cells per face edge of the grid a block of `cells` cells at `tile`'s level belongs to (`cells << level`), if the tiled
/// bake accepts that grid (cells a power of two up to the block limit, a level and a resolution within the limits). A record
/// that says otherwise is forged or from another build; its `res` is never computed by a wrapping shift.
pub fn grid_res(tile: TileId, cells: u32) -> Option<u32> {
    check_tiled(tile.level(), cells).ok()
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
    let n = raw.len() / 16;
    // One pass: sample `i` takes byte `b` of channel `c` from `raw[(4 c + b) n + i]`. Every row is cut to `n` bytes first, so the
    // loop has no bounds checks.
    let row = |c: usize, b: usize| &raw[(4 * c + b) * n..(4 * c + b + 1) * n];
    let [c0, c1, c2, c3] = [row(0, 0), row(0, 1), row(0, 2), row(0, 3)];
    let [e0, e1, e2, e3] = [row(1, 0), row(1, 1), row(1, 2), row(1, 3)];
    let [t0, t1, t2, t3] = [row(2, 0), row(2, 1), row(2, 2), row(2, 3)];
    let [p0, p1, p2, p3] = [row(3, 0), row(3, 1), row(3, 2), row(3, 3)];
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        samples.push(GlobalSample {
            continent: f32::from_le_bytes([c0[i], c1[i], c2[i], c3[i]]),
            elevation_m: f32::from_le_bytes([e0[i], e1[i], e2[i], e3[i]]),
            temperature_c: f32::from_le_bytes([t0[i], t1[i], t2[i], t3[i]]),
            precipitation_mm: f32::from_le_bytes([p0[i], p1[i], p2[i], p3[i]]),
        });
    }
    Ok((cells, samples))
}

fn ocean_raw_len(cells: u32) -> u64 {
    ((u64::from(cells) + 1) * (u64::from(cells) + 1)).div_ceil(8)
}

/// Encode an ocean block: `codec version (u8) | cells per block edge B (u32 LE) | raw length (u32 LE) | LZ4 block` of the
/// `(B + 1)^2` bits, row-major (`k` fastest), eight to a byte, the first vertex in the lowest bit.
pub fn encode_ocean_block(bits: &OceanBits) -> Result<Vec<u8>, BlockCodecError> {
    if !valid_cells(bits.cells) {
        return Err(BlockCodecError::Cells(bits.cells));
    }
    let raw = bits.to_bytes();
    if raw.len() as u64 != ocean_raw_len(bits.cells) {
        return Err(BlockCodecError::Samples { got: raw.len(), expected: ocean_raw_len(bits.cells) });
    }
    let mut out = Vec::with_capacity(HEADER_LEN + raw.len() / 4);
    out.push(OCEAN_CODEC_VERSION);
    out.extend_from_slice(&bits.cells.to_le_bytes());
    out.extend_from_slice(&u32::try_from(raw.len()).expect("at most 1025^2 / 8 bytes").to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress(&raw));
    Ok(out)
}

/// Decode an ocean block payload; it refuses what [`encode_ocean_block`] would not write, without allocating more than `B` allows.
pub fn decode_ocean_block(bytes: &[u8]) -> Result<OceanBits, BlockCodecError> {
    if bytes.len() < HEADER_LEN {
        return Err(BlockCodecError::Short);
    }
    if bytes[0] != OCEAN_CODEC_VERSION {
        return Err(BlockCodecError::OceanVersion(bytes[0]));
    }
    let cells = u32::from_le_bytes(bytes[1..5].try_into().expect("4 bytes"));
    if !valid_cells(cells) {
        return Err(BlockCodecError::Cells(cells));
    }
    let claimed = u32::from_le_bytes(bytes[5..9].try_into().expect("4 bytes"));
    let expected = ocean_raw_len(cells);
    if u64::from(claimed) != expected {
        return Err(BlockCodecError::RawLength { claimed, expected });
    }
    let raw = lz4_flex::decompress(&bytes[HEADER_LEN..], claimed as usize).map_err(|e| BlockCodecError::Lz4(e.to_string()))?;
    if raw.len() as u64 != expected {
        return Err(BlockCodecError::Truncated { got: raw.len(), expected });
    }
    OceanBits::from_bytes(cells, &raw).ok_or(BlockCodecError::OceanBits)
}

/// Encode the water summary: `codec version (u8) | res (u32 LE) | B (u32 LE) | raw length (u32 LE) | LZ4 block` of the `6 g^2` flag
/// bytes followed by the `6 g^2` band elevations as a byte-shuffled little-endian `f32` plane (`g = res / min(B, 64)`; faces, rows,
/// columns in that order, `x` fastest).
pub fn encode_summary(summary: &WaterSummary) -> Result<Vec<u8>, BlockCodecError> {
    let cells = summary.cells();
    let n = cells.len();
    let mut raw = Vec::with_capacity(5 * n);
    raw.extend(cells.iter().map(|c| c.flags));
    for b in 0..4 {
        raw.extend(cells.iter().map(|c| c.band_elevation_m.to_le_bytes()[b]));
    }
    let mut out = Vec::with_capacity(SUMMARY_HEADER_LEN + raw.len() / 4);
    out.push(SUMMARY_CODEC_VERSION);
    out.extend_from_slice(&summary.res().to_le_bytes());
    out.extend_from_slice(&summary.block_cells().to_le_bytes());
    out.extend_from_slice(&u32::try_from(raw.len()).map_err(|_| BlockCodecError::Summary("too many cells".into()))?.to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress(&raw));
    Ok(out)
}

/// Decode a water summary payload; it refuses what [`encode_summary`] would not write (another version, a resolution or block size
/// that makes no summary, a wrong raw length, a damaged block, a flag bit this version does not know) without allocating more than the
/// resolution allows.
pub fn decode_summary(bytes: &[u8]) -> Result<WaterSummary, BlockCodecError> {
    if bytes.len() < SUMMARY_HEADER_LEN {
        return Err(BlockCodecError::Short);
    }
    if bytes[0] != SUMMARY_CODEC_VERSION {
        return Err(BlockCodecError::SummaryVersion(bytes[0]));
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    let (res, cells, claimed) = (word(1), word(5), word(9));
    if u64::from(res) > MAX_TILED_RES || !valid_cells(cells) || cells > res {
        return Err(BlockCodecError::Summary(format!("a bake of {res} cells in blocks of {cells} makes no summary")));
    }
    // The size the header implies is checked against the claim before any cell is allocated.
    let n = planet_generators::water_summary::summary_cell_count(res, cells).ok_or_else(|| {
        BlockCodecError::Summary(format!("a bake of {res} cells in blocks of {cells} makes no summary (or too many cells)"))
    })? as usize;
    let expected = 5 * n as u64;
    if u64::from(claimed) != expected {
        return Err(BlockCodecError::RawLength { claimed, expected });
    }
    let raw = lz4_flex::decompress(&bytes[SUMMARY_HEADER_LEN..], claimed as usize).map_err(|e| BlockCodecError::Lz4(e.to_string()))?;
    if raw.len() as u64 != expected {
        return Err(BlockCodecError::Truncated { got: raw.len(), expected });
    }
    let decoded: Vec<SummaryCell> = (0..n)
        .map(|i| SummaryCell {
            flags: raw[i],
            band_elevation_m: f32::from_le_bytes([raw[n + i], raw[2 * n + i], raw[3 * n + i], raw[4 * n + i]]),
        })
        .collect();
    let out = WaterSummary::from_cells(res, cells, decoded)?;
    if !out.flags_known() {
        return Err(BlockCodecError::Summary("a cell has a flag bit this version does not know".into()));
    }
    Ok(out)
}

impl From<SummaryError> for BlockCodecError {
    fn from(e: SummaryError) -> Self {
        BlockCodecError::Summary(e.to_string())
    }
}

/// The global-coarse bake of one body of a definition as the store sees it: its seed (through the recording context), radius,
/// the entries of its read set and what identifies its store and its blocks.
pub struct GlobalWorld {
    seed: u64,
    body: BodyId,
    radius_m: f64,
    reads: Vec<ReadEntry>,
    node_set_id: [u8; 16],
    /// Cells per block edge at most (store policy, ADR 0027 point 1): not a key input, it decides `B`.
    block_limit: u32,
    graph: FieldGraph,
    /// The sea of the body (ADR 0041): its level and liquid for the water field, and the area threshold of the ocean channel,
    /// which is part of every ocean block's key (not of the bake's, which it does not change).
    sea: SeaParams,
}

impl GlobalWorld {
    /// A smaller block limit (a power of two, at most [`MAX_BLOCK_CELLS`]): the store policy that decides `B = min(res, limit)`.
    /// Test support: it exercises levels above 0 with small grids. A store keeps one limit for good (it is recorded nowhere, so a
    /// store built with another limit is not found by a reader with the default, and verification reports its blocks as of another layout).
    #[doc(hidden)]
    pub fn with_block_limit(mut self, limit: u32) -> GlobalWorld {
        assert!(
            limit.is_power_of_two() && limit <= MAX_BLOCK_CELLS,
            "a block limit is a power of two up to {MAX_BLOCK_CELLS}, got {limit}"
        );
        self.block_limit = limit;
        self
    }

    /// The canonical layout of a bake of `res` cells per face edge (ADR 0027 point 1): `B = min(res, limit)` cells per
    /// block and the blocks are the tiles of level `log2(res / B)`. Returns `(level, cells)`. A `res` that is not a power of
    /// two, or a grid the tiled bake refuses, is an error.
    pub fn layout(&self, res: u32) -> Result<(u8, u32), BakeError> {
        // A resolution the tiled bake cannot reach is a resolution error naming the limit, whatever level it would need.
        if res == 0 || !res.is_power_of_two() || u64::from(res) > MAX_TILED_RES {
            return Err(BakeError::BadResolution(res));
        }
        let cells = res.min(self.block_limit);
        let level = (res / cells).trailing_zeros() as u8;
        check_tiled(level, cells)?;
        Ok((level, cells))
    }

    /// Build the world of body 0. The seed is read through a fresh [`GenContext`] (CON-06); the radius is part of the body
    /// parameter hash of every key (ADR 0018) but is not an input of the field, so it is read from the definition directly
    /// and does not enter the read set.
    pub fn from_definition(def: &WorldDefinition) -> Result<GlobalWorld, MissingNode> {
        GlobalWorld::from_definition_of_body(def, BodyId::PLANET)
    }

    /// The same for any body of the definition: its own derived seed, so its own world, keys and store (a store is per body,
    /// `StoreParams::body`).
    pub fn from_definition_of_body(def: &WorldDefinition, body: BodyId) -> Result<GlobalWorld, MissingNode> {
        let ctx = GenContext::new(def);
        let field = GlobalCoarseGenerator.field(&ctx, body)?;
        let radius_m = match def.get(&body_radius_path(body)) {
            Some(NodeValue::F64(r)) if r.is_finite() && r > 0.0 => r,
            _ => return Err(MissingNode(body_radius_path(body))),
        };
        let records = field.read_records();
        let reads: Vec<ReadEntry> = records.iter().map(read_entry).collect();
        // ADR 0026 (consequences): ADR 0025 point 3's construction over the body's seed node path only, `world.seed` for body 0
        // and `derived.body.N.seed` otherwise, no radius node, length-prefixed. (The context also records the other seed
        // path as a read, and both are in the read entries of the keys; the node set id is the liveness identity.)
        let seed_path = if body == BodyId::PLANET { WorldDefinition::SEED_PATH.to_string() } else { body_seed_path(body) };
        let mut material = Vec::new();
        material.extend_from_slice(&u32::try_from(seed_path.len()).expect("a node path is under 4 GiB").to_le_bytes());
        material.extend_from_slice(seed_path.as_bytes());
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
        // The sea is read from the definition directly, like the radius: it is not an input of the bake, so it does not enter the
        // bake's read set; the one number the ocean channel depends on (the area threshold) is in the ocean blocks' keys.
        let sea = def.sea(body)?;
        // The sea's liquid is a row of the world's table: a dangling kind would only fail later, in the water pass.
        if def.liquids()?.get(sea.liquid).is_none() {
            return Err(MissingNode(format!(
                "{} (liquid kind {} is not in the world's liquid table)",
                planet_world_def::body_sea_liquid_path(body),
                sea.liquid.0
            )));
        }
        Ok(GlobalWorld { seed: field.seed(), body, radius_m, reads, node_set_id, graph, block_limit: MAX_BLOCK_CELLS, sea })
    }

    /// The sea of this body as the definition gives it (Earth defaults for what it does not set).
    pub fn sea(&self) -> SeaParams {
        self.sea
    }

    /// Another area threshold for the ocean channel (test support): a fraction of the body's surface from 0 to 1.
    #[doc(hidden)]
    pub fn with_ocean_area_fraction(mut self, fraction: f64) -> GlobalWorld {
        assert!((0.0..=1.0).contains(&fraction), "an area fraction is from 0 to 1, got {fraction}");
        self.sea.min_area_fraction = fraction;
        self
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
    pub fn block_key_inputs(&self, tile: TileId, cells: u32) -> KeyInputs {
        let res = grid_res(tile, cells).expect("a block of a bake the tiled bake accepts: see grid_res");
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

    pub fn block_key(&self, tile: TileId, cells: u32) -> CacheKey {
        cache_key(&self.block_key_inputs(tile, cells))
    }

    /// (The implementation id `cpu-ocean-connectivity-v1` is the channel's own version: a change of the connectivity rule or the area
    /// formula that leaves the samples alone renames it, and the stale ocean blocks are unreachable without touching the bake.)
    /// The inputs of the content key of one block of the ocean channel: kind 4, the block's tile, the bake's stage key as its
    /// source (so a new bake is a new channel), `B`, `res` and the area threshold as output parameters, an empty read set.
    pub fn ocean_key_inputs(&self, tile: TileId, cells: u32) -> KeyInputs {
        let res = grid_res(tile, cells).expect("a block of a bake the tiled bake accepts: see grid_res");
        let mut output_params = BTreeMap::new();
        output_params.insert(PARAM_RESOLUTION, cells.to_le_bytes().to_vec());
        output_params.insert(PARAM_GRID_RES, res.to_le_bytes().to_vec());
        output_params.insert(PARAM_OCEAN_AREA_FRACTION, self.sea.min_area_fraction.to_bits().to_le_bytes().to_vec());
        KeyInputs {
            kind: Kind::GlobalOcean,
            hash_version: HASH_VERSION,
            generator_version: GLOBAL_VERSION,
            implementation_id: OCEAN_IMPLEMENTATION_ID.to_string(),
            face_mapping_id: TangentWarp.id().to_string(),
            body: self.body,
            body_params: self.body_params(),
            tile,
            source: Some(self.stage_key(res)),
            output_params,
            reads: Vec::new(),
        }
    }

    pub fn ocean_key(&self, tile: TileId, cells: u32) -> CacheKey {
        cache_key(&self.ocean_key_inputs(tile, cells))
    }

    /// The tile the summary record is filed under (the record covers the whole bake): face 0 at level 0.
    pub fn summary_tile() -> TileId {
        TileId::new(Face(0), 0, 0, 0).expect("the root tile of a face")
    }

    /// The inputs of the content key of the water summary of the bake of `res` cells per face edge in blocks of `cells`: kind 5, the
    /// root tile of face 0, the bake's stage key as its source (so a new bake is a new summary), `B` under tag 1, `res` under tag 4,
    /// the area threshold of the ocean channel it summarises under tag 7, an empty read set. The sea level of the definition is no input
    /// (it is applied when a tile is asked, ADR 0058).
    pub fn summary_key_inputs(&self, res: u32, cells: u32) -> KeyInputs {
        let tile = GlobalWorld::summary_tile();
        let mut output_params = BTreeMap::new();
        output_params.insert(PARAM_RESOLUTION, cells.to_le_bytes().to_vec());
        output_params.insert(PARAM_GRID_RES, res.to_le_bytes().to_vec());
        output_params.insert(PARAM_OCEAN_AREA_FRACTION, self.sea.min_area_fraction.to_bits().to_le_bytes().to_vec());
        KeyInputs {
            kind: Kind::GlobalWaterSummary,
            hash_version: HASH_VERSION,
            generator_version: GLOBAL_VERSION,
            // A function of the ocean bits too: renaming the ocean channel's implementation renames the summary.
            implementation_id: format!("{SUMMARY_IMPLEMENTATION_ID}+{OCEAN_IMPLEMENTATION_ID}"),
            face_mapping_id: TangentWarp.id().to_string(),
            body: self.body,
            body_params: self.body_params(),
            tile,
            source: Some(self.stage_key(res)),
            output_params,
            reads: Vec::new(),
        }
    }

    pub fn summary_key(&self, res: u32, cells: u32) -> CacheKey {
        cache_key(&self.summary_key_inputs(res, cells))
    }

    /// The record of the water summary.
    pub fn summary_record(&self, summary: &WaterSummary) -> Result<Record, BlockCodecError> {
        let (res, cells) = (summary.res(), summary.block_cells());
        if self.layout(res).ok().map(|(_, c)| c) != Some(cells) {
            return Err(BlockCodecError::Summary(format!("{cells} cells per block is not the canonical layout of a {res}-cell grid")));
        }
        Ok(Record {
            key: self.summary_key(res, cells),
            kind: KIND_GLOBAL_WATER_SUMMARY,
            tile: GlobalWorld::summary_tile(),
            generator_version: GLOBAL_VERSION,
            hash_version: HASH_VERSION,
            node_set_id: self.node_set_id,
            payload: encode_summary(summary)?,
        })
    }

    /// The record of a block of ocean bits.
    pub fn ocean_record(&self, tile: TileId, bits: &OceanBits) -> Result<Record, BlockCodecError> {
        let cells = bits.cells;
        let res = grid_res(tile, cells).ok_or(BlockCodecError::Grid { level: tile.level(), cells })?;
        if self.layout(res).ok() != Some((tile.level(), cells)) {
            return Err(BlockCodecError::Grid { level: tile.level(), cells });
        }
        Ok(Record {
            key: self.ocean_key(tile, cells),
            kind: KIND_GLOBAL_OCEAN,
            tile,
            generator_version: GLOBAL_VERSION,
            hash_version: HASH_VERSION,
            node_set_id: self.node_set_id,
            payload: encode_ocean_block(bits)?,
        })
    }

    /// Whether a stored global block (of the bake or of its ocean channel) is still current for this world: its generator version,
    /// hash version and node set id equal the world's (STRM-013). Other kinds are not this world's to judge: `true`.
    pub fn is_live(&self, record: &Record) -> bool {
        (record.kind != KIND_GLOBAL_BLOCK && record.kind != KIND_GLOBAL_OCEAN && record.kind != KIND_GLOBAL_WATER_SUMMARY)
            || (record.generator_version == GLOBAL_VERSION && record.hash_version == HASH_VERSION && record.node_set_id == self.node_set_id)
    }

    /// The record of a block of samples.
    pub fn block_record(&self, tile: TileId, cells: u32, samples: &[GlobalSample]) -> Result<Record, BlockCodecError> {
        let res = grid_res(tile, cells).ok_or(BlockCodecError::Grid { level: tile.level(), cells })?;
        if self.layout(res).ok() != Some((tile.level(), cells)) {
            return Err(BlockCodecError::Grid { level: tile.level(), cells });
        }
        Ok(Record {
            key: self.block_key(tile, cells),
            kind: KIND_GLOBAL_BLOCK,
            tile,
            generator_version: GLOBAL_VERSION,
            hash_version: HASH_VERSION,
            node_set_id: self.node_set_id,
            payload: encode_block(cells, samples)?,
        })
    }
}

/// The stored bake of `res` cells per face edge as a [`LatticeSource`] for the reader (ADR 0027 point 7, STRM-014): it finds the
/// block that holds a lattice vertex through the canonical layout and decodes it once. A block that is missing, damaged or of
/// another shape is an error naming the block and its key, never a default.
///
/// Reads by many threads do not meet on a lock. Each thread keeps the block it read last (one `Arc`, matched by source and
/// block), so a run of reads inside a block touches no shared memory at all, however many threads evict meanwhile (a batch that
/// reads a block on one thread decodes it once whatever the budget: GEN-020, LOD-009).
/// Only a change of block goes to the shared cache: a least-recently-used list under a mutex, bounded by a byte budget, whose
/// entries are shared decode slots, so threads that want the same absent block wait for one decode. A block that fails to read
/// is not kept. Whatever served a read, it returns the stored bits.
///
/// Memory: blocks resident in the shared cache never exceed the budget (at least one block); a thread pins at most one block
/// more (the one it read last) until its next read of another block, or for good if it never reads again, and a thread that
/// read this source and then another source or none keeps its last block until its next read or until it exits.
pub struct StoreSource<'a> {
    store: &'a TileStore,
    world: &'a GlobalWorld,
    res: u32,
    level: u8,
    cells: u32,
    id: u64,
    block_bytes: u64,
    budget_blocks: usize,
    decodes: AtomicU64,
    /// Acquisitions of the shared-cache lock by reads (a test observable of STRM-014: only a change of block takes it).
    lookups: AtomicU64,
    /// Recorded only (STRM-022): shared-cache hits, blocks pushed out for the budget, reads that found a block another thread was
    /// still decoding, and the time spent waiting for the cache lock, fetching the record, and decoding it (all threads together).
    shared_hits: AtomicU64,
    evictions: AtomicU64,
    decode_waits: AtomicU64,
    lock_wait_ns: AtomicU64,
    fetch_ns: AtomicU64,
    decode_ns: AtomicU64,
    /// The most blocks the shared cache has held at once, counted under its lock after every insert and eviction (STRM-014: the
    /// budget is never exceeded, at any moment, not only when a test samples it).
    peak_blocks: AtomicUsize,
    shared: Mutex<Vec<(TileId, Arc<Slot>)>>,
}

/// One block of the shared cache: filled once by the first thread that wants it.
struct Slot {
    block: OnceLock<Result<Arc<Vec<GlobalSample>>, ReadError>>,
}

/// What the shared block cache holds, for budgets and tests (CON-20).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceStats {
    /// Blocks decoded so far (a block evicted and wanted again counts again).
    pub decodes: u64,
    /// Times a read took the shared-cache lock: one per change of block that the thread's pinned block did not serve.
    pub shared_lookups: u64,
    /// Blocks in the shared cache, including blocks being decoded.
    pub resident_blocks: usize,
    pub resident_bytes: u64,
    /// The most blocks the shared cache has held at once since the source was made (never above `budget_blocks`).
    pub peak_resident_blocks: usize,
    pub budget_bytes: u64,
    /// The budget in whole blocks (at least one).
    pub budget_blocks: usize,
    /// Shared-cache lookups that found the block's slot (`shared_lookups - shared_hits` made one: a miss that decodes).
    pub shared_hits: u64,
    /// Blocks pushed out of the shared cache for the budget (each decodes again if it is wanted again).
    pub evictions: u64,
    /// Lookups that found a block still being decoded by another thread and waited for it.
    pub decode_waits: u64,
    /// Nanoseconds all threads together spent acquiring the cache lock, fetching block records, and decoding them.
    pub lock_wait_ns: u64,
    pub fetch_ns: u64,
    pub decode_ns: u64,
}

/// Default budget of decoded blocks a [`StoreSource`] keeps: 512 MiB, ADR 0033 point 6's proposal for bake blocks (31 blocks of
/// 1024 cells; it was 16 blocks).
pub const SOURCE_CACHE_BYTES: u64 = 512 << 20;

/// The block a thread read last.
struct Pinned {
    source: u64,
    /// Face, block column and block row.
    at: (u8, u32, u32),
    block: Arc<Vec<GlobalSample>>,
}

thread_local! {
    static PINNED: RefCell<Option<Pinned>> = const { RefCell::new(None) };
    /// Nanoseconds this thread spent in the shared cache (lock, waiting for another thread's decode, decoding): recorded only.
    static BLOCK_WAIT_NS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Blocks this thread decoded itself (recorded only).
    static THREAD_DECODES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Blocks the calling thread has decoded itself so far, for any [`StoreSource`] (recorded only, STRM-022): a job reads it before and
/// after its load to learn how many of the decodes were its own.
pub fn thread_block_decodes() -> u64 {
    THREAD_DECODES.with(std::cell::Cell::get)
}

/// Nanoseconds the calling thread has spent so far getting blocks from the shared cache of any [`StoreSource`] (lock, waiting for
/// another thread's decode, decoding; recorded only): a job reads it before and after its load to learn how much of its time went to blocks.
pub fn thread_block_wait_ns() -> u64 {
    BLOCK_WAIT_NS.with(std::cell::Cell::get)
}

static NEXT_SOURCE_ID: AtomicU64 = AtomicU64::new(1);

impl<'a> StoreSource<'a> {
    /// The reader of the bake of `res` cells per face edge in `store` (the canonical layout of `world`), with the default budget.
    pub fn new(store: &'a TileStore, world: &'a GlobalWorld, res: u32) -> Result<StoreSource<'a>, BakeError> {
        StoreSource::with_budget(store, world, res, SOURCE_CACHE_BYTES)
    }

    /// The same with a budget of `budget_bytes` for decoded blocks (rounded down to whole blocks, at least one).
    pub fn with_budget(store: &'a TileStore, world: &'a GlobalWorld, res: u32, budget_bytes: u64) -> Result<StoreSource<'a>, BakeError> {
        let (level, cells) = world.layout(res)?;
        let block_bytes = raw_len(cells);
        let budget_blocks = usize::try_from((budget_bytes / block_bytes).max(1)).unwrap_or(usize::MAX);
        Ok(StoreSource {
            store,
            world,
            res,
            level,
            cells,
            id: NEXT_SOURCE_ID.fetch_add(1, Ordering::Relaxed),
            block_bytes,
            budget_blocks,
            decodes: AtomicU64::new(0),
            lookups: AtomicU64::new(0),
            shared_hits: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            decode_waits: AtomicU64::new(0),
            lock_wait_ns: AtomicU64::new(0),
            fetch_ns: AtomicU64::new(0),
            decode_ns: AtomicU64::new(0),
            peak_blocks: AtomicUsize::new(0),
            shared: Mutex::new(Vec::new()),
        })
    }

    /// What the shared cache holds now.
    pub fn stats(&self) -> SourceStats {
        let resident = self.shared.lock().expect("block cache mutex").len();
        SourceStats {
            decodes: self.decodes.load(Ordering::Relaxed),
            shared_lookups: self.lookups.load(Ordering::Relaxed),
            resident_blocks: resident,
            resident_bytes: resident as u64 * self.block_bytes,
            peak_resident_blocks: self.peak_blocks.load(Ordering::Relaxed),
            budget_bytes: self.budget_blocks as u64 * self.block_bytes,
            budget_blocks: self.budget_blocks,
            shared_hits: self.shared_hits.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            decode_waits: self.decode_waits.load(Ordering::Relaxed),
            lock_wait_ns: self.lock_wait_ns.load(Ordering::Relaxed),
            fetch_ns: self.fetch_ns.load(Ordering::Relaxed),
            decode_ns: self.decode_ns.load(Ordering::Relaxed),
        }
    }

    /// The slot of `tile`, made if absent (least recently used blocks leave beyond the budget).
    fn slot(&self, tile: TileId) -> (Arc<Slot>, bool) {
        self.lookups.fetch_add(1, Ordering::Relaxed);
        let waiting = Instant::now();
        let mut shared = self.shared.lock().expect("block cache mutex");
        self.lock_wait_ns.fetch_add(u64::try_from(waiting.elapsed().as_nanos()).unwrap_or(u64::MAX), Ordering::Relaxed);
        if let Some(at) = shared.iter().position(|(t, _)| *t == tile) {
            self.shared_hits.fetch_add(1, Ordering::Relaxed);
            let entry = shared.remove(at);
            let slot = Arc::clone(&entry.1);
            shared.push(entry);
            return (slot, true);
        }
        let slot = Arc::new(Slot { block: OnceLock::new() });
        shared.push((tile, Arc::clone(&slot)));
        if shared.len() > self.budget_blocks {
            shared.remove(0);
            self.evictions.fetch_add(1, Ordering::Relaxed);
        }
        self.peak_blocks.fetch_max(shared.len(), Ordering::Relaxed);
        (slot, false)
    }

    fn forget(&self, tile: TileId, slot: &Arc<Slot>) {
        let mut shared = self.shared.lock().expect("block cache mutex");
        if let Some(at) = shared.iter().position(|(t, s)| *t == tile && Arc::ptr_eq(s, slot)) {
            shared.remove(at);
        }
    }

    fn load(&self, tile: TileId) -> Result<Arc<Vec<GlobalSample>>, ReadError> {
        let key = self.world.block_key(tile, self.cells);
        let what = |why: String| {
            ReadError::Source(format!(
                "block {:#018x} (face {} level {} x {} y {}) key {key}: {why}",
                tile.raw(),
                tile.face().0,
                tile.level(),
                tile.x(),
                tile.y()
            ))
        };
        let fetching = Instant::now();
        let rec = self
            .store
            .get(KIND_GLOBAL_BLOCK, tile, key)
            .map_err(|e| what(e.to_string()))?
            .ok_or_else(|| what("not stored under the current key".into()))?;
        let decoding = Instant::now();
        self.fetch_ns.fetch_add(u64::try_from((decoding - fetching).as_nanos()).unwrap_or(u64::MAX), Ordering::Relaxed);
        let (cells, samples) = decode_block(&rec.payload).map_err(|e| what(e.to_string()))?;
        self.decode_ns.fetch_add(u64::try_from(decoding.elapsed().as_nanos()).unwrap_or(u64::MAX), Ordering::Relaxed);
        if cells != self.cells {
            return Err(what(format!("its payload has {cells} cells per edge, the layout says {}", self.cells)));
        }
        self.decodes.fetch_add(1, Ordering::Relaxed);
        THREAD_DECODES.with(|c| c.set(c.get() + 1));
        Ok(Arc::new(samples))
    }

    /// The block of `tile` from the shared cache (decoded by this thread or another).
    fn shared_block(&self, tile: TileId) -> Result<Arc<Vec<GlobalSample>>, ReadError> {
        let (slot, hit) = self.slot(tile);
        if hit && slot.block.get().is_none() {
            // Found in the cache but not decoded yet: another thread is decoding it, and this one waits for it below.
            self.decode_waits.fetch_add(1, Ordering::Relaxed);
        }
        match slot.block.get_or_init(|| self.load(tile)) {
            Ok(block) => Ok(Arc::clone(block)),
            Err(e) => {
                // Not kept: the next reader reads again (the store may have gained the block).
                self.forget(tile, &slot);
                Err(e.clone())
            }
        }
    }

    fn sample(&self, at: (u8, u32, u32), index: usize) -> Result<GlobalSample, ReadError> {
        let hit = PINNED.with(|p| p.borrow().as_ref().filter(|p| p.source == self.id && p.at == at).map(|p| p.block[index]));
        if let Some(s) = hit {
            return Ok(s);
        }
        let tile = TileId::new(Face(at.0), self.level, at.1, at.2).expect("a tile of the grid");
        let waiting = Instant::now();
        let block = self.shared_block(tile);
        BLOCK_WAIT_NS.with(|c| c.set(c.get().saturating_add(u64::try_from(waiting.elapsed().as_nanos()).unwrap_or(u64::MAX))));
        let block = block?;
        let s = block[index];
        PINNED.with(|p| *p.borrow_mut() = Some(Pinned { source: self.id, at, block }));
        Ok(s)
    }
}

impl LatticeSource for StoreSource<'_> {
    fn res(&self) -> u32 {
        self.res
    }

    /// Cells per block edge: batch reads group their samples by block (GEN-020).
    fn block_cells(&self) -> u32 {
        self.cells
    }

    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        if i > self.res || j > self.res {
            return Err(ReadError::Source(format!("vertex ({i}, {j}) of a {}-cell grid", self.res)));
        }
        // The block that holds a vertex: the last block of a row holds the far edge as its own last sample.
        let side = 1u32 << self.level;
        let shift = self.cells.trailing_zeros();
        let (bx, by) = ((i >> shift).min(side - 1), (j >> shift).min(side - 1));
        let (k, l) = (i - bx * self.cells, j - by * self.cells);
        self.sample((face.0, bx, by), (l * (self.cells + 1) + k) as usize)
    }
}

/// Why a build failed.
#[derive(Clone, Debug, PartialEq)]
pub enum GlobalBuildError {
    Bake(BakeError),
    Codec(BlockCodecError),
    Pack(PackError),
    Ocean(OceanError),
    /// A block the ocean channel needs is not in the store (under its current key, decodable, of the layout's cells).
    MissingBlock(String),
}

impl std::fmt::Display for GlobalBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GlobalBuildError::Bake(e) => write!(f, "{e}"),
            GlobalBuildError::Codec(e) => write!(f, "{e}"),
            GlobalBuildError::Pack(e) => write!(f, "{e}"),
            GlobalBuildError::Ocean(e) => write!(f, "{e}"),
            GlobalBuildError::MissingBlock(e) => write!(f, "the ocean channel needs a block the store does not hold: {e}"),
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
impl From<SummaryError> for GlobalBuildError {
    fn from(e: SummaryError) -> Self {
        GlobalBuildError::Codec(BlockCodecError::Summary(e.to_string()))
    }
}
impl From<OceanError> for GlobalBuildError {
    fn from(e: OceanError) -> Self {
        GlobalBuildError::Ocean(e)
    }
}

/// What a build did.
#[derive(Clone, Copy, Debug)]
pub struct GlobalBuildReport {
    pub written: usize,
    pub reused: usize,
    /// The most blocks the bake held in memory at once.
    pub peak_resident_blocks: usize,
    /// Ocean blocks written, and found in the store under their current key.
    pub ocean_written: usize,
    pub ocean_reused: usize,
    /// The sea components of the bake (0 when every ocean block was already stored and the table was not recomputed).
    pub ocean_components: usize,
    /// How many of them are ocean.
    pub ocean_oceans: usize,
    /// The bytes the ocean merger held at its largest: the extra memory of the global union-find (0 when it did not run).
    pub ocean_merger_bytes: usize,
    /// Seconds of the build spent on the ocean channel beyond the bake itself (the analysis of reused blocks, the merge and the
    /// bits), and of the analysis that ran inside the bake's worker threads (thread time, summed over blocks).
    pub ocean_seconds: f64,
    pub ocean_in_bake_thread_seconds: f64,
    /// Whether this run wrote the water summary (ADR 0058; `false`: it was already stored under its current key), the seconds that
    /// took and the payload's bytes (0 when not written).
    pub summary_written: bool,
    pub summary_seconds: f64,
    pub summary_bytes: usize,
}

/// Bytes per vertex of a block alive in the ocean passes beyond the bake's own 16: a block read back from the store holds its payload,
/// the decompressed planes and the samples (about 33 B), and the analysis adds the sea mask and the union-find (about 10 B).
const OCEAN_READ_BYTES_PER_VERTEX: u64 = 48;
/// The same while a baked block is analysed on its worker: 16 B of samples and about 10 B of analysis.
const OCEAN_BAKE_BYTES_PER_VERTEX: u64 = 26;

/// How many blocks of `cells` cells an ocean pass keeps alive at once: the thread count, at most [`MAX_BATCH_BYTES`] of blocks at
/// `bytes_per_vertex` each (so the 1 GiB bound of the bake holds with the ocean pass included), and at least one.
fn batch_blocks_of(cells: u32, threads: usize, bytes_per_vertex: u64) -> usize {
    let block_bytes = bytes_per_vertex * (u64::from(cells) + 1) * (u64::from(cells) + 1);
    threads.max(1).min(usize::try_from((MAX_BATCH_BYTES / block_bytes).max(1)).unwrap_or(usize::MAX))
}

/// Finish the merge once every one of the `total` blocks has reported: a missing or repeated report would change an area, so it is an error.
fn finish_merger(merger: OceanMerger, total: usize) -> Result<OceanTable, GlobalBuildError> {
    if merger.blocks() != total {
        return Err(GlobalBuildError::MissingBlock(format!("the ocean merger received {} reports for {total} blocks", merger.blocks())));
    }
    Ok(merger.finish())
}

/// The tile of block `index` of a grid at `level`, in the order of the bake (face, row, column): no list of tiles is built.
fn tile_at(level: u8, index: usize) -> TileId {
    let side = 1usize << level;
    let (face, rest) = (index / (side * side), index % (side * side));
    TileId::new(Face(face as u8), level, (rest % side) as u32, (rest / side) as u32).expect("in range")
}

/// The stored sample block of `tile`, decoded: an error naming the block when it is missing, damaged or of other cells.
fn stored_block(store: &TileStore, world: &GlobalWorld, tile: TileId, cells: u32) -> Result<Vec<GlobalSample>, GlobalBuildError> {
    let key = world.block_key(tile, cells);
    let what = |why: String| GlobalBuildError::MissingBlock(format!("block {:#018x} key {key}: {why}", tile.raw()));
    let rec = store
        .get(KIND_GLOBAL_BLOCK, tile, key)
        .map_err(|e| what(e.to_string()))?
        .ok_or_else(|| what("not stored under the current key".into()))?;
    let (c, samples) = decode_block(&rec.payload).map_err(|e| what(e.to_string()))?;
    if c != cells {
        return Err(what(format!("its payload has {c} cells per edge, the layout says {cells}")));
    }
    Ok(samples)
}

/// The reports of `tiles` (stored blocks) for the ocean merger, a batch at a time on up to `threads` threads: at most
/// [`batch_blocks`] blocks are decoded at once.
fn analyse_stored(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    cells: u32,
    threads: usize,
    tiles: &mut dyn Iterator<Item = TileId>,
    merger: &mut OceanMerger,
) -> Result<(), GlobalBuildError> {
    let batch_n = batch_blocks_of(cells, threads, OCEAN_READ_BYTES_PER_VERTEX);
    loop {
        let batch: Vec<TileId> = (&mut *tiles).take(batch_n).collect();
        if batch.is_empty() {
            return Ok(());
        }
        let results = parallel_map(&batch, threads.min(batch_n), &|t| -> Result<BlockAnalysis, GlobalBuildError> {
            let samples = stored_block(store, world, *t, cells)?;
            Ok(analyse_block(&TangentWarp, res, *t, cells, &samples)?)
        });
        for r in results {
            merger.add(r?);
        }
    }
}

/// The ocean table of a stored bake, computed from its stored blocks (the known answers of a bake, and the reference of
/// [`verify_global`] for ocean blocks): the same table the build computes.
pub fn ocean_table_of_store(store: &TileStore, world: &GlobalWorld, res: u32, threads: usize) -> Result<OceanTable, GlobalBuildError> {
    let (level, cells) = world.layout(res)?;
    let total = 6usize << (2 * u32::from(level));
    let mut merger = OceanMerger::new(res, world.sea.min_area_fraction)?;
    analyse_stored(store, world, res, cells, threads, &mut (0..total).map(|i| tile_at(level, i)), &mut merger)?;
    finish_merger(merger, total)
}

/// Bake the grid of `res` cells per face edge into the store in `dir` in its canonical layout (ADR 0027 point 1: blocks of
/// `min(res, limit)` cells, the tiles of the level that covers the grid), then its ocean-connectivity channel (ADR 0041
/// point 2). A block the store already holds under the current key, and whose payload decodes, is reused and not computed, so a
/// second run writes nothing.
///
/// The ocean channel is computed in three steps (`planet_generators::ocean`): each block that is baked is analysed on its worker
/// thread while it is in memory, the blocks that were already stored are decoded and analysed in batches, the reports are merged,
/// and the ocean blocks the store lacks are written from a second pass over the stored blocks. When every ocean block is already
/// stored under its current key, none of this runs.
pub fn build_global(
    dir: &Path,
    world: &GlobalWorld,
    res: u32,
    threads: usize,
    max_pack_bytes: u64,
) -> Result<GlobalBuildReport, GlobalBuildError> {
    // Refuse a bad shape before anything is opened, created or written.
    let (level, cells) = world.layout(res)?;
    // The writer first (it takes the directory's lock), then a snapshot of what is stored: a run that waited for the lock
    // sees the blocks the run before it wrote and does not compute them again.
    let mut writer = PackWriter::open(dir, world.params(), max_pack_bytes)?;
    let existing = TileStore::open(dir, world.params())?;
    // A block is kept only if it decodes with the cells the key says: a payload of another `B` under this key is a
    // wrongly written record and is written again.
    let have = |tile: TileId| matches!(existing.get(KIND_GLOBAL_BLOCK, tile, world.block_key(tile, cells)), Ok(Some(r)) if matches!(decode_block(&r.payload), Ok((c, _)) if c == cells));
    let have_ocean = |tile: TileId| matches!(existing.get(KIND_GLOBAL_OCEAN, tile, world.ocean_key(tile, cells)), Ok(Some(r)) if matches!(decode_ocean_block(&r.payload), Ok(b) if b.cells == cells));
    let total = 6usize << (2 * u32::from(level));
    let need_ocean = (0..total).any(|i| !have_ocean(tile_at(level, i)));
    let mut merger = if need_ocean { Some(OceanMerger::new(res, world.sea.min_area_fraction)?) } else { None };
    let (mut written, mut failed) = (0usize, None);
    let stop = AtomicBool::new(false);
    let in_bake_ns = AtomicU64::new(0);
    let stats = bake_blocks_with(
        world.seed(),
        &TangentWarp,
        level,
        cells,
        if need_ocean { batch_blocks_of(cells, threads, OCEAN_BAKE_BYTES_PER_VERTEX) } else { threads },
        &|t| !stop.load(Ordering::SeqCst) && !have(t),
        &|t, samples| {
            if !need_ocean {
                return None;
            }
            let started = Instant::now();
            let a = analyse_block(&TangentWarp, res, t, cells, samples);
            in_bake_ns.fetch_add(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX), Ordering::Relaxed);
            Some(a)
        },
        &mut |tile, samples, analysis| {
            if failed.is_some() {
                return;
            }
            let analysed = match (analysis, merger.as_mut()) {
                (Some(Ok(a)), Some(m)) => {
                    m.add(a);
                    Ok(())
                }
                (Some(Err(e)), _) => Err(GlobalBuildError::from(e)),
                _ => Ok(()),
            };
            match analysed.and_then(|()| {
                world
                    .block_record(tile, cells, &samples)
                    .map_err(GlobalBuildError::from)
                    .and_then(|r| writer.append(&r).map_err(Into::into))
            }) {
                Ok(()) => written += 1,
                Err(e) => {
                    // The first failure ends the bake: no further block is accepted for computing.
                    stop.store(true, Ordering::SeqCst);
                    failed = Some(e);
                }
            }
        },
    )?;
    if let Some(e) = failed {
        return Err(e);
    }
    let mut report = GlobalBuildReport {
        written,
        reused: total - stats.blocks,
        peak_resident_blocks: stats.peak_resident_blocks,
        ocean_written: 0,
        ocean_reused: total,
        ocean_components: 0,
        ocean_oceans: 0,
        ocean_merger_bytes: 0,
        ocean_seconds: 0.0,
        ocean_in_bake_thread_seconds: in_bake_ns.load(Ordering::Relaxed) as f64 * 1e-9,
        summary_written: false,
        summary_seconds: 0.0,
        summary_bytes: 0,
    };
    let Some(mut merger) = merger else {
        write_summary_if_missing(dir, world, res, threads, &mut writer, &mut report)?;
        writer.finish()?;
        return Ok(report);
    };
    let started = Instant::now();
    // The blocks that were stored before this run were not in memory: decode and analyse them now.
    analyse_stored(&existing, world, res, cells, threads, &mut (0..total).map(|i| tile_at(level, i)).filter(|t| have(*t)), &mut merger)?;
    report.ocean_merger_bytes = merger.held_bytes();
    let table = finish_merger(merger, total)?;
    report.ocean_components = table.components.len();
    report.ocean_oceans = table.ocean_components().count();
    // The blocks written above must be readable for the second pass: put their index on disk and look at the store again.
    writer.write_index()?;
    let store = TileStore::open(dir, world.params())?;
    let batch_n = batch_blocks_of(cells, threads, OCEAN_READ_BYTES_PER_VERTEX);
    let mut todo = (0..total).map(|i| tile_at(level, i)).filter(|t| !have_ocean(*t));
    report.ocean_reused = total;
    loop {
        let batch: Vec<TileId> = todo.by_ref().take(batch_n).collect();
        if batch.is_empty() {
            break;
        }
        let records = parallel_map(&batch, threads.min(batch_n), &|t| -> Result<Record, GlobalBuildError> {
            let samples = stored_block(&store, world, *t, cells)?;
            let bits = ocean_bits(res, *t, cells, &samples, &table)?;
            Ok(world.ocean_record(*t, &bits)?)
        });
        for r in records {
            writer.append(&r?)?;
            report.ocean_written += 1;
            report.ocean_reused -= 1;
        }
    }
    report.ocean_seconds = started.elapsed().as_secs_f64();
    write_summary_if_missing(dir, world, res, threads, &mut writer, &mut report)?;
    writer.finish()?;
    Ok(report)
}

/// The water summary of a stored bake, computed from its stored sample blocks and ocean blocks (ADR 0058): the known answer of a
/// bake and the reference of [`verify_global`] for the summary record. A pure function of the stored blocks, so the thread count
/// changes nothing.
pub fn water_summary_of_store(store: &TileStore, world: &GlobalWorld, res: u32, threads: usize) -> Result<WaterSummary, GlobalBuildError> {
    let (level, cells) = world.layout(res)?;
    let total = 6usize << (2 * u32::from(level));
    let mut summary = WaterSummary::new(res, cells)?;
    let batch_n = batch_blocks_of(cells, threads, OCEAN_READ_BYTES_PER_VERTEX);
    let mut tiles = (0..total).map(|i| tile_at(level, i));
    loop {
        let batch: Vec<TileId> = tiles.by_ref().take(batch_n).collect();
        if batch.is_empty() {
            return Ok(summary);
        }
        let results = parallel_map(&batch, threads.min(batch_n), &|t| -> Result<Vec<SummaryCell>, GlobalBuildError> {
            let samples = stored_block(store, world, *t, cells)?;
            let key = world.ocean_key(*t, cells);
            let what = |why: String| GlobalBuildError::MissingBlock(format!("ocean block {:#018x} key {key}: {why}", t.raw()));
            let rec = store
                .get(KIND_GLOBAL_OCEAN, *t, key)
                .map_err(|e| what(e.to_string()))?
                .ok_or_else(|| what("not stored under the current key".into()))?;
            let bits = decode_ocean_block(&rec.payload).map_err(|e| what(e.to_string()))?;
            Ok(summarize_block(cells, &samples, &bits)?)
        });
        for (t, r) in batch.iter().zip(results) {
            summary.set_block(*t, &r?)?;
        }
    }
}

/// The stored water summary of the bake of `res` cells in `store`, decoded: an error naming the record and the command that adds it
/// when it is missing, damaged or of another shape (ADR 0058).
pub fn load_water_summary(store: &TileStore, world: &GlobalWorld, res: u32) -> Result<WaterSummary, ReadError> {
    let (_, cells) = world.layout(res).map_err(|e| ReadError::Source(e.to_string()))?;
    let key = world.summary_key(res, cells);
    let what = |why: String| ReadError::Source(format!("water summary (record kind {KIND_GLOBAL_WATER_SUMMARY}) key {key}: {why}"));
    let rec =
        store.get(KIND_GLOBAL_WATER_SUMMARY, GlobalWorld::summary_tile(), key).map_err(|e| what(e.to_string()))?.ok_or_else(|| {
            what("not stored under the current key (add it with `generate --global-dir DIR`, which writes only what is missing)".into())
        })?;
    let summary = decode_summary(&rec.payload).map_err(|e| what(e.to_string()))?;
    if summary.res() != res || summary.block_cells() != cells {
        return Err(what(format!(
            "it is of a {}-cell bake in blocks of {}, the layout says {res} cells in blocks of {cells}",
            summary.res(),
            summary.block_cells()
        )));
    }
    Ok(summary)
}

/// Write the water summary if the store does not hold it under its current key: from the stored blocks, after putting the blocks this
/// run wrote on disk. A bake whose summary is stored recomputes nothing.
fn write_summary_if_missing(
    dir: &Path,
    world: &GlobalWorld,
    res: u32,
    threads: usize,
    writer: &mut PackWriter,
    report: &mut GlobalBuildReport,
) -> Result<(), GlobalBuildError> {
    writer.write_index()?;
    let store = TileStore::open(dir, world.params())?;
    if load_water_summary(&store, world, res).is_ok() {
        return Ok(());
    }
    let started = Instant::now();
    let summary = water_summary_of_store(&store, world, res, threads)?;
    let record = world.summary_record(&summary)?;
    report.summary_bytes = record.payload.len();
    writer.append(&record)?;
    report.summary_seconds = started.elapsed().as_secs_f64();
    report.summary_written = true;
    Ok(())
}

/// The ocean channel of a stored bake as a [`OceanLattice`] for the water-level field (ADR 0041 point 1): it finds the block that
/// holds a vertex through the canonical layout, decodes it once and keeps the most recent ones (a small bound: a block is
/// `(B + 1)^2 / 8` bytes, 131 kB at 1024 cells). A block that is missing or damaged is an error naming the block and its key, never
/// a default (dry or wet).
pub struct StoreOceanSource<'a> {
    store: &'a TileStore,
    world: &'a GlobalWorld,
    res: u32,
    level: u8,
    cells: u32,
    budget_blocks: usize,
    decodes: AtomicU64,
    shared: Mutex<Vec<(TileId, Arc<OceanSlot>)>>,
}

/// One block of the ocean cache: filled once by the first thread that wants it.
struct OceanSlot {
    block: OnceLock<Result<Arc<OceanBits>, ReadError>>,
}

/// Default number of decoded ocean blocks a [`StoreOceanSource`] keeps: 64 (8 MB at 1024 cells).
pub const OCEAN_SOURCE_BLOCKS: usize = 64;

impl<'a> StoreOceanSource<'a> {
    pub fn new(store: &'a TileStore, world: &'a GlobalWorld, res: u32) -> Result<StoreOceanSource<'a>, BakeError> {
        StoreOceanSource::with_budget(store, world, res, OCEAN_SOURCE_BLOCKS)
    }

    /// The same keeping at most `budget_blocks` decoded blocks (at least one).
    pub fn with_budget(
        store: &'a TileStore,
        world: &'a GlobalWorld,
        res: u32,
        budget_blocks: usize,
    ) -> Result<StoreOceanSource<'a>, BakeError> {
        let (level, cells) = world.layout(res)?;
        Ok(StoreOceanSource {
            store,
            world,
            res,
            level,
            cells,
            budget_blocks: budget_blocks.max(1),
            decodes: AtomicU64::new(0),
            shared: Mutex::new(Vec::new()),
        })
    }

    /// Blocks decoded so far (a block evicted and wanted again counts again).
    pub fn decodes(&self) -> u64 {
        self.decodes.load(Ordering::Relaxed)
    }

    /// Blocks in the cache now.
    pub fn resident_blocks(&self) -> usize {
        self.shared.lock().expect("ocean cache mutex").len()
    }

    fn load(&self, tile: TileId) -> Result<Arc<OceanBits>, ReadError> {
        let key = self.world.ocean_key(tile, self.cells);
        let what = |why: String| {
            ReadError::Source(format!(
                "ocean block {:#018x} (face {} level {} x {} y {}) key {key}: {why}",
                tile.raw(),
                tile.face().0,
                tile.level(),
                tile.x(),
                tile.y()
            ))
        };
        let rec = self
            .store
            .get(KIND_GLOBAL_OCEAN, tile, key)
            .map_err(|e| what(e.to_string()))?
            .ok_or_else(|| what("not stored under the current key (build the bake again)".into()))?;
        let bits = decode_ocean_block(&rec.payload).map_err(|e| what(e.to_string()))?;
        if bits.cells != self.cells {
            return Err(what(format!("its payload has {} cells per edge, the layout says {}", bits.cells, self.cells)));
        }
        self.decodes.fetch_add(1, Ordering::Relaxed);
        Ok(Arc::new(bits))
    }

    fn block(&self, tile: TileId) -> Result<Arc<OceanBits>, ReadError> {
        let slot = {
            let mut shared = self.shared.lock().expect("ocean cache mutex");
            if let Some(at) = shared.iter().position(|(t, _)| *t == tile) {
                let entry = shared.remove(at);
                let slot = Arc::clone(&entry.1);
                shared.push(entry);
                slot
            } else {
                let slot = Arc::new(OceanSlot { block: OnceLock::new() });
                shared.push((tile, Arc::clone(&slot)));
                if shared.len() > self.budget_blocks {
                    shared.remove(0);
                }
                slot
            }
        };
        match slot.block.get_or_init(|| self.load(tile)) {
            Ok(b) => Ok(Arc::clone(b)),
            Err(e) => {
                // Not kept: the next reader reads again.
                let mut shared = self.shared.lock().expect("ocean cache mutex");
                if let Some(at) = shared.iter().position(|(t, s)| *t == tile && Arc::ptr_eq(s, &slot)) {
                    shared.remove(at);
                }
                Err(e.clone())
            }
        }
    }
}

impl OceanLattice for StoreOceanSource<'_> {
    fn res(&self) -> u32 {
        self.res
    }

    fn is_ocean(&self, face: Face, i: u32, j: u32) -> Result<bool, ReadError> {
        if i > self.res || j > self.res {
            return Err(ReadError::Source(format!("vertex ({i}, {j}) of a {}-cell grid", self.res)));
        }
        // The block that holds a vertex: the last block of a row holds the far edge as its own last sample.
        let side = 1u32 << self.level;
        let shift = self.cells.trailing_zeros();
        let (bx, by) = ((i >> shift).min(side - 1), (j >> shift).min(side - 1));
        let tile = TileId::new(face, self.level, bx, by).expect("a tile of the grid");
        Ok(self.block(tile)?.get(i - bx * self.cells, j - by * self.cells))
    }
}

/// One ocean block of the store against the channel recomputed from the stored sample blocks (the sample blocks themselves are
/// compared with regeneration by [`verify_global`]): the same rules as for sample blocks, decode first, a record under another key
/// stale, a record under the world's key equal bit for bit.
fn verify_ocean_record(
    store: &TileStore,
    world: &GlobalWorld,
    stored: &crate::pack::Stored,
    reference: &mut Option<Result<OceanTable, String>>,
    report: &mut VerifyReport,
) {
    let r = &stored.record;
    let where_ = format!("{}: ocean block {:#018x} key {} at offset {}", stored.pack, r.tile.raw(), r.key, stored.offset);
    let kept = match decode_ocean_block(&r.payload) {
        Ok(b) => b,
        Err(e) => {
            report.unreadable.push(format!("{where_}: {e}"));
            return;
        }
    };
    let cells = kept.cells;
    let Some(res) = grid_res(r.tile, cells) else {
        report.unreadable.push(format!("{where_}: {cells} cells per block at level {} is not a grid this build bakes", r.tile.level()));
        return;
    };
    if world.layout(res).ok() != Some((r.tile.level(), cells)) {
        report.unreadable.push(format!(
            "{where_}: {cells} cells per block at level {} is not the canonical layout of a {res}-cell grid",
            r.tile.level()
        ));
        return;
    }
    let metadata_current = r.generator_version == GLOBAL_VERSION && r.hash_version == HASH_VERSION && r.node_set_id == world.node_set_id;
    if world.ocean_key(r.tile, cells) != r.key {
        if metadata_current {
            report.unexplained_stale += 1;
        } else {
            report.stale += 1;
        }
        return;
    }
    let mismatch = |detail: String| Mismatch { tile: r.tile, key: r.key, pack: stored.pack.clone(), offset: stored.offset, detail };
    if !metadata_current {
        report.mismatches.push(mismatch(
            "the key is the world's but the record disagrees with it in its versions or node set id: the record was written wrongly".into(),
        ));
        return;
    }
    let table = reference.get_or_insert_with(|| {
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        ocean_table_of_store(store, world, res, threads).map_err(|e| e.to_string())
    });
    let table = match table {
        Ok(t) if t.res == res => t,
        Ok(t) => {
            report.unreadable.push(format!("{where_}: the store holds ocean blocks of a {res}-cell bake and of a {}-cell bake", t.res));
            return;
        }
        Err(e) => {
            report.unreadable.push(format!("{where_}: the ocean channel cannot be recomputed from the stored blocks: {e}"));
            return;
        }
    };
    let fresh = stored_block(store, world, r.tile, cells)
        .and_then(|samples| ocean_bits(res, r.tile, cells, &samples, table).map_err(GlobalBuildError::from));
    match fresh {
        Err(e) => report.unreadable.push(format!("{where_}: {e}")),
        Ok(fresh) if fresh == kept => report.matching += 1,
        Ok(fresh) => {
            let n = cells + 1;
            let first = (0..n * n).find(|i| fresh.get(i % n, i / n) != kept.get(i % n, i / n)).unwrap_or(0);
            let count = (0..n * n).filter(|i| fresh.get(i % n, i / n) != kept.get(i % n, i / n)).count();
            report.mismatches.push(mismatch(format!(
                "vertex ({}, {}): stored {}, recomputed {}; {count} of {} vertices differ: an input of the ocean channel is missing from its key",
                first % n,
                first / n,
                kept.get(first % n, first / n),
                fresh.get(first % n, first / n),
                n * n
            )));
        }
    }
}

/// The water summary record of the store against the summary recomputed from the stored sample and ocean blocks: decode first, a
/// record under another key stale, a record under the world's key equal cell for cell.
fn verify_summary_record(store: &TileStore, world: &GlobalWorld, stored: &crate::pack::Stored, report: &mut VerifyReport) {
    let r = &stored.record;
    let where_ = format!("{}: water summary {:#018x} key {} at offset {}", stored.pack, r.tile.raw(), r.key, stored.offset);
    let kept = match decode_summary(&r.payload) {
        Ok(s) => s,
        Err(e) => {
            report.unreadable.push(format!("{where_}: {e}"));
            return;
        }
    };
    let (res, cells) = (kept.res(), kept.block_cells());
    if world.layout(res).ok().map(|(_, c)| c) != Some(cells) {
        report.unreadable.push(format!("{where_}: {cells} cells per block is not the canonical layout of a {res}-cell grid"));
        return;
    }
    let metadata_current = r.generator_version == GLOBAL_VERSION && r.hash_version == HASH_VERSION && r.node_set_id == world.node_set_id;
    if world.summary_key(res, cells) != r.key || r.tile != GlobalWorld::summary_tile() {
        if metadata_current {
            report.unexplained_stale += 1;
        } else {
            report.stale += 1;
        }
        return;
    }
    let mismatch = |detail: String| Mismatch { tile: r.tile, key: r.key, pack: stored.pack.clone(), offset: stored.offset, detail };
    if !metadata_current {
        report.mismatches.push(mismatch(
            "the key is the world's but the record disagrees with it in its versions or node set id: the record was written wrongly".into(),
        ));
        return;
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    match water_summary_of_store(store, world, res, threads) {
        Err(e) => report.unreadable.push(format!("{where_}: the summary cannot be recomputed from the stored blocks: {e}")),
        Ok(fresh) => {
            let differ = |i: &usize| {
                let (a, b) = (fresh.cells()[*i], kept.cells()[*i]);
                a.flags != b.flags || a.band_elevation_m.to_bits() != b.band_elevation_m.to_bits()
            };
            match (0..fresh.cells().len()).find(differ) {
                None => report.matching += 1,
                Some(first) => {
                    let count = (0..fresh.cells().len()).filter(differ).count();
                    report.mismatches.push(mismatch(format!(
                        "cell {first}: stored {:?}, recomputed {:?}; {count} of {} cells differ: an input of the summary is missing from its key",
                        kept.cells()[first],
                        fresh.cells()[first],
                        fresh.cells().len()
                    )));
                }
            }
        }
    }
}

/// Verify `sample` evenly spaced block records of the store (all of them when `sample` is 0) against regeneration: the
/// same rules as for height tiles (STRM-004): every record is decoded first; a record under the key the world gives must carry
/// the world's metadata and equal its regeneration bit for bit; a record under another key is stale (explained when its
/// versions or node set id differ, without a known reason when they are all equal). Records of another kind are counted.
pub fn verify_global(store: &TileStore, world: &GlobalWorld, sample: usize) -> VerifyReport {
    let mut report = VerifyReport::default();
    report.unreadable.extend(store.damaged().iter().map(ToString::to_string));
    // The ocean table of the stored bake, computed once when the first ocean block is checked.
    let mut ocean_reference: Option<Result<OceanTable, String>> = None;
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
        if r.kind == KIND_GLOBAL_OCEAN {
            verify_ocean_record(store, world, &stored, &mut ocean_reference, &mut report);
            continue;
        }
        if r.kind == KIND_GLOBAL_WATER_SUMMARY {
            verify_summary_record(store, world, &stored, &mut report);
            continue;
        }
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
        // A forged record whose cells and level make no grid the tiled bake accepts is unreadable, never keyed.
        let Some(res) = grid_res(r.tile, cells) else {
            report.unreadable.push(format!("{where_}: {cells} cells per block at level {} is not a grid this build bakes", r.tile.level()));
            continue;
        };
        // Only the canonical layout of a grid is stored (ADR 0027 point 1): another one would double the bake.
        if world.layout(res).ok() != Some((r.tile.level(), cells)) {
            report.unreadable.push(format!(
                "{where_}: {cells} cells per block at level {} is not the canonical layout of a {res}-cell grid (blocks of min(res, {}) cells)",
                r.tile.level(),
                world.block_limit
            ));
            continue;
        }
        let metadata_current =
            r.generator_version == GLOBAL_VERSION && r.hash_version == HASH_VERSION && r.node_set_id == world.node_set_id;
        if world.block_key(r.tile, cells) != r.key {
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
