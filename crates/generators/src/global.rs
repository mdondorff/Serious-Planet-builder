//! The global-coarse stage: continents, elevation and climate of a whole body (GEN-013, GEN-007, CON-08, ADR 0022).
//!
//! One field with four channels, because under ADR 0022 option 1 a chain of steps is one field: the continent mask feeds
//! the elevation and the elevation feeds the temperature, all inside [`sample`]. It is a pure function of `(seed,
//! unit direction)`: integer hashing, IEEE `+ - * /`, `libm::floor`, `libm::asin`, `libm::exp`, so results
//! are bit-identical across platforms and thread counts (CON-14). The bake evaluates it at the vertices of a cube-face
//! grid (the same directions as the tile samples of the same level), so a shared border sample is one value, and the
//! result does not depend on the order or the number of threads that computed it.
//!
//! This is a new implementation ([`IMPLEMENTATION_ID`]) beside the reference height field: existing goldens, plan
//! snapshots and known answers do not move. Hydrology is not here (SPIKE-03).

use planet_core::cube::{face_to_direction, FaceMapping};
use planet_core::{Face, TileId, Vec3};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use planet_world_def::{BodyId, GenContext, MissingNode, ReadRecord};

use crate::height::layer_noise;

/// Bump whenever any output of this module changes (ADR 0006).
pub const GLOBAL_VERSION: u32 = 2;
/// Names this implementation in cache keys (CON-07).
pub const IMPLEMENTATION_ID: &str = "cpu-global-coarse-v1";

/// Domain tags so the lattices of different layers never alias for the same seed.
const LAYER_WARP: u64 = 0x474c_0001;
const LAYER_CONTINENT: u64 = 0x474c_0002;
const LAYER_RIDGE: u64 = 0x474c_0003;
const LAYER_RANGE_MASK: u64 = 0x474c_0004;
const LAYER_TEMPERATURE: u64 = 0x474c_0005;
const LAYER_RAIN: u64 = 0x474c_0006;

/// The continentalness value above which a point is land: tuned so about three tenths of a sphere is land.
pub const SEA_LEVEL_F32: f32 = 0.1;
/// [`SEA_LEVEL_F32`] as an `f64`: exactly representable in `f32`, so a reader that compares the stored `f32` continentalness
/// against it in `f32` and one that compares in `f64` agree (ADR 0026 point 3).
pub const SEA_LEVEL_C: f64 = SEA_LEVEL_F32 as f64;
/// Lowest and highest elevation in metres (clamps).
pub const MIN_ELEVATION_M: f64 = -9000.0;
pub const MAX_ELEVATION_M: f64 = 9000.0;

/// The global-coarse field of one body, built through a [`GenContext`] (CON-06, GEN-001): the seed is read from the
/// definition, so the read set (the world seed and the body's derived seed) is recorded and is what a key covers.
/// [`sample`] and [`bake`] are pure functions of that seed and read nothing else.
pub struct GlobalCoarseField {
    seed: u64,
    body: BodyId,
    inputs: u64,
    records: Vec<ReadRecord>,
}

impl GlobalCoarseField {
    /// The seed this field was built from.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn body(&self) -> BodyId {
        self.body
    }

    pub fn implementation_id(&self) -> &'static str {
        IMPLEMENTATION_ID
    }

    pub fn version(&self) -> u32 {
        GLOBAL_VERSION
    }

    /// Hash of the recorded read set (the 64-bit form of the key's definition part).
    pub fn inputs_hash(&self) -> u64 {
        self.inputs
    }

    /// The recorded reads, sorted by path: what a persistent key is built from (ADR 0018, STRM-011).
    pub fn read_records(&self) -> Vec<ReadRecord> {
        self.records.clone()
    }

    pub fn sample(&self, d: Vec3) -> GlobalSample {
        sample(self.seed, d)
    }

    /// Bake this field on the six cube-face grids (see [`bake`]).
    pub fn bake(&self, map: &(dyn FaceMapping + Sync), res: u32, threads: usize) -> Result<GlobalGrid, BakeError> {
        bake(self.seed, map, res, threads)
    }
}

/// Builds the [`GlobalCoarseField`] of a body from a definition.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlobalCoarseGenerator;

impl GlobalCoarseGenerator {
    pub fn implementation_id(&self) -> &'static str {
        IMPLEMENTATION_ID
    }

    pub fn version(&self) -> u32 {
        GLOBAL_VERSION
    }

    /// The field of `body`: reads the body's seed (and with it the world seed) through `ctx`. A missing node is an error
    /// naming it, not a default.
    pub fn field(&self, ctx: &GenContext, body: BodyId) -> Result<GlobalCoarseField, MissingNode> {
        assert!(ctx.reads().is_empty(), "a field is built from a fresh context: its key covers the whole read set of the context");
        let seed = ctx.body_seed(body)?;
        Ok(GlobalCoarseField { seed, body, inputs: ctx.read_set_hash(), records: ctx.read_records() })
    }
}

