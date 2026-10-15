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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use planet_core::cube::TangentWarp;
use planet_core::hash::HASH_VERSION;
use planet_core::{Face, FaceMapping, TileId};
use planet_generators::global::{
    bake_block, bake_blocks_where, check_tiled, BakeError, GlobalCoarseGenerator, GlobalSample, LatticeSource, ReadError, GLOBAL_VERSION,
    IMPLEMENTATION_ID, MAX_BLOCK_CELLS, MAX_TILED_RES,
};
use planet_generators::stage::{FieldGraph, FieldSpec, Scope, Stage};
use planet_world_def::{body_radius_path, body_seed_path, BodyId, GenContext, MissingNode, NodeValue, WorldDefinition, SPHERE_SURFACE_ID};

use crate::codec::shuffle;
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
        Ok(GlobalWorld { seed: field.seed(), body, radius_m, reads, node_set_id, graph, block_limit: MAX_BLOCK_CELLS })
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

    /// Whether a stored global block is still current for this world: its generator version, hash version and node set id
    /// equal the world's (STRM-013). Other kinds are not this world's to judge: `true`.
    pub fn is_live(&self, record: &Record) -> bool {
        record.kind != KIND_GLOBAL_BLOCK
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
/// Reads by many threads do not meet on a lock. Each thread keeps the block it read last (one `Arc`, checked against the
/// source's eviction epoch with one relaxed load), so a run of reads inside a block touches no shared memory but that load.
/// Only a change of block goes to the shared cache: a least-recently-used list under a mutex, bounded by a byte budget, whose
/// entries are shared decode slots, so threads that want the same absent block wait for one decode. A block that fails to read
/// is not kept. Whatever served a read, it returns the stored bits.
///
/// Memory: blocks resident in the shared cache never exceed the budget (at least one block); a thread pins at most one block
/// more (the one it read last) until its next read after an eviction, or for good if it never reads again, and a thread that
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
    /// Bumped (under the cache lock) when a block leaves the shared cache: a thread's pinned block is valid for one epoch.
    epoch: AtomicU64,
    decodes: AtomicU64,
    /// Acquisitions of the shared-cache lock by reads (a test observable of STRM-014: only a change of block takes it).
    lookups: AtomicU64,
    shared: Mutex<Vec<(TileId, Arc<Slot>)>>,
}

/// One block of the shared cache: filled once by the first thread that wants it.
struct Slot {
    block: OnceLock<Result<Arc<Vec<GlobalSample>>, ReadError>>,
}

/// What the shared block cache holds, for budgets and tests (CON-20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceStats {
    /// Blocks decoded so far (a block evicted and wanted again counts again).
    pub decodes: u64,
    /// Times a read took the shared-cache lock: one per change of block that the thread's pinned block did not serve.
    pub shared_lookups: u64,
    /// Blocks in the shared cache, including blocks being decoded.
    pub resident_blocks: usize,
    pub resident_bytes: u64,
    pub budget_bytes: u64,
}

/// Default budget of decoded blocks a [`StoreSource`] keeps: 512 MiB, ADR 0033 point 6's proposal for bake blocks (31 blocks of
/// 1024 cells; it was 16 blocks).
pub const SOURCE_CACHE_BYTES: u64 = 512 << 20;

/// The block a thread read last.
struct Pinned {
    source: u64,
    epoch: u64,
    /// Face, block column and block row.
    at: (u8, u32, u32),
    block: Arc<Vec<GlobalSample>>,
}

