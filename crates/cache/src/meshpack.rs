//! Pre-filled coarse tiles in the tile pack (STRM-021, ADR 0045 point 3).
//!
//! The bake step also writes the finished render tiles of levels `0..=N` (default 4) into the pack of the bake, as records of
//! kind 2 ([`Kind::Mesh`]) under the existing ADR 0018 key of the tile's mesh (`mesh_key_inputs`) plus one output parameter
//! naming the form of the tile ([`PARAM_RENDER_FORM`]), so the streamer reads a coarse tile from disk instead of generating it
//! (a level-0 tile reads about 320 bake blocks: 6.6 s on one thread). The payload is the tile exactly as the generators build it
//! (every `f32`, `u16` and `f64` bit), so a pre-filled tile equals the on-demand one bit for bit (CON-14), whichever thread
//! count wrote it.
//!
//! Payload: `codec version (u8) | form (u8: 0 vertex, 1 compact) | cells (u32 LE) | raw length (u32 LE) | LZ4 block`. The raw
//! bytes are the tile's mesh key (u64), its origin (3 x f64), then for the vertex form the `vertex_count(cells)` vertices as 11
//! `f32` each (byte-shuffled as in the height codec) and the 324 bytes of the climate field ([`ClimateTexture::to_bytes`]), for the compact form the 9 control points, `h_origin`, `hr_min`,
//! `hr_step`, `skirt_depth` (f32), the flip flag (u8), the relief (f64) and the `(cells + 1)^2` texels of 4 `u16` (the fourth channel
//! carries the climate field). Everything is little endian. The decoder never allocates more than `cells` allows.
//!
//! Version 2 (MESH_VERSION 7, ADR 0039 point 2): the vertex form gained the climate block. A version-1 payload (and every record
//! under a version-6 key, which the new keys never name) is a miss: [`decode_render_tile`] refuses the version, [`TileSpec::lookup`]
//! turns that into `None`, and the pre-fill writes the tile again.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::hash::HASH_VERSION;
use planet_core::{Face, PlanetFixed, TileId, Vec3};
use planet_generators::climate::{ClimateTexture, CLIMATE_BYTES};
use planet_generators::compact::{tile_data_field, vertex_tile_field, CompactTile, RenderTile, COMPACT_MIN_LEVEL};
use planet_generators::height::HeightField;
use planet_generators::mesh::{vertex_count, MeshVertex, TileMesh};
use planet_world_def::{body_radius_path, ReadRecord};

use crate::codec::{shuffle, unshuffle};
use crate::key::{cache_key, mesh_key_inputs, CacheKey, Kind};
use crate::pack::{PackError, PackWriter, Record, StoreParams, TileStore};

/// The kind byte of a pre-filled render tile.
pub const KIND_MESH: u8 = Kind::Mesh as u8;

/// Output parameter tag: the form of the stored tile (0 vertex, 1 compact).
pub const PARAM_RENDER_FORM: u8 = 5;

/// Version of the payload layout. History: 1 = LZ4 block of the exact tile; 2 = the vertex form carries the climate block.
pub const PAYLOAD_VERSION: u8 = 2;

/// The default depth of the pre-fill: levels 0 to this one (2,046 tiles).
pub const DEFAULT_PREFILL_LEVELS: u8 = 4;

/// The deepest level a pre-fill may name (4^8 x 6 tiles at level 8 alone are 393,216): a guard against a typo, and the deepest
/// level a reader looks up.
pub const MAX_PREFILL_LEVELS: u8 = 8;

/// Most cells per tile edge a payload may claim: a hostile header must not make a decoder allocate gigabytes.
const MAX_CELLS: u32 = 1024;

const HEADER_LEN: usize = 10;

/// The form a tile is stored and drawn in (the renderer's `TileFormat` decides which one a run asks for).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderForm {
    Vertex = 0,
    Compact = 1,
}

