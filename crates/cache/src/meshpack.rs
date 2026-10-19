//! Pre-filled coarse tiles in the tile pack (STRM-021, ADR 0045 point 3).
//!
//! The bake step also writes the finished render tiles of levels `F..=N` (F the render floor, default 3, LOD-011; N default 4) into
//! the pack of the bake, as records of
//! kind 2 ([`Kind::Mesh`]) under the existing ADR 0018 key of the tile's mesh (`mesh_key_inputs`) plus one output parameter
//! naming the form of the tile ([`PARAM_RENDER_FORM`], always the compact form since ADR 0062), so the streamer reads a coarse tile
//! from disk instead of generating it
//! (a level-0 tile reads about 320 bake blocks: 6.6 s on one thread). The payload is the tile exactly as the generators build it
//! (every `f32`, `u16` and `f64` bit), so a pre-filled tile equals the on-demand one bit for bit (CON-14), whichever thread
//! count wrote it.
//!
//! Payload: `codec version (u8) | form (u8: 1 compact) | cells (u32 LE) | raw length (u32 LE) | LZ4 block`. The raw bytes are the
//! tile's mesh key (u64), its origin (3 x f64), the relief (f64) and the slot of GEN-025 byte for byte ([`CompactTile::slot_data`]:
//! header, climate block, pad, texels). Everything is little endian. The decoder never allocates more than `cells` allows.
//!
//! Version 2 (MESH_VERSION 7, ADR 0039 point 2): the vertex form gained the climate block. A version-1 payload (and every record
//! under a version-6 key, which the new keys never name) is a miss: [`decode_render_tile`] refuses the version, [`TileSpec::lookup`]
//! turns that into `None`, and the pre-fill writes the tile again.
//!
//! Version 3 (MESH_VERSION 8, ADR 0062, GEN-025): the compact form is the slot (wide texel, climate block in the slot). A version-1 or
//! version-2 payload is refused naming the version, and records written under MESH_VERSION 7 are unreachable (the version is in the key).
//!
//! The vertex form (form byte 0) was removed with the vertex render path (`remove-vertex-render-path`, ADR 0062): a payload that
//! claims it is refused naming the form. The payload version stays 3 and the key parameter [`PARAM_RENDER_FORM`] stays the value 1
//! compact tiles always had, so no record of an existing pack changes its key or its bytes.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::hash::HASH_VERSION;
use planet_core::{Face, PlanetFixed, TileId, Vec3};
use planet_generators::compact::{compact_tile_field, slot_bytes, CompactTile, COMPACT_MIN_LEVEL};
use planet_generators::height::HeightField;
use planet_world_def::{body_radius_path, ReadRecord};

use crate::key::{cache_key, mesh_key_inputs, CacheKey, Kind};
use crate::pack::{PackError, PackWriter, Record, StoreParams, TileStore};

/// The kind byte of a pre-filled render tile.
pub const KIND_MESH: u8 = Kind::Mesh as u8;

/// Output parameter tag: the form of the stored tile.
pub const PARAM_RENDER_FORM: u8 = 5;

/// The value of [`PARAM_RENDER_FORM`] and the form byte of a payload: the compact tile. (Form 0, the vertex tile, is gone; the value
/// stays 1 so the keys of the records of an existing pack are unchanged.)
pub const COMPACT_FORM: u8 = 1;

/// Version of the payload layout. History: 1 = LZ4 block of the exact tile; 2 = the vertex form carries the climate block;
/// 3 = the compact form is the slot of GEN-025 (`MESH_VERSION` 8).
pub const PAYLOAD_VERSION: u8 = 3;

/// The default depth of the pre-fill: levels from the render floor to this one (1,920 tiles at floor 3, 2,046 at floor 0).
pub const DEFAULT_PREFILL_LEVELS: u8 = 4;

/// The deepest level a pre-fill may name (4^8 x 6 tiles at level 8 alone are 393,216): a guard against a typo, and the deepest
/// level a reader looks up.
pub const MAX_PREFILL_LEVELS: u8 = 8;

/// Most cells per tile edge a payload may claim: a hostile header must not make a decoder allocate gigabytes.
const MAX_CELLS: u32 = 1024;

const HEADER_LEN: usize = 10;

/// Why a payload could not be encoded or decoded. Every variant says what was wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshCodecError {
    Short,
    Version(u8),
    Form(u8),
    Cells(u32),
    RawLength {
        claimed: u32,
        expected: u64,
    },
    Lz4(String),
    /// Encoding: the tile's own counts do not match its cells.
    Shape(String),
}