/// The four channels of the global-coarse field at one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalSample {
    /// Continentalness in [-1.7, 1.7]; above [`SEA_LEVEL_C`] is land.
    pub continent: f32,
    /// Elevation above the reference surface in metres (negative below sea level).
    pub elevation_m: f32,
    /// Mean annual temperature at that elevation in degrees Celsius.
    pub temperature_c: f32,
    /// Mean annual precipitation in millimetres.
    pub precipitation_mm: f32,
}

impl GlobalSample {
    pub fn is_land(&self) -> bool {
        f64::from(self.continent) > SEA_LEVEL_C
    }
}

fn fbm(seed: u64, layer: u64, p: Vec3, base_frequency: f64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, base_frequency);
    for o in 0..octaves {
        sum += amp * layer_noise(seed, layer, o, p * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn gauss(x: f64, mean: f64, width: f64) -> f64 {
    let z = (x - mean) / width;
    libm::exp(-z * z)
}

/// The global-coarse field at the unit direction `d`. Pure: the same `(seed, d)` gives the same bits everywhere.
pub fn sample(seed: u64, d: Vec3) -> GlobalSample {
    // Continents: low-frequency fBm of a domain-warped direction, so coastlines are not round blobs.
    let warp = Vec3::new(
        fbm(seed, LAYER_WARP, d + Vec3::new(0.0, 0.0, 0.0), 1.3, 3),
        fbm(seed, LAYER_WARP, d + Vec3::new(17.0, 0.0, 0.0), 1.3, 3),
        fbm(seed, LAYER_WARP, d + Vec3::new(0.0, 31.0, 0.0), 1.3, 3),
    );
    let warped = d + warp * 0.45;
    // Rounded to the stored f32 first, so land and the sign of the elevation are decided from the same number.
    let continent = f64::from((fbm(seed, LAYER_CONTINENT, warped, 1.1, 5) * 1.7) as f32);

    // Elevation: ocean basins and shelves from the continent value, mountain ranges from ridged noise inside range
    // zones, rising inland.
    let land_depth = continent - SEA_LEVEL_C;
    let base = if land_depth < 0.0 {
        // Sea level at the coast, down to -4500 m far from it (the field is continuous across the coast).
        -4500.0 * smoothstep(0.0, 0.9, -land_depth)
    } else {
        // Sea level at the coast, rising slowly to plateaus.
        1000.0 * smoothstep(0.0, 0.9, land_depth)
    };
    let ridge = {
        let n = fbm(seed, LAYER_RIDGE, warped, 3.0, 5);
        let r = 1.0 - libm::fabs(n) * 2.0;
        r.max(0.0) * r.max(0.0)
    };
    let zone = smoothstep(0.1, 0.6, fbm(seed, LAYER_RANGE_MASK, warped, 1.7, 3) + 0.2);
    let inland = smoothstep(0.02, 0.35, land_depth);
    let elevation = (base + 5200.0 * ridge * zone * inland).clamp(MIN_ELEVATION_M, MAX_ELEVATION_M);

    // Climate: temperature falls with latitude and, over land, with height; rain follows the latitude bands.
    let sin_lat = d.y.clamp(-1.0, 1.0);
    let lat_deg = libm::asin(sin_lat) * (180.0 / core::f64::consts::PI);
    let abs_lat = libm::fabs(lat_deg);
    let altitude_km = elevation.max(0.0) / 1000.0;
    let temperature = 30.0 - 55.0 * sin_lat * sin_lat - 6.5 * altitude_km + 4.0 * fbm(seed, LAYER_TEMPERATURE, d, 2.0, 3);
    let bands = 700.0 + 1900.0 * gauss(lat_deg, 0.0, 12.0) + 900.0 * gauss(abs_lat, 52.0, 14.0) - 650.0 * gauss(abs_lat, 27.0, 8.0);
    let wetness = 1.0 + 0.45 * fbm(seed, LAYER_RAIN, d, 2.5, 3) - if land_depth > 0.0 { 0.25 * inland } else { 0.0 };
    let precipitation = (bands * wetness).clamp(0.0, 4500.0);

    GlobalSample {
        continent: continent as f32, // exact: already rounded above
        elevation_m: elevation as f32,
        temperature_c: temperature as f32,
        precipitation_mm: precipitation as f32,
    }
}

/// The baked field of one body: `(res + 1)^2` samples per face (`res` cells per face edge), vertices included, so the
/// samples on a shared face edge appear on both faces with equal bits. Row-major per face, `k` (the s direction) fastest.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalGrid {
    pub seed: u64,
    pub res: u32,
    pub faces: [Vec<GlobalSample>; 6],
}

impl GlobalGrid {
    /// Samples per face edge.
    pub fn n(&self) -> u32 {
        self.res + 1
    }

    pub fn at(&self, face: Face, k: u32, l: u32) -> GlobalSample {
        self.faces[face.0 as usize][(l * self.n() + k) as usize]
    }

    /// Number of samples of all faces.
    pub fn len(&self) -> usize {
        self.faces.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Why a bake was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BakeError {
    /// `res` must be a power of two from 1 to [`MAX_BAKE_RES`].
    BadResolution(u32),
    /// A block of a tiled bake needs a power-of-two number of cells from 1 to [`MAX_BLOCK_CELLS`].
    BadBlock(u32),
    /// A tiled bake goes down to [`MAX_TILED_LEVEL`].
    BadLevel(u8),
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::BadResolution(r) => {
                write!(f, "a global bake needs a power-of-two resolution from 1 to {MAX_BAKE_RES} cells per face edge (a tiled bake up to {MAX_TILED_RES}), got {r}")
            }
            BakeError::BadBlock(c) => {
                write!(f, "a block of a tiled bake needs a power-of-two number of cells from 1 to {MAX_BLOCK_CELLS}, got {c}")
            }
            BakeError::BadLevel(l) => {
                write!(f, "a tiled bake goes down to level {MAX_TILED_LEVEL} (6 x 4^{MAX_TILED_LEVEL} blocks), got {l}")
            }
        }
    }
}

impl std::error::Error for BakeError {}

/// The largest in-memory bake: 2048 cells per face edge is about 5 km at Earth's radius (6 x 2049^2 samples of 16 bytes
/// is about 400 MB). A global grid of about 1 km needs 16384 cells per face edge (8192 is 1.2 km) and is baked as tiles,
/// never held whole ([`bake_blocks`]).
pub const MAX_BAKE_RES: u32 = 2048;

/// Sample position of vertex `(k, l)` of an `res`-cell face grid in face coordinates `[-1, 1]`.
fn face_coord(i: u32, res: u32) -> f64 {
    // Exact for powers of two: 2i/res - 1 has no rounding when res is a power of two.
    f64::from(2 * i) / f64::from(res) - 1.0
}

/// Bake the global-coarse field on the six faces with `threads` worker threads. Every sample is independent and each
/// thread writes its own rows, so the result does not depend on the thread count (CON-14).
pub fn bake(seed: u64, map: &(dyn FaceMapping + Sync), res: u32, threads: usize) -> Result<GlobalGrid, BakeError> {
    if res == 0 || !res.is_power_of_two() || res > MAX_BAKE_RES {
        return Err(BakeError::BadResolution(res));
    }
    let n = res + 1;
    let faces: [Vec<GlobalSample>; 6] = std::array::from_fn(|f| {
        let face = Face(f as u8);
        // Rows are the unit of work; chunks of consecutive rows go to threads and are joined in order.
        let rows: Vec<u32> = (0..n).collect();
        let threads = threads.max(1).min(rows.len());
        let chunk = rows.len().div_ceil(threads);
        let mut out = Vec::with_capacity((n * n) as usize);
        std::thread::scope(|scope| {
            let handles: Vec<_> = rows
                .chunks(chunk)
                .map(|rs| {
                    scope.spawn(move || {
                        let mut v = Vec::with_capacity(rs.len() * n as usize);
                        for &l in rs {
                            let t = face_coord(l, res);
                            for k in 0..n {
                                let s = face_coord(k, res);
                                v.push(sample(seed, face_to_direction(map, face, s, t)));
                            }
                        }
                        v
                    })
                })
                .collect();
            for h in handles {
                out.extend(h.join().expect("bake thread panicked"));
            }
        });
        out
    });
    Ok(GlobalGrid { seed, res, faces })
}

/// The largest block of a tiled bake: 1024 cells per tile edge is 1025^2 samples of 16 bytes, 16 MB.
pub const MAX_BLOCK_CELLS: u32 = 1024;
/// The finest global grid a tiled bake accepts: 65536 cells per face edge (about 150 m at Earth's radius).
pub const MAX_TILED_RES: u64 = 1 << 16;
/// The deepest level of a tiled bake: 6 * 4^12 = 100 million blocks at most.
pub const MAX_TILED_LEVEL: u8 = 12;

/// The samples of one block: tile `tile` on a grid of `cells` cells per edge, `(cells + 1)^2` row-major (`k`, the s
/// direction, fastest). They are the samples of the level-0 lattice of `cells << level` cells per face edge at the tile's
/// position, bit for bit: both come from `TileId::sample_direction`, whose lattice index is exact (GEN-013).
pub fn bake_block(seed: u64, map: &dyn FaceMapping, tile: TileId, cells: u32) -> Vec<GlobalSample> {
    let n = cells + 1;
    let mut v = Vec::with_capacity((n * n) as usize);
    for l in 0..n {
        for k in 0..n {
            v.push(sample(seed, tile.sample_direction(map, cells, k, l)));
        }
    }
    v
}

/// The most bytes of baked blocks a tiled bake keeps alive at once: 1 GiB, about 64 blocks of 1024 cells. The batch is
/// `min(threads, MAX_BATCH_BYTES / block bytes)` blocks, so a large thread count cannot exhaust memory (CON-21).
pub const MAX_BATCH_BYTES: u64 = 1 << 30;

/// How many blocks of `cells` cells a tiled bake bakes at once with `threads` threads: the thread count, at most
/// [`MAX_BATCH_BYTES`] of blocks, and at least one.
pub fn batch_blocks(cells: u32, threads: usize) -> usize {
    let block_bytes = 16 * (u64::from(cells) + 1) * (u64::from(cells) + 1);
    threads.max(1).min(usize::try_from((MAX_BATCH_BYTES / block_bytes).max(1)).unwrap_or(usize::MAX))
}

/// Check the shape of a tiled bake before anything is allocated, opened or written: `cells` a power of two up to
/// [`MAX_BLOCK_CELLS`], `level` at most [`MAX_TILED_LEVEL`] (checked before any shift), and at most [`MAX_TILED_RES`] cells per
/// face edge. Returns the cells per face edge.
pub fn check_tiled(level: u8, cells: u32) -> Result<u32, BakeError> {
    if cells == 0 || !cells.is_power_of_two() || cells > MAX_BLOCK_CELLS {
        return Err(BakeError::BadBlock(cells));
    }
    if level > MAX_TILED_LEVEL {
        return Err(BakeError::BadLevel(level));
    }
    let res = u64::from(cells) << level;
    if res > MAX_TILED_RES {
        return Err(BakeError::BadResolution(u32::try_from(res).unwrap_or(u32::MAX)));
    }
    Ok(res as u32)
}

/// What a tiled bake did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TiledBakeStats {
    pub blocks: usize,
    /// The most blocks alive at once between their creation and their delivery to the sink (counted, batches of at most `threads` blocks, delivered in order and
    /// dropped before the next batch): the memory clause of GEN-014 is this number times the block size.
    pub peak_resident_blocks: usize,
}