thread_local! {
    static PINNED: RefCell<Option<Pinned>> = const { RefCell::new(None) };
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
            epoch: AtomicU64::new(0),
            decodes: AtomicU64::new(0),
            lookups: AtomicU64::new(0),
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
            budget_bytes: self.budget_blocks as u64 * self.block_bytes,
        }
    }

    /// The slot of `tile` and the epoch it is valid for, made if absent (least recently used blocks leave beyond the budget).
    fn slot(&self, tile: TileId) -> (Arc<Slot>, u64) {
        self.lookups.fetch_add(1, Ordering::Relaxed);
        let mut shared = self.shared.lock().expect("block cache mutex");
        if let Some(at) = shared.iter().position(|(t, _)| *t == tile) {
            let entry = shared.remove(at);
            let slot = Arc::clone(&entry.1);
            shared.push(entry);
            return (slot, self.epoch.load(Ordering::Relaxed));
        }
        let slot = Arc::new(Slot { block: OnceLock::new() });
        shared.push((tile, Arc::clone(&slot)));
        if shared.len() > self.budget_blocks {
            shared.remove(0);
            self.epoch.fetch_add(1, Ordering::Relaxed);
        }
        (slot, self.epoch.load(Ordering::Relaxed))
    }

    fn forget(&self, tile: TileId, slot: &Arc<Slot>) {
        let mut shared = self.shared.lock().expect("block cache mutex");
        if let Some(at) = shared.iter().position(|(t, s)| *t == tile && Arc::ptr_eq(s, slot)) {
            shared.remove(at);
            self.epoch.fetch_add(1, Ordering::Relaxed);
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
        let rec = self
            .store
            .get(KIND_GLOBAL_BLOCK, tile, key)
            .map_err(|e| what(e.to_string()))?
            .ok_or_else(|| what("not stored under the current key".into()))?;
        let (cells, samples) = decode_block(&rec.payload).map_err(|e| what(e.to_string()))?;
        if cells != self.cells {
            return Err(what(format!("its payload has {cells} cells per edge, the layout says {}", self.cells)));
        }
        self.decodes.fetch_add(1, Ordering::Relaxed);
        Ok(Arc::new(samples))
    }

    /// The block of `tile` from the shared cache (decoded by this thread or another), and the epoch to pin it for.
    fn shared_block(&self, tile: TileId) -> Result<(Arc<Vec<GlobalSample>>, u64), ReadError> {
        let (slot, epoch) = self.slot(tile);
        match slot.block.get_or_init(|| self.load(tile)) {
            Ok(block) => Ok((Arc::clone(block), epoch)),
            Err(e) => {
                // Not kept: the next reader reads again (the store may have gained the block).
                self.forget(tile, &slot);
                Err(e.clone())
            }
        }
    }

    fn sample(&self, at: (u8, u32, u32), index: usize) -> Result<GlobalSample, ReadError> {
        let epoch = self.epoch.load(Ordering::Relaxed);
        let hit =
            PINNED.with(|p| p.borrow().as_ref().filter(|p| p.source == self.id && p.epoch == epoch && p.at == at).map(|p| p.block[index]));
        if let Some(s) = hit {
            return Ok(s);
        }
        let tile = TileId::new(Face(at.0), self.level, at.1, at.2).expect("a tile of the grid");
        let (block, epoch) = self.shared_block(tile)?;
        let s = block[index];
        PINNED.with(|p| *p.borrow_mut() = Some(Pinned { source: self.id, epoch, at, block }));
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

/// Bake the grid of `res` cells per face edge into the store in `dir` in its canonical layout (ADR 0027 point 1: blocks of
/// `min(res, limit)` cells, the tiles of the level that covers the grid). A block the store already holds under the current
/// key, and whose payload decodes, is reused and not computed, so a second run writes nothing.
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
    let (mut written, mut failed) = (0usize, None);
    let stop = AtomicBool::new(false);
    let stats = bake_blocks_where(
        world.seed(),
        &TangentWarp,
        level,
        cells,
        threads,
        &|t| !stop.load(Ordering::SeqCst) && !have(t),
        &mut |tile, samples| {
            if failed.is_some() {
                return;
            }
            match world
                .block_record(tile, cells, &samples)
                .map_err(GlobalBuildError::from)
                .and_then(|r| writer.append(&r).map_err(Into::into))
            {
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