impl std::fmt::Display for MeshCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeshCodecError::Short => write!(f, "the tile payload is shorter than its header"),
            MeshCodecError::Version(v) => write!(f, "tile payload version {v}, this build reads {PAYLOAD_VERSION}"),
            MeshCodecError::Form(v) => {
                write!(f, "tile payload form {v} is not the compact form ({COMPACT_FORM}); the vertex form (0) is no longer read")
            }
            MeshCodecError::Cells(c) => write!(f, "{c} cells per edge is not a power of two from 2 to {MAX_CELLS}"),
            MeshCodecError::RawLength { claimed, expected } => {
                write!(f, "the tile payload claims {claimed} raw bytes, {expected} are needed for its cells")
            }
            MeshCodecError::Lz4(e) => write!(f, "the tile payload's LZ4 block is damaged: {e}"),
            MeshCodecError::Shape(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MeshCodecError {}

fn valid_cells(cells: u32) -> bool {
    (2..=MAX_CELLS).contains(&cells) && cells.is_power_of_two()
}

fn put_f64(out: &mut Vec<u8>, v: f64) {
    out.extend_from_slice(&v.to_bits().to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let out: [u8; N] = self.bytes[self.at..self.at + N].try_into().expect("lengths are checked before reading");
        self.at += N;
        out
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take())
    }
    fn f64(&mut self) -> f64 {
        f64::from_bits(self.u64())
    }
}

/// Raw bytes of the fixed part (key, origin) and the relief a compact tile starts with.
const COMPACT_FIXED: usize = 8 + 3 * 8 + 8;

fn raw_len(cells: u32) -> u64 {
    COMPACT_FIXED as u64 + slot_bytes(cells)
}

/// Encode a render tile as the payload of a kind-2 record.
pub fn encode_render_tile(tile: &CompactTile) -> Result<Vec<u8>, MeshCodecError> {
    let cells = tile.cells;
    if !valid_cells(cells) {
        return Err(MeshCodecError::Cells(cells));
    }
    let mut raw: Vec<u8> = Vec::with_capacity(raw_len(cells) as usize);
    raw.extend_from_slice(&tile.key.to_le_bytes());
    let o = tile.origin.0;
    for v in [o.x, o.y, o.z] {
        put_f64(&mut raw, v);
    }
    let texels = (cells as usize + 1) * (cells as usize + 1);
    if tile.texels.len() != texels {
        return Err(MeshCodecError::Shape(format!(
            "the compact tile has {} texels, {texels} are needed for {cells} cells",
            tile.texels.len()
        )));
    }
    put_f64(&mut raw, tile.relief_m);
    raw.extend_from_slice(&tile.slot_data());
    debug_assert_eq!(raw.len() as u64, raw_len(cells));
    let mut out = Vec::with_capacity(HEADER_LEN + raw.len() / 2);
    out.push(PAYLOAD_VERSION);
    out.push(COMPACT_FORM);
    out.extend_from_slice(&cells.to_le_bytes());
    out.extend_from_slice(&u32::try_from(raw.len()).expect("a tile payload is far below 4 GiB").to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress(&raw));
    Ok(out)
}

/// The cells a payload claims, if its header is well formed (version, compact form, valid cells).
pub fn payload_cells(bytes: &[u8]) -> Result<u32, MeshCodecError> {
    if bytes.len() < HEADER_LEN {
        return Err(MeshCodecError::Short);
    }
    if bytes[0] != PAYLOAD_VERSION {
        return Err(MeshCodecError::Version(bytes[0]));
    }
    if bytes[1] != COMPACT_FORM {
        return Err(MeshCodecError::Form(bytes[1]));
    }
    let cells = u32::from_le_bytes(bytes[2..6].try_into().expect("4 bytes"));
    if !valid_cells(cells) {
        return Err(MeshCodecError::Cells(cells));
    }
    Ok(cells)
}