impl RenderForm {
    /// The form a tile of `level` has when the run asks for `wanted`: levels 0-2 are in the vertex form whatever is asked
    /// (`vertex_reason`: the biquadratic base surface errs too much there), so those records are shared by both forms.
    pub fn of_level(level: u8, wanted: RenderForm) -> RenderForm {
        if level < COMPACT_MIN_LEVEL {
            RenderForm::Vertex
        } else {
            wanted
        }
    }
}

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
            MeshCodecError::Form(v) => write!(f, "tile payload form {v} is neither vertex (0) nor compact (1)"),
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

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
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
    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.take())
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take())
    }
    fn u8(&mut self) -> u8 {
        self.take::<1>()[0]
    }
}

/// Raw bytes of the fixed part (key, origin) both forms start with.
const FIXED: usize = 8 + 3 * 8;
/// Raw bytes of a compact tile besides its texels: control points, four f32, the flip flag and the relief.
const COMPACT_FIXED: usize = FIXED + 27 * 4 + 4 * 4 + 1 + 8;

fn raw_len(form: RenderForm, cells: u32) -> u64 {
    match form {
        RenderForm::Vertex => FIXED as u64 + vertex_count(cells) as u64 * MeshVertex::STRIDE + CLIMATE_BYTES as u64,
        RenderForm::Compact => COMPACT_FIXED as u64 + (u64::from(cells) + 1) * (u64::from(cells) + 1) * 8,
    }
}

/// The form of a tile.
pub fn form_of(tile: &RenderTile) -> RenderForm {
    if tile.is_compact() {
        RenderForm::Compact
    } else {
        RenderForm::Vertex
    }
}

/// Encode a render tile as the payload of a kind-2 record.
pub fn encode_render_tile(tile: &RenderTile) -> Result<Vec<u8>, MeshCodecError> {
    let cells = tile.cells();
    if !valid_cells(cells) {
        return Err(MeshCodecError::Cells(cells));
    }
    let form = form_of(tile);
    let mut raw: Vec<u8> = Vec::with_capacity(raw_len(form, cells) as usize);
    raw.extend_from_slice(&tile.key().to_le_bytes());
    let o = tile.origin().0;
    for v in [o.x, o.y, o.z] {
        put_f64(&mut raw, v);
    }
    match tile {
        RenderTile::Vertex(m) => {
            if m.vertices.len() != vertex_count(cells) {
                return Err(MeshCodecError::Shape(format!(
                    "the mesh has {} vertices, {} are needed for {cells} cells",
                    m.vertices.len(),
                    vertex_count(cells)
                )));
            }
            let floats: Vec<f32> = m.vertices.iter().flat_map(|v| v.to_floats()).collect();
            raw.extend_from_slice(&shuffle(&floats));
            raw.extend_from_slice(&m.climate.to_bytes());
        }
        RenderTile::Compact(c) => {
            let texels = (cells as usize + 1) * (cells as usize + 1);
            if c.texels.len() != texels {
                return Err(MeshCodecError::Shape(format!(
                    "the compact tile has {} texels, {texels} are needed for {cells} cells",
                    c.texels.len()
                )));
            }
            for p in c.ctrl.iter().flatten() {
                put_f32(&mut raw, *p);
            }
            for v in [c.h_origin, c.hr_min, c.hr_step, c.skirt_depth] {
                put_f32(&mut raw, v);
            }
            raw.push(u8::from(c.flip));
            put_f64(&mut raw, c.relief_m);
            for t in &c.texels {
                for w in t {
                    raw.extend_from_slice(&w.to_le_bytes());
                }
            }
        }
    }
    debug_assert_eq!(raw.len() as u64, raw_len(form, cells));
    let mut out = Vec::with_capacity(HEADER_LEN + raw.len() / 2);
    out.push(PAYLOAD_VERSION);
    out.push(form as u8);
    out.extend_from_slice(&cells.to_le_bytes());
    out.extend_from_slice(&u32::try_from(raw.len()).expect("a tile payload is far below 4 GiB").to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress(&raw));
    Ok(out)
}