/// Bake the global grid of `cells << level` cells per face edge as tile-aligned blocks of `cells` cells (the tiles of
/// `level`), handing each to `sink` in order (face, then row, then column) and never holding more than `threads` of them
/// (ADR 0026 points 2 and 4). Every sample is the pointwise field at its lattice direction, so the result does not depend
/// on the thread count or the block size (CON-14).
pub fn bake_blocks(
    seed: u64,
    map: &(dyn FaceMapping + Sync),
    level: u8,
    cells: u32,
    threads: usize,
    sink: &mut dyn FnMut(TileId, Vec<GlobalSample>),
) -> Result<TiledBakeStats, BakeError> {
    bake_blocks_where(seed, map, level, cells, threads, &|_| true, sink)
}

/// [`bake_blocks`] for the blocks `want` accepts only: a rebuild that finds a block already stored does not compute it. The
/// accepted blocks come in the same order, in batches of at most `threads`.
pub fn bake_blocks_where(
    seed: u64,
    map: &(dyn FaceMapping + Sync),
    level: u8,
    cells: u32,
    threads: usize,
    want: &(dyn Fn(TileId) -> bool + Sync),
    sink: &mut dyn FnMut(TileId, Vec<GlobalSample>),
) -> Result<TiledBakeStats, BakeError> {
    check_tiled(level, cells)?;
    let side = 1u64 << level;
    let total = 6 * side * side;
    // A batch is bounded by a memory budget, not by the caller's thread count: at most MAX_BATCH_BYTES of blocks (and at
    // least one) are alive at once (CON-21).
    let threads = batch_blocks(cells, threads);
    // Blocks alive between their creation and their delivery to the sink: measured, not derived from the batch size.
    let live = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let mut stats = TiledBakeStats { blocks: 0, peak_resident_blocks: 0 };
    let mut next = 0u64;
    while next < total {
        // The tiles of this batch come from a running index (face, row, column): no list of all tiles is ever built.
        let mut batch: Vec<TileId> = Vec::with_capacity(threads.min(1024));
        let mut end = next;
        while end < total && batch.len() < threads {
            let (face, rest) = (end / (side * side), end % (side * side));
            let tile = TileId::new(Face(face as u8), level, (rest % side) as u32, (rest / side) as u32).expect("in range");
            if want(tile) {
                batch.push(tile);
            }
            end += 1;
        }
        if batch.is_empty() {
            next = end;
            continue;
        }
        let blocks: Vec<Vec<GlobalSample>> = std::thread::scope(|scope| {
            let (live, peak) = (&live, &peak);
            let handles: Vec<_> = batch
                .iter()
                .map(|t| {
                    scope.spawn(move || {
                        let block = bake_block(seed, map, *t, cells);
                        peak.fetch_max(live.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                        block
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("bake thread panicked")).collect()
        });
        for (t, b) in batch.iter().zip(blocks) {
            sink(*t, b);
            live.fetch_sub(1, Ordering::SeqCst);
            stats.blocks += 1;
        }
        next = end;
    }
    stats.peak_resident_blocks = peak.load(Ordering::SeqCst);
    Ok(stats)
}

/// Version of the reader (ADR 0027): the position arithmetic, the interpolation formula and the halo rule. A reader field
/// folds it into its own version; changing any of them bumps it.
pub const READER_VERSION: u32 = 1;

/// Why a read failed: the lattice sample could not be had from the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// A block or sample the source should hold is missing or unreadable; the message names it.
    Source(String),
    /// A tile sample grid the reader does not accept (`cells` not a power of two, or the level too deep).
    Shape(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Source(s) => write!(f, "the global bake cannot be read: {s}"),
            ReadError::Shape(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// Where a reader gets the baked lattice samples: vertex `(i, j)` of face `face` of a grid of [`LatticeSource::res`] cells per
/// face edge, `0 <= i, j <= res`. Implemented by an in-memory grid and by the tile store (`planet_cache::global_store`).
pub trait LatticeSource: Sync {
    fn res(&self) -> u32;
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError>;
    /// Cells per block edge of the source's storage (the unit it fetches and decodes at once), or 0 when it has no block
    /// structure (an in-memory grid). Only a hint for the order of batch reads (GEN-020); it never changes a value.
    fn block_cells(&self) -> u32 {
        0
    }
}

/// A [`GlobalGrid`] as a lattice source.
pub struct GridSource<'a>(pub &'a GlobalGrid);

impl LatticeSource for GridSource<'_> {
    fn res(&self) -> u32 {
        self.0.res
    }

    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        if i > self.0.res || j > self.0.res {
            return Err(ReadError::Source(format!("vertex ({i}, {j}) of a {}-cell grid", self.0.res)));
        }
        Ok(self.0.at(face, i, j))
    }
}

/// `a (1 - w) + b w` for exact dyadic `w`: the edge-symmetric form of ADR 0027 point 3 (the same two products added in
/// either order when a face edge is read from the other face, where the neighbours swap and `w` becomes `1 - w`).
fn blend(a: f64, b: f64, w: f64) -> f64 {
    a * (1.0 - w) + b * w
}

fn blend_samples(a: &GlobalSample, b: &GlobalSample, w: f64) -> [f64; 4] {
    [
        blend(f64::from(a.continent), f64::from(b.continent), w),
        blend(f64::from(a.elevation_m), f64::from(b.elevation_m), w),
        blend(f64::from(a.temperature_c), f64::from(b.temperature_c), w),
        blend(f64::from(a.precipitation_mm), f64::from(b.precipitation_mm), w),
    ]
}

/// Read the global field at integer position `(u, v)` of the lattice of `2^n_log2` cells per face edge (the sample index of
/// a tile: `x m + k`), by bilinear interpolation of the bake's own lattice (ADR 0027 points 2 to 5). The position is clamped
/// into `[0, 2^n_log2]` on both axes first (a halo beyond a face edge holds the edge value, point 4). Both lattices are
/// powers of two, so cell and weight come from the integer: with `s = n_log2 - log2 res`, `s >= 0` gives cell `u >> s` and weight
/// `(u mod 2^s) / 2^s`, and `s < 0` a lattice point (weight 0). Channels are blended in `f64` from the stored `f32` and
/// rounded to `f32` once.
pub fn read_lattice(src: &dyn LatticeSource, face: Face, u: i64, v: i64, n_log2: u32) -> Result<GlobalSample, ReadError> {
    let res = src.res();
    if res == 0 || !res.is_power_of_two() || n_log2 > 52 {
        return Err(ReadError::Shape(format!(
            "a reader needs a power-of-two bake resolution and at most 2^52 cells per face edge (res {res}, 2^{n_log2})"
        )));
    }
    let res_log2 = res.trailing_zeros();
    let n = 1i64 << n_log2;
    // A halo is at most one tile edge (ADR 0022), so at most one face edge of the lattice; a position farther out is a
    // caller that used the wrong lattice (a level or cell count off by one), not a halo.
    if !(-n..=2 * n).contains(&u) || !(-n..=2 * n).contains(&v) {
        return Err(ReadError::Shape(format!(
            "position ({u}, {v}) is more than one lattice edge outside a face lattice of 2^{n_log2} cells"
        )));
    }
    let (u, v) = (u.clamp(0, n), v.clamp(0, n));
    // Cell index and weight of one axis.
    let axis = |p: i64| -> (u32, f64) {
        if n_log2 >= res_log2 {
            let s = n_log2 - res_log2;
            let cell = (p >> s) as u32;
            let frac = (p & ((1i64 << s) - 1)) as f64 / (1u64 << s) as f64;
            (cell.min(res), if cell >= res { 0.0 } else { frac })
        } else {
            ((p << (res_log2 - n_log2)) as u32, 0.0)
        }
    };
    let ((i, wu), (j, wv)) = (axis(u), axis(v));
    blend_cell(src, face, i, j, wu, wv)
}

/// The bilinear blend of cell `(i, j)` of `face` with weights `(wu, wv)` (ADR 0027 point 3): the shared core of every read.
pub(crate) fn blend_cell(src: &dyn LatticeSource, face: Face, i: u32, j: u32, wu: f64, wv: f64) -> Result<GlobalSample, ReadError> {
    let res = src.res();
    // The far neighbour is read only when its weight is not zero: a lattice point is its own sample (a stored -0.0 stays -0.0,
    // an infinite or missing neighbour cannot change it) and reads touch no block they do not need.
    let i1 = if wu == 0.0 { i } else { (i + 1).min(res) };
    let j1 = if wv == 0.0 { j } else { (j + 1).min(res) };
    let a = src.at(face, i, j)?;
    let b = if i1 == i { a } else { src.at(face, i1, j)? };
    let c = if j1 == j { a } else { src.at(face, i, j1)? };
    let d = match (i1 == i, j1 == j) {
        (true, true) => a,
        (false, true) => b,
        (true, false) => c,
        (false, false) => src.at(face, i1, j1)?,
    };
    let (r0, r1) = (blend_samples(&a, &b, wu), blend_samples(&c, &d, wu));
    let ch = |k: usize| blend(r0[k], r1[k], wv) as f32;
    Ok(GlobalSample { continent: ch(0), elevation_m: ch(1), temperature_c: ch(2), precipitation_mm: ch(3) })
}

/// Read the global field at sample `(k, l)` of tile `tile` on a grid of `cells` cells per tile edge (`-cells..=2 cells`, a halo of
/// at most one tile edge, ADR 0022), by [`read_lattice`]: the tile's lattice index is `x cells + k` on a face lattice of `cells << level` cells.
pub fn read_tile_sample(src: &dyn LatticeSource, tile: TileId, cells: u32, k: i64, l: i64) -> Result<GlobalSample, ReadError> {
    if cells == 0 || !cells.is_power_of_two() {
        return Err(ReadError::Shape(format!("a tile grid needs a power-of-two number of cells, got {cells}")));
    }
    // The halo of a regional field is a fraction of the tile edge, at most one edge (ADR 0022): `-cells..=2 cells`.
    let m = i64::from(cells);
    if !(-m..=2 * m).contains(&k) || !(-m..=2 * m).contains(&l) {
        return Err(ReadError::Shape(format!("sample ({k}, {l}) is more than one tile edge outside a {cells}-cell tile")));
    }
    let n_log2 = u32::from(tile.level()) + cells.trailing_zeros();
    let (x, y) = (i64::from(tile.x()) * i64::from(cells), i64::from(tile.y()) * i64::from(cells));
    read_lattice(src, tile.face(), x + k, y + l, n_log2)
}

/// Read the global field at the unit direction `dir`: the face and face coordinates come from `direction_to_face`, the
/// position in the bake's lattice from the face coordinates in floating point, and the blend is [`read_lattice`]'s. A pure
/// function of `dir` (the same direction bits, the same value), which is what a direction-based height field needs: shared
/// border samples of tiles, also across faces, have identical direction bits (COORD-007) and so identical heights. Unlike
/// [`read_tile_sample`] its weights are not exact dyadic fractions and it is not bit-equal to a lattice read of the same
/// sample; a field uses one or the other, never both (ADR 0028).
pub fn read_direction(src: &dyn LatticeSource, map: &dyn FaceMapping, dir: Vec3) -> Result<GlobalSample, ReadError> {
    let res = src.res();
    if res == 0 || !res.is_power_of_two() {
        return Err(ReadError::Shape(format!("a reader needs a power-of-two bake resolution (res {res})")));
    }
    let (face, s, t) = planet_core::cube::direction_to_face(map, dir);
    // Face coordinates run over [-1, 1]; the bake's lattice has `res` cells over that range.
    let pos = |c: f64| ((c + 1.0) * 0.5 * f64::from(res)).clamp(0.0, f64::from(res));
    let axis = |p: f64| -> (u32, f64) {
        let cell = libm::floor(p) as u32;
        if cell >= res {
            (res, 0.0)
        } else {
            (cell, p - f64::from(cell))
        }
    };
    let ((i, wu), (j, wv)) = (axis(pos(s)), axis(pos(t)));
    blend_cell(src, face, i, j, wu, wv)
}

/// Interleave the bits of two block indices (Z order): blocks that are neighbours on the face are close in this order, so a
/// small cache holds the neighbours a border sample reads while a sweep moves on.
fn morton(x: u32, y: u32) -> u64 {
    let spread = |v: u32| {
        let mut v = u64::from(v);
        v = (v | v << 16) & 0x0000_ffff_0000_ffff;
        v = (v | v << 8) & 0x00ff_00ff_00ff_00ff;
        v = (v | v << 4) & 0x0f0f_0f0f_0f0f_0f0f;
        v = (v | v << 2) & 0x3333_3333_3333_3333;
        (v | v << 1) & 0x5555_5555_5555_5555
    };
    spread(x) | spread(y) << 1
}

fn cell_key(face: u8, i: u32, j: u32) -> u64 {
    u64::from(face) << 40 | u64::from(i) << 20 | u64::from(j)
}

/// Records the cells a footprint pass asks for and answers a placeholder (a footprint depends on the direction, never on a value).
struct Gather {
    res: u32,
    seen: Mutex<Vec<u64>>,
}

impl LatticeSource for Gather {
    fn res(&self) -> u32 {
        self.res
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        self.seen.lock().expect("gather mutex").push(cell_key(face.0, i, j));
        Ok(GlobalSample { continent: 0.0, elevation_m: 0.0, temperature_c: 0.0, precipitation_mm: 0.0 })
    }
}

/// The samples (or the errors) of the cells a batch needs, fetched once from the real source in block order.
struct CellTable {
    res: u32,
    cells: Vec<(u64, Result<GlobalSample, ReadError>)>,
}

impl LatticeSource for CellTable {
    fn res(&self) -> u32 {
        self.res
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let key = cell_key(face.0, i, j);
        match self.cells.binary_search_by_key(&key, |c| c.0) {
            Ok(at) => self.cells[at].1.clone(),
            Err(_) => Err(ReadError::Source(format!("cell ({i}, {j}) of face {} was not in the batch's footprint", face.0))),
        }
    }
}

/// Evaluate `eval` on every direction of `dirs` and return the results in the caller's order (GEN-020). `eval(src, d)` is a
/// pure function of `d` that reads the lattice only through `src`; `footprint(src, d)` makes the same lattice reads as
/// `eval(src, d)` (it may skip the arithmetic that follows them).
///
/// When `src` has blocks (`block_cells() > 0`) the batch runs in two passes: the footprint of every direction is collected,
/// the distinct cells are read from `src` grouped by block (each block is touched by one run of reads, so it is decoded once
/// however small the source's cache), and `eval` then reads those cells from memory, in the caller's order. A cell is the
/// source's own sample, so every result is bit for bit what `eval(src, d)` returns. Without blocks (an in-memory grid) it is
/// the plain loop. `stop` is polled before each block, every 64 cells and every 64 samples; after it answers true nothing more
/// is read and the unfinished results are 0.0.
pub fn eval_gathered(
    src: &dyn LatticeSource,
    dirs: &[Vec3],
    stop: &dyn Fn() -> bool,
    footprint: &dyn Fn(&dyn LatticeSource, Vec3),
    eval: &dyn Fn(&dyn LatticeSource, Vec3) -> f64,
) -> Vec<f64> {
    let mut out = vec![0.0; dirs.len()];
    let (res, bc) = (src.res(), src.block_cells());
    if bc == 0 || res == 0 || bc > res || !res.is_power_of_two() || res >= 1 << 20 || dirs.len() < 64 {
        // (`cell_key` packs a cell index into 20 bits; a small batch is not worth a gather.)
        for (k, &d) in dirs.iter().enumerate() {
            if k % 64 == 0 && stop() {
                break;
            }
            out[k] = eval(src, d);
        }
        return out;
    }
    let gather = Gather { res, seen: Mutex::new(Vec::new()) };
    for (k, &d) in dirs.iter().enumerate() {
        if k % 64 == 0 && stop() {
            return out;
        }
        footprint(&gather, d);
    }
    let mut keys = gather.seen.into_inner().expect("gather mutex");
    keys.sort_unstable();
    keys.dedup();
    // The block that holds a cell: the last block of a row holds the far edge as its own last sample.
    let last = res / bc - 1;
    let mut by_block: Vec<(u8, u64, u64)> = keys
        .iter()
        .map(|&k| {
            let (face, i, j) = ((k >> 40) as u8, ((k >> 20) & 0xf_ffff) as u32, (k & 0xf_ffff) as u32);
            (face, morton((i / bc).min(last), (j / bc).min(last)), k)
        })
        .collect();
    by_block.sort_unstable();
    let mut cells = Vec::with_capacity(by_block.len());
    for (n, &(face, block, k)) in by_block.iter().enumerate() {
        if (n == 0 || (by_block[n - 1].0, by_block[n - 1].1) != (face, block) || n % 64 == 0) && stop() {
            return out;
        }
        cells.push((k, src.at(Face(face), ((k >> 20) & 0xf_ffff) as u32, (k & 0xf_ffff) as u32)));
    }
    cells.sort_unstable_by_key(|c| c.0);
    let table = CellTable { res, cells };
    for (k, &d) in dirs.iter().enumerate() {
        if k % 64 == 0 && stop() {
            break;
        }
        out[k] = eval(&table, d);
    }
    out
}

/// A 64-bit hash of every bit of every channel of a grid, in face and row-major order, with the resolution and
/// [`GLOBAL_VERSION`]: the quick equality check of tests and the pinned known answer of a small bake.
pub fn grid_hash(g: &GlobalGrid) -> u64 {
    let mut words = Vec::with_capacity(g.len() * 2 + 2);
    words.push(u64::from(g.res));
    words.push(u64::from(GLOBAL_VERSION));
    for face in &g.faces {
        for s in face {
            words.push(u64::from(s.continent.to_bits()) << 32 | u64::from(s.elevation_m.to_bits()));
            words.push(u64::from(s.temperature_c.to_bits()) << 32 | u64::from(s.precipitation_mm.to_bits()));
        }
    }
    planet_core::hash::hash_u64s(g.seed, &words)
}

/// A channel of the global field as an image: the data products of the bake (report §16), one cube net per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// Land in green, sea in blue, by continentalness.
    Continent,
    /// A hypsometric tint: deep blue to light blue below sea level, green, brown, white above.
    Elevation,
    /// Blue (cold) to red (hot), -40 to +35 degrees.
    Temperature,
    /// Tan (dry) to blue (wet), 0 to 3000 mm.
    Precipitation,
}

impl Channel {
    pub const ALL: [Channel; 4] = [Channel::Continent, Channel::Elevation, Channel::Temperature, Channel::Precipitation];

    pub fn name(self) -> &'static str {
        match self {
            Channel::Continent => "continent",
            Channel::Elevation => "elevation",
            Channel::Temperature => "temperature",
            Channel::Precipitation => "precipitation",
        }
    }
}

fn mix(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn rgba(c: [f64; 3]) -> [u8; 4] {
    let q = |v: f64| libm::floor(v.clamp(0.0, 255.0) + 0.5) as u8;
    [q(c[0]), q(c[1]), q(c[2]), 255]
}

/// The colour of one sample in a channel's image.
pub fn colour(s: &GlobalSample, channel: Channel) -> [u8; 4] {
    match channel {
        Channel::Continent => {
            if s.is_land() {
                rgba(mix([90.0, 150.0, 70.0], [230.0, 230.0, 200.0], (f64::from(s.continent) - SEA_LEVEL_C) / 0.8))
            } else {
                rgba(mix([10.0, 40.0, 110.0], [70.0, 130.0, 200.0], (f64::from(s.continent) - SEA_LEVEL_C + 0.8) / 0.8))
            }
        }
        Channel::Elevation => {
            let e = f64::from(s.elevation_m);
            if e < 0.0 {
                rgba(mix([10.0, 30.0, 100.0], [110.0, 170.0, 220.0], (e + 5000.0) / 5000.0))
            } else if e < 1500.0 {
                rgba(mix([70.0, 140.0, 60.0], [170.0, 150.0, 90.0], e / 1500.0))
            } else {
                rgba(mix([170.0, 150.0, 90.0], [250.0, 250.0, 250.0], (e - 1500.0) / 4000.0))
            }
        }
        Channel::Temperature => rgba(mix([40.0, 70.0, 220.0], [230.0, 60.0, 40.0], (f64::from(s.temperature_c) + 40.0) / 75.0)),
        Channel::Precipitation => rgba(mix([200.0, 180.0, 120.0], [30.0, 80.0, 200.0], f64::from(s.precipitation_mm) / 3000.0)),
    }
}

/// The unfolded six-face cube net of one channel (4 by 3 faces of `res` pixels, the vertex at each cell's lower corner),
/// in the layout of [`crate::dump::face_net`]; pixels outside the six faces are `NET_BACKGROUND`.
pub fn product(grid: &GlobalGrid, channel: Channel) -> crate::dump::Rgba {
    use crate::dump::{NET_BACKGROUND, NET_CELLS};
    let cell = grid.res;
    let (w, h) = (cell * 4, cell * 3);
    let mut data = NET_BACKGROUND.repeat((w * h) as usize);
    for (f, &(cx, cy)) in NET_CELLS.iter().enumerate() {
        for py in 0..cell {
            for px in 0..cell {
                // Image rows run downwards, the t direction upwards.
                let c = colour(&grid.at(Face(f as u8), px, cell - 1 - py), channel);
                let i = (((cy * cell + py) * w + cx * cell + px) * 4) as usize;
                data[i..i + 4].copy_from_slice(&c);
            }
        }
    }
    crate::dump::Rgba { width: w, height: h, data }
}

/// An equirectangular map (longitude along x, latitude up) of one channel straight from [`sample`], `width` by `width / 2`
/// pixels: the picture people expect of a planet, and independent of the face grid.
pub fn equirect_product(seed: u64, width: u32, channel: Channel) -> crate::dump::Rgba {
    let height = (width / 2).max(1);
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for py in 0..height {
        let lat = (0.5 - (f64::from(py) + 0.5) / f64::from(height)) * core::f64::consts::PI;
        let (sl, cl) = (libm::sin(lat), libm::cos(lat));
        for px in 0..width {
            let lon = ((f64::from(px) + 0.5) / f64::from(width) - 0.5) * 2.0 * core::f64::consts::PI;
            let d = Vec3::new(cl * libm::sin(lon), sl, cl * libm::cos(lon));
            data.extend(colour(&sample(seed, d), channel));
        }
    }
    crate::dump::Rgba { width, height, data }
}