/// Decode the payload of the render tile `id`.
pub fn decode_render_tile(id: TileId, bytes: &[u8]) -> Result<CompactTile, MeshCodecError> {
    let cells = payload_cells(bytes)?;
    let claimed = u32::from_le_bytes(bytes[6..10].try_into().expect("4 bytes"));
    let expected = raw_len(cells);
    if u64::from(claimed) != expected {
        return Err(MeshCodecError::RawLength { claimed, expected });
    }
    let raw = lz4_flex::decompress(&bytes[HEADER_LEN..], claimed as usize).map_err(|e| MeshCodecError::Lz4(e.to_string()))?;
    if raw.len() as u64 != expected {
        return Err(MeshCodecError::Lz4(format!("the block gives {} raw bytes, {expected} are needed", raw.len())));
    }
    let mut r = Reader { bytes: &raw, at: 0 };
    let key = r.u64();
    let origin = PlanetFixed(Vec3::new(r.f64(), r.f64(), r.f64()));
    let relief_m = r.f64();
    let slot = &raw[r.at..];
    CompactTile::from_slot_data(id, cells, key, origin, relief_m, slot)
        .map_err(|e| MeshCodecError::Shape(format!("the compact slot is damaged: {e}")))
}

/// The persistent key of the pre-filled tile `tile`: the ADR 0018 mesh key with the form (always compact) as an output parameter.
pub fn render_key(field: &dyn HeightField, map: &dyn FaceMapping, tile: TileId, cells: u32, radius_m: f64) -> CacheKey {
    let mut inputs = mesh_key_inputs(field, map, tile, cells, radius_m);
    inputs.output_params.insert(PARAM_RENDER_FORM, vec![COMPACT_FORM]);
    cache_key(&inputs)
}

/// The id stored in every record of a field's tiles: BLAKE3-128 (context `planet-builder node set v1`) of the sorted,
/// deduplicated, length-prefixed paths of the field's reads and of the body's radius node (ADR 0025 point 3), as for height tiles.
pub fn node_set_id_of(field: &dyn HeightField) -> [u8; 16] {
    node_set_id_of_reads(field.body(), &field.read_records())
}

/// The node set id of one tile (STRM-010, ADR 0025 point 3). A field that reads no items has one id for the world
/// ([`node_set_id_of`]); a field that reads items (`reads_cells`) has one per tile: the paths of the tile's own reads, which name
/// each item query by kind, body, layer, tile and halo and never by what it found, so the id depends on which reads the tile makes
/// and not on the items. `map` is the mapping the tile id was made with.
pub fn node_set_id_of_tile(field: &dyn HeightField, tile: TileId, map: &dyn FaceMapping) -> [u8; 16] {
    if field.reads_cells() {
        node_set_id_of_reads(field.body(), &field.read_records_for(tile, map))
    } else {
        node_set_id_of(field)
    }
}

fn node_set_id_of_reads(body: planet_world_def::BodyId, records: &[ReadRecord]) -> [u8; 16] {
    let mut paths: Vec<String> = records.iter().map(ReadRecord::path).collect();
    paths.push(body_radius_path(body));
    paths.sort();
    paths.dedup();
    let mut material = Vec::new();
    for p in &paths {
        material.extend_from_slice(&u32::try_from(p.len()).expect("a node path is under 4 GiB").to_le_bytes());
        material.extend_from_slice(p.as_bytes());
    }
    blake3::derive_key("planet-builder node set v1", &material)[..16].try_into().expect("16 bytes")
}

/// What identifies the pack of a body-0 world.
pub fn store_params() -> StoreParams {
    StoreParams { hash_version: HASH_VERSION, body: 0 }
}

/// What a run needs to know to make or find a pre-filled tile.
#[derive(Clone)]
pub struct TileSpec<'a> {
    field: &'a dyn HeightField,
    radius_m: f64,
    /// Cells per tile edge of the run (the product 32).
    cells: u32,
    /// The run's finest level (the LOD cap): a tile at or below it is a finest tile and is never pre-filled (a finest tile is seen
    /// at one place at a time, not worth a pack record).
    max_level: u8,
    /// The render floor of the run (LOD-011): a tile of a lower level is never drawn, so never stored or looked up. 0 by default.
    floor: u8,
    node_set_id: std::sync::OnceLock<[u8; 16]>,
}

impl<'a> TileSpec<'a> {
    pub fn new(field: &'a dyn HeightField, radius_m: f64, cells: u32, max_level: u8) -> TileSpec<'a> {
        TileSpec { field, radius_m, cells, max_level, floor: 0, node_set_id: std::sync::OnceLock::new() }
    }

    /// The same run with render floor `floor` (the effective floor of LOD-011): the pre-fill covers levels `floor..=N` and a tile
    /// below the floor is not storable.
    pub fn with_floor(mut self, floor: u8) -> TileSpec<'a> {
        self.floor = floor;
        self
    }