/// The form and cells a payload claims, if its header is well formed.
pub fn payload_form(bytes: &[u8]) -> Result<(RenderForm, u32), MeshCodecError> {
    if bytes.len() < HEADER_LEN {
        return Err(MeshCodecError::Short);
    }
    if bytes[0] != PAYLOAD_VERSION {
        return Err(MeshCodecError::Version(bytes[0]));
    }
    let form = match bytes[1] {
        0 => RenderForm::Vertex,
        1 => RenderForm::Compact,
        v => return Err(MeshCodecError::Form(v)),
    };
    let cells = u32::from_le_bytes(bytes[2..6].try_into().expect("4 bytes"));
    if !valid_cells(cells) {
        return Err(MeshCodecError::Cells(cells));
    }
    Ok((form, cells))
}

/// Decode the payload of the render tile `id`.
pub fn decode_render_tile(id: TileId, bytes: &[u8]) -> Result<RenderTile, MeshCodecError> {
    let (form, cells) = payload_form(bytes)?;
    let claimed = u32::from_le_bytes(bytes[6..10].try_into().expect("4 bytes"));
    let expected = raw_len(form, cells);
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
    Ok(match form {
        RenderForm::Vertex => {
            let n = vertex_count(cells);
            let split = raw.len() - CLIMATE_BYTES;
            let floats = unshuffle(&raw[FIXED..split]);
            let climate = ClimateTexture::from_bytes(raw[split..].try_into().expect("the climate block is the last 324 bytes"));
            debug_assert_eq!(floats.len(), n * MeshVertex::FLOATS);
            let vertices = floats
                .as_chunks::<{ MeshVertex::FLOATS }>()
                .0
                .iter()
                .map(|f| MeshVertex {
                    pos: [f[0], f[1], f[2]],
                    coarse: [f[3], f[4], f[5]],
                    normal: [f[6], f[7], f[8]],
                    height: f[9],
                    coarse_height: f[10],
                })
                .collect();
            RenderTile::Vertex(TileMesh { id, cells, key, origin, vertices, climate })
        }
        RenderForm::Compact => {
            let mut ctrl = [[0f32; 3]; 9];
            for p in ctrl.iter_mut().flatten() {
                *p = r.f32();
            }
            let (h_origin, hr_min, hr_step, skirt_depth) = (r.f32(), r.f32(), r.f32(), r.f32());
            let flip = r.u8() != 0;
            let relief_m = r.f64();
            let texels = (0..(cells as usize + 1) * (cells as usize + 1)).map(|_| [r.u16(), r.u16(), r.u16(), r.u16()]).collect();
            RenderTile::Compact(CompactTile {
                id,
                cells,
                key,
                origin,
                ctrl,
                h_origin,
                hr_min,
                hr_step,
                skirt_depth,
                flip,
                relief_m,
                texels,
            })
        }
    })
}

/// The persistent key of the pre-filled tile `tile` in `form`: the ADR 0018 mesh key with the form as an output parameter.
pub fn render_key(field: &dyn HeightField, map: &dyn FaceMapping, tile: TileId, cells: u32, radius_m: f64, form: RenderForm) -> CacheKey {
    let mut inputs = mesh_key_inputs(field, map, tile, cells, radius_m);
    inputs.output_params.insert(PARAM_RENDER_FORM, vec![form as u8]);
    cache_key(&inputs)
}

/// The id stored in every record of a field's tiles: BLAKE3-128 (context `planet-builder node set v1`) of the sorted,
/// deduplicated, length-prefixed paths of the field's reads and of the body's radius node (ADR 0025 point 3), as for height tiles.
pub fn node_set_id_of(field: &dyn HeightField) -> [u8; 16] {
    let mut paths: Vec<String> = field.read_records().iter().map(ReadRecord::path).collect();
    paths.push(body_radius_path(field.body()));
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
    /// The run's finest level (the LOD cap): a tile at or below it is a finest tile, whose format depends on its relief, so it is
    /// never pre-filled.
    max_level: u8,
    /// The form the run draws levels 3 and deeper in.
    form: RenderForm,
    node_set_id: std::sync::OnceLock<[u8; 16]>,
}