    pub fn floor(&self) -> u8 {
        self.floor
    }

    pub fn max_level(&self) -> u8 {
        self.max_level
    }

    /// The tile as the streamer generates it on demand.
    pub fn generate(&self, tile: TileId) -> CompactTile {
        compact_tile_field(self.field, &TangentWarp, tile, self.cells, self.radius_m)
    }

    /// Whether a tile may be read from the pack: not below the render floor or level 3 (such a tile is never drawn), shallow enough to be
    /// pre-filled, and not a finest tile of this run.
    pub fn storable(&self, tile: TileId) -> bool {
        tile.level() >= self.floor.max(COMPACT_MIN_LEVEL) && tile.level() <= MAX_PREFILL_LEVELS && tile.level() < self.max_level
    }

    fn key_of(&self, tile: TileId) -> CacheKey {
        render_key(self.field, &TangentWarp, tile, self.cells, self.radius_m)
    }

    /// The record of a generated tile.
    pub fn record(&self, tile: &CompactTile) -> Result<Record, MeshCodecError> {
        Ok(Record {
            key: self.key_of(tile.id),
            kind: KIND_MESH,
            tile: tile.id,
            // The version of the field whose key this is: a tile a stage passes through is the base's (STRM-029).
            generator_version: planet_generators::height::key_source(self.field, tile.id, &TangentWarp).version(),
            hash_version: HASH_VERSION,
            node_set_id: *self.node_set_id.get_or_init(|| node_set_id_of(self.field)),
            payload: encode_render_tile(tile)?,
        })
    }

    /// The pre-filled tile from the pack, if it holds one under the current key and the payload decodes (a cache degrades to
    /// a miss: damage, another version and a tile that is not storable are `None`, the caller generates).
    pub fn lookup(&self, store: &TileStore, tile: TileId) -> Option<CompactTile> {
        if !self.storable(tile) {
            return None;
        }
        let rec = store.get(KIND_MESH, tile, self.key_of(tile)).ok()??;
        let out = decode_render_tile(tile, &rec.payload).ok()?;
        (out.cells == self.cells).then_some(out)
    }
}

/// The tiles of levels `0..=levels` in the order the pre-fill works and writes them: level, face, row, column.
pub fn prefill_tiles(levels: u8) -> Vec<TileId> {
    prefill_tiles_from(0, levels)
}

/// The tiles of levels `floor..=levels` (the pre-fill of a run with render floor `floor`), in the order of [`prefill_tiles`]. None
/// when `levels` is below `floor`.
pub fn prefill_tiles_from(floor: u8, levels: u8) -> Vec<TileId> {
    let mut out = Vec::new();
    for level in floor..=levels {
        let n = 1u32 << level;
        for face in 0..6u8 {
            for y in 0..n {
                for x in 0..n {
                    out.push(TileId::new(Face(face), level, x, y).expect("a tile of its own grid"));
                }
            }
        }
    }
    out
}

/// Count of tiles in levels `0..=levels`: `6 (4^(levels + 1) - 1) / 3`.
pub fn prefill_tile_count(levels: u8) -> usize {
    6 * ((1usize << (2 * (usize::from(levels) + 1))) - 1) / 3
}

/// Count of tiles in levels `floor..=levels`: 1,920 for floor 3 and levels 4, 384 for floor 3 and levels 3, 0 below the floor.
pub fn prefill_tile_count_from(floor: u8, levels: u8) -> usize {
    if levels < floor {
        return 0;
    }
    prefill_tile_count(levels) - if floor == 0 { 0 } else { prefill_tile_count(floor - 1) }
}

/// Why a pre-fill failed.
#[derive(Debug)]
pub enum PrefillError {
    /// The depth or the run's finest level cannot hold a pre-fill.
    Levels(String),
    Codec(MeshCodecError),
    Pack(PackError),
}

impl std::fmt::Display for PrefillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrefillError::Levels(s) => write!(f, "{s}"),
            PrefillError::Codec(e) => write!(f, "{e}"),
            PrefillError::Pack(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PrefillError {}

impl From<PackError> for PrefillError {
    fn from(e: PackError) -> Self {
        PrefillError::Pack(e)
    }
}

impl From<MeshCodecError> for PrefillError {
    fn from(e: MeshCodecError) -> Self {
        PrefillError::Codec(e)
    }
}

/// What a pre-fill did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefillReport {
    /// Tiles in levels `floor..=levels`.
    pub total: usize,
    /// Tiles the pack already held (under the current key, payload decoding): skipped.
    pub present: usize,
    /// Tiles generated and appended.
    pub written: usize,
    /// Payload bytes of the tiles appended.
    pub payload_bytes: u64,
}

/// Which of the tiles of levels `floor..=levels` (the floor of `spec`) the pack lacks.
pub fn missing_tiles(store: &TileStore, spec: &TileSpec<'_>, levels: u8) -> Vec<TileId> {
    prefill_tiles_from(spec.floor, levels).into_iter().filter(|t| spec.lookup(store, *t).is_none()).collect()
}

/// Write the tiles of levels `floor..=levels` (the floor of `spec`, 0 by default) the pack of `dir` lacks. A `levels` below the
/// floor writes nothing and opens nothing (the caller says so). Tiles are generated on `threads` threads, heaviest (coarsest)
/// first, and appended in the fixed order of [`prefill_tiles`], so the records, their order and the bytes of the pack do not
/// depend on the thread count (CON-14). A tile the pack already holds under the current key, whose payload decodes, is skipped
/// and a run that finds nothing missing opens no writer and changes no byte. `progress` is called after each level with
/// `(level, tiles of it generated)`.
pub fn prefill(
    dir: &Path,
    spec: &TileSpec<'_>,
    levels: u8,
    threads: usize,
    max_pack_bytes: u64,
    progress: &(dyn Fn(u8, usize) + Sync),
) -> Result<PrefillReport, PrefillError> {
    if levels > MAX_PREFILL_LEVELS {
        return Err(PrefillError::Levels(format!(
            "a pre-fill of levels {}..={levels} is too deep (at most {MAX_PREFILL_LEVELS})",
            spec.floor
        )));
    }
    if levels < spec.floor {
        return Ok(PrefillReport { total: 0, present: 0, written: 0, payload_bytes: 0 });
    }
    if spec.cells < planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS && levels >= COMPACT_MIN_LEVEL {
        return Err(PrefillError::Levels(format!(
            "the compact tile format needs at least {} cells per tile edge (the climate of a tile rides in the spare channel of its first 162 texels, GEN-021), got {}: use more cells",
            planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS,
            spec.cells
        )));
    }
    if levels >= spec.max_level {
        return Err(PrefillError::Levels(format!(
            "a pre-fill of levels {}..={levels} reaches the finest level {} of the view: such tiles are never stored",
            spec.floor, spec.max_level
        )));
    }
    if spec.floor < COMPACT_MIN_LEVEL {
        return Err(PrefillError::Levels(format!(
            "a pre-fill from level {} reaches below level {COMPACT_MIN_LEVEL}, the first level of the compact format (the only format; the render floor is at least {COMPACT_MIN_LEVEL})",
            spec.floor
        )));
    }
    let store = TileStore::open(dir, store_params())?;
    let all = prefill_tiles_from(spec.floor, levels);
    let total = all.len();
    let missing = missing_tiles(&store, spec, levels);
    let present = total - missing.len();
    if missing.is_empty() {
        return Ok(PrefillReport { total, present, written: 0, payload_bytes: 0 });
    }
    drop(store);
    // Generate in parallel, heaviest first (the order of `prefill_tiles` is by level); keep each result at its own index so the
    // append order is the fixed one whichever thread finished first.
    let results: Vec<Mutex<Option<Result<Record, MeshCodecError>>>> = missing.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let done_per_level: Vec<AtomicUsize> = (0..=levels).map(|_| AtomicUsize::new(0)).collect();
    let in_level: Vec<usize> = (0..=levels).map(|l| missing.iter().filter(|t| t.level() == l).count()).collect();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(tile) = missing.get(i) else { break };
                let made = spec.record(&spec.generate(*tile));
                *results[i].lock().expect("a result slot") = Some(made);
                let level = usize::from(tile.level());
                if done_per_level[level].fetch_add(1, Ordering::SeqCst) + 1 == in_level[level] {
                    progress(tile.level(), in_level[level]);
                }
            });
        }
    });
    let mut writer = PackWriter::open(dir, store_params(), max_pack_bytes)?;
    let mut payload_bytes = 0u64;
    for slot in results {
        let record = slot.into_inner().expect("a result slot").expect("every missing tile was generated")?;
        payload_bytes += record.payload.len() as u64;
        writer.append(&record)?;
    }
    writer.finish()?;
    Ok(PrefillReport { total, present, written: missing.len(), payload_bytes })
}