impl<'a> TileSpec<'a> {
    pub fn new(field: &'a dyn HeightField, radius_m: f64, cells: u32, max_level: u8, form: RenderForm) -> TileSpec<'a> {
        TileSpec { field, radius_m, cells, max_level, form, node_set_id: std::sync::OnceLock::new() }
    }

    pub fn max_level(&self) -> u8 {
        self.max_level
    }

    /// The tile as the streamer generates it on demand (`tile_data_field` for the compact form, the vertex tile for the other).
    pub fn generate(&self, tile: TileId) -> RenderTile {
        match self.form {
            RenderForm::Compact => tile_data_field(self.field, &TangentWarp, tile, self.cells, self.radius_m, self.max_level),
            RenderForm::Vertex => {
                RenderTile::Vertex(vertex_tile_field(self.field, &TangentWarp, tile, self.cells, self.radius_m, self.max_level))
            }
        }
    }

    /// Whether a tile may be read from the pack: shallow enough to be pre-filled, and not a finest tile of this run.
    pub fn storable(&self, tile: TileId) -> bool {
        tile.level() <= MAX_PREFILL_LEVELS && tile.level() < self.max_level
    }

    fn key_of(&self, tile: TileId) -> CacheKey {
        render_key(self.field, &TangentWarp, tile, self.cells, self.radius_m, RenderForm::of_level(tile.level(), self.form))
    }

    /// The record of a generated tile.
    pub fn record(&self, tile: &RenderTile) -> Result<Record, MeshCodecError> {
        Ok(Record {
            key: self.key_of(tile.id()),
            kind: KIND_MESH,
            tile: tile.id(),
            generator_version: self.field.version(),
            hash_version: HASH_VERSION,
            node_set_id: *self.node_set_id.get_or_init(|| node_set_id_of(self.field)),
            payload: encode_render_tile(tile)?,
        })
    }

    /// The pre-filled tile from the pack, if it holds one under the current key and the payload decodes (a cache degrades to
    /// a miss: damage, another version and a tile that is not storable are `None`, the caller generates).
    pub fn lookup(&self, store: &TileStore, tile: TileId) -> Option<RenderTile> {
        if !self.storable(tile) {
            return None;
        }
        let rec = store.get(KIND_MESH, tile, self.key_of(tile)).ok()??;
        let out = decode_render_tile(tile, &rec.payload).ok()?;
        (out.cells() == self.cells).then_some(out)
    }
}

/// The tiles of levels `0..=levels` in the order the pre-fill works and writes them: level, face, row, column.
pub fn prefill_tiles(levels: u8) -> Vec<TileId> {
    let mut out = Vec::new();
    for level in 0..=levels {
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
    /// Tiles in levels `0..=levels`.
    pub total: usize,
    /// Tiles the pack already held (under the current key, payload decoding): skipped.
    pub present: usize,
    /// Tiles generated and appended.
    pub written: usize,
    /// Payload bytes of the tiles appended.
    pub payload_bytes: u64,
}

/// Which of the tiles of levels `0..=levels` the pack holds.
pub fn missing_tiles(store: &TileStore, spec: &TileSpec<'_>, levels: u8) -> Vec<TileId> {
    prefill_tiles(levels).into_iter().filter(|t| spec.lookup(store, *t).is_none()).collect()
}

/// Write the tiles of levels `0..=levels` the pack of `dir` lacks. Tiles are generated on `threads` threads, heaviest (coarsest)
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
        return Err(PrefillError::Levels(format!("a pre-fill of levels 0..={levels} is too deep (at most {MAX_PREFILL_LEVELS})")));
    }
    if spec.form == RenderForm::Compact && spec.cells < planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS && levels >= COMPACT_MIN_LEVEL
    {
        return Err(PrefillError::Levels(format!(
            "the compact tile format needs at least {} cells per tile edge (the climate of a tile rides in the spare channel of its first 162 texels, GEN-021), got {}: use more cells or the vertex tile format",
            planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS,
            spec.cells
        )));
    }
    if levels >= spec.max_level {
        return Err(PrefillError::Levels(format!(
            "a pre-fill of levels 0..={levels} reaches the finest level {} of the view: such tiles are decided by their relief and are never stored",
            spec.max_level
        )));
    }
    let store = TileStore::open(dir, store_params())?;
    let all = prefill_tiles(levels);
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
