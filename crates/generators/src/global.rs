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
use std::sync::Arc;
use std::sync::Mutex;

use planet_world_def::{BodyId, GenContext, MissingNode, ReadRecord};

use crate::height::layer_noise;

/// Bump whenever any output of this module changes (ADR 0006). History: 2 = the climate's pole axis moved to COORD's +z (3);
/// 4 = the global stage gains the ocean-connectivity channel (`crate::ocean`, ADR 0041 point 2); the four sample channels are
/// unchanged, but every key under this version is new, so stored blocks of version 3 are unreachable; 5 = the hill band joins the elevation
/// (GEN-064, ADR 0080).
pub const GLOBAL_VERSION: u32 = 5;
/// Names this implementation in cache keys (CON-07).
pub const IMPLEMENTATION_ID: &str = "cpu-global-coarse-v1";

/// Domain tags so the lattices of different layers never alias for the same seed.
const LAYER_WARP: u64 = 0x474c_0001;
const LAYER_CONTINENT: u64 = 0x474c_0002;
const LAYER_RIDGE: u64 = 0x474c_0003;
const LAYER_RANGE_MASK: u64 = 0x474c_0004;
const LAYER_TEMPERATURE: u64 = 0x474c_0005;
const LAYER_RAIN: u64 = 0x474c_0006;
const LAYER_HILL: u64 = 0x474c_0007;

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

use planet_core::smoothstep;

fn gauss(x: f64, mean: f64, width: f64) -> f64 {
    let z = (x - mean) / width;
    libm::exp(-z * z)
}

/// Sine of the latitude of the unit direction `d` in COORD's axes: +z is the north pole, so it is `d.z` (the equator is the
/// plane through +x, longitude 0, and +y, longitude 90 degrees east). GLOBAL_VERSION 2 took it from `d.y`, which put the poles at
/// +-y and disagreed with every other consumer of the axes (the sun, the camera); version 3 is this one.
pub fn sin_latitude(d: Vec3) -> f64 {
    d.z.clamp(-1.0, 1.0)
}

/// The terms of the elevation that the landform driver and the hill band share: the warped direction, the continent value and the range zone.
struct Terrain {
    warped: Vec3,
    continent: f64,
    land_depth: f64,
    zone: f64,
    inland: f64,
}

fn terrain(seed: u64, d: Vec3) -> Terrain {
    // Continents: low-frequency fBm of a domain-warped direction, so coastlines are not round blobs.
    let warp = Vec3::new(
        fbm(seed, LAYER_WARP, d + Vec3::new(0.0, 0.0, 0.0), 1.3, 3),
        fbm(seed, LAYER_WARP, d + Vec3::new(17.0, 0.0, 0.0), 1.3, 3),
        fbm(seed, LAYER_WARP, d + Vec3::new(0.0, 31.0, 0.0), 1.3, 3),
    );
    let warped = d + warp * 0.45;
    // Rounded to the stored f32 first, so land and the sign of the elevation are decided from the same number.
    let continent = f64::from((fbm(seed, LAYER_CONTINENT, warped, 1.1, 5) * 1.7) as f32);
    let land_depth = continent - SEA_LEVEL_C;
    let zone = smoothstep(0.1, 0.6, fbm(seed, LAYER_RANGE_MASK, warped, 1.7, 3) + 0.2);
    let inland = smoothstep(0.02, 0.35, land_depth);
    Terrain { warped, continent, land_depth, zone, inland }
}

/// The ridge term of the mountain ranges at the warped direction, in `[0, 1]`.
fn ridge_at(seed: u64, warped: Vec3) -> f64 {
    let n = fbm(seed, LAYER_RIDGE, warped, 3.0, 5);
    let r = 1.0 - libm::fabs(n) * 2.0;
    r.max(0.0) * r.max(0.0)
}

/// The landform driver in `[0, 1]` at the unit direction `d` (GEN-064, ADR 0080 point 3): the range-zone strength times the distance from the
/// coast, 0 over the sea, on the plains and at the coast, 1 deep inland in the middle of a range zone. A pure function of `(seed, d)` that does
/// not read the hill band, so the band's amplitude does not feed back into the driver that sets it, and the same bits at any bake resolution.
pub fn landform(seed: u64, d: Vec3) -> f64 {
    let t = terrain(seed, d);
    t.zone * t.inland
}

/// Cells per face edge of the [`LandformLattice`]: 256 cells of 2/256 in the face coordinate, about 39 km at the face centre.
const LANDFORM_LATTICE_N: usize = 256;

/// The landform driver on a fixed lattice over the cube faces, read by bilinear interpolation (GEN-062): the field of the erosion creases reads the landform
/// at every height, and the 17 noise evaluations of [`landform`] cost 58 % of the height; the lattice costs a few multiplications. The lattice is a pure
/// function of the seed: `N + 1` points per face edge at the face coordinates `2 i / N - 1` of the tangent warp, the value there [`landform`] rounded to
/// f32, so a tile of any size and any bake reads the same bits. The interpolant is continuous across face edges (the edge points of two faces are the same
/// directions) and, being a convex combination of the samples, stays in `[0, 1]`.
pub struct LandformLattice {
    faces: Vec<Vec<f32>>,
}

impl LandformLattice {
    fn build(seed: u64) -> LandformLattice {
        let n = LANDFORM_LATTICE_N;
        let map = planet_core::cube::TangentWarp;
        // One thread per face: every value is independent of the others, so the lattice does not depend on the thread count (CON-14).
        let faces = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..6u8)
                .map(|f| {
                    scope.spawn(move || {
                        let mut values = Vec::with_capacity((n + 1) * (n + 1));
                        for j in 0..=n {
                            for i in 0..=n {
                                let (s, t) = (2.0 * i as f64 / n as f64 - 1.0, 2.0 * j as f64 / n as f64 - 1.0);
                                values.push(landform(seed, face_to_direction(&map, Face(f), s, t)) as f32);
                            }
                        }
                        values
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("a landform lattice thread")).collect()
        });
        LandformLattice { faces }
    }

    fn sample(&self, face: Face, i: usize, j: usize) -> f64 {
        f64::from(self.faces[face.0 as usize][j * (LANDFORM_LATTICE_N + 1) + i])
    }

    /// The lattice position of a face coordinate in `[-1, 1]`: the cell index and the fraction within it.
    fn locate(s: f64) -> (usize, f64) {
        let x = ((s + 1.0) * 0.5 * LANDFORM_LATTICE_N as f64).clamp(0.0, LANDFORM_LATTICE_N as f64);
        let i = (x as usize).min(LANDFORM_LATTICE_N - 1);
        (i, x - i as f64)
    }

    /// The landform at the unit direction `d`: the bilinear interpolant of the samples, in `[0, 1]`.
    pub fn at(&self, d: Vec3) -> f64 {
        let (face, s, t) = planet_core::cube::direction_to_face(&planet_core::cube::TangentWarp, d);
        let ((i, fx), (j, fy)) = (Self::locate(s), Self::locate(t));
        let (v00, v10) = (self.sample(face, i, j), self.sample(face, i + 1, j));
        let (v01, v11) = (self.sample(face, i, j + 1), self.sample(face, i + 1, j + 1));
        let (a, b) = (v00 + (v10 - v00) * fx, v01 + (v11 - v01) * fx);
        (a + (b - a) * fy).clamp(0.0, 1.0)
    }

    /// The range of [`LandformLattice::at`] over the cap of angular radius `angular_radius` (radians) round the unit direction `centre` (GEN-062): the
    /// least and the greatest sample of the cells the cap touches, which the interpolant of a cell cannot leave. A cap that may leave its face (it reaches
    /// the edge, or is over 0.05 rad) or touches over 16 cells an axis gets `[0, 1]`. On a face a cap point is within `4 / (pi / 4)` face units of the centre
    /// per radian (the plane coordinate `a / w` has a gradient of at most `sqrt(1 + u^2) / w` per radian with `w >= cos(0.955 + 0.05)` and `|u| <= 1 / w`,
    /// and the tangent warp stretches by at least `pi / 4`).
    pub fn bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        if angular_radius.is_nan() || angular_radius > 0.05 {
            return (0.0, 1.0);
        }
        let (face, s, t) = planet_core::cube::direction_to_face(&planet_core::cube::TangentWarp, centre);
        let reach = 5.2 * angular_radius + 1e-9;
        if s - reach <= -1.0 || s + reach >= 1.0 || t - reach <= -1.0 || t + reach >= 1.0 {
            return (0.0, 1.0);
        }
        let (i0, _) = Self::locate(s - reach);
        let (i1, _) = Self::locate(s + reach);
        let (j0, _) = Self::locate(t - reach);
        let (j1, _) = Self::locate(t + reach);
        let (i1, j1) = (i1 + 1, j1 + 1);
        if i1 - i0 > 16 || j1 - j0 > 16 {
            return (0.0, 1.0);
        }
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let v = self.sample(face, i, j);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        ((lo - 1e-12).max(0.0), (hi + 1e-12).min(1.0))
    }
}

/// The landform lattice of `seed`, built once and shared (the few seeds of a run: the cache holds the last four).
pub fn landform_lattice(seed: u64) -> Arc<LandformLattice> {
    static CACHE: Mutex<Vec<(u64, Arc<LandformLattice>)>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().expect("landform lattice cache");
    if let Some((_, lattice)) = cache.iter().find(|(k, _)| *k == seed) {
        return Arc::clone(lattice);
    }
    let lattice = Arc::new(LandformLattice::build(seed));
    if cache.len() >= 4 {
        cache.remove(0);
    }
    cache.push((seed, Arc::clone(&lattice)));
    lattice
}

/// The largest amplitude of the hill band in metres (GEN-064); the elevation clamp has room for it above the highest base and range
/// (`1000 + 5200` m), so the clamp is not raised.
pub const HILL_BAND_MAX_M: f64 = 1000.0;

/// Frequencies of the three octaves of the hill band in units of the unit direction: wavelengths of 8, 4 and 2 km on a body of Earth's radius
/// (they scale with the radius of the body).
const HILL_BAND_FREQ: f64 = 796.0;
const HILL_BAND_OCTAVES: u32 = 3;

/// The amplitude of the hill band in metres for the mountain strength `m` in `[0, 1]`: plains 30 m, hills up to 175 m by `m` 0.25, then up to
/// the mountains' 1000 m.
fn hill_amplitude_m(m: f64) -> f64 {
    30.0 + 145.0 * smoothstep(0.0, 0.25, m) + (HILL_BAND_MAX_M - 175.0) * smoothstep(0.25, 0.7, m)
}

/// The hill band at a point of land: a ridged multifractal of three octaves (wavelengths 8, 4 and 2 km) on the warped direction, in `[0, 1]`,
/// times the amplitude of the landform and a ramp from the coast (so the band is 0 at the coast and over the sea, and only lifts).
fn hill_band(seed: u64, t: &Terrain, ridge: f64) -> f64 {
    if t.land_depth <= 0.0 {
        return 0.0;
    }
    let m = t.zone * t.inland * (0.25 + 0.75 * ridge);
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, HILL_BAND_FREQ);
    for o in 0..HILL_BAND_OCTAVES {
        let n = layer_noise(seed, LAYER_HILL, o, t.warped * freq);
        let r = 1.0 - libm::fabs(n);
        sum += amp * r * r;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    hill_amplitude_m(m) * (sum / norm) * smoothstep(0.0, 0.1, t.land_depth)
}

/// The hill band in metres at the unit direction `d` (GEN-064): what the bake adds to the elevation for wavelengths of 2 to 8 km.
pub fn hill_band_m(seed: u64, d: Vec3) -> f64 {
    let t = terrain(seed, d);
    hill_band(seed, &t, ridge_at(seed, t.warped))
}

/// The global-coarse field at the unit direction `d`. Pure: the same `(seed, d)` gives the same bits everywhere.
pub fn sample(seed: u64, d: Vec3) -> GlobalSample {
    let t = terrain(seed, d);
    let (continent, inland, zone) = (t.continent, t.inland, t.zone);

    // Elevation: ocean basins and shelves from the continent value, mountain ranges from ridged noise inside range
    // zones, rising inland.
    let land_depth = t.land_depth;
    let base = if land_depth < 0.0 {
        // Sea level at the coast, down to -4500 m far from it (the field is continuous across the coast).
        -4500.0 * smoothstep(0.0, 0.9, -land_depth)
    } else {
        // Sea level at the coast, rising slowly to plateaus.
        1000.0 * smoothstep(0.0, 0.9, land_depth)
    };
    let ridge = ridge_at(seed, t.warped);
    let elevation = (base + 5200.0 * ridge * zone * inland + hill_band(seed, &t, ridge)).clamp(MIN_ELEVATION_M, MAX_ELEVATION_M);

    // Climate (latitude from COORD's +z pole, see [`sin_latitude`]): temperature falls with latitude and, over land, with height; rain follows the latitude bands.
    let sin_lat = sin_latitude(d);
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
    bake_blocks_with(seed, map, level, cells, threads, want, &|_, _| (), &mut |t, b, ()| sink(t, b))
}

/// [`bake_blocks_where`] with `analyse`, a pure function of a block that runs on the worker thread that baked it while the block
/// is in memory (the ocean-connectivity pass of ADR 0041 point 2 labels its components here); the sink receives the block and what
/// `analyse` returned for it. `analyse` runs once per accepted block and its result is part of what a batch holds, so a large
/// result counts against the caller's memory, not the block budget.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn bake_blocks_with<A: Send>(
    seed: u64,
    map: &(dyn FaceMapping + Sync),
    level: u8,
    cells: u32,
    threads: usize,
    want: &(dyn Fn(TileId) -> bool + Sync),
    analyse: &(dyn Fn(TileId, &[GlobalSample]) -> A + Sync),
    sink: &mut dyn FnMut(TileId, Vec<GlobalSample>, A),
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
        let blocks: Vec<(Vec<GlobalSample>, A)> = std::thread::scope(|scope| {
            let (live, peak) = (&live, &peak);
            let handles: Vec<_> = batch
                .iter()
                .map(|t| {
                    scope.spawn(move || {
                        let block = bake_block(seed, map, *t, cells);
                        peak.fetch_max(live.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                        let a = analyse(*t, &block);
                        (block, a)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("bake thread panicked")).collect()
        });
        for (t, (b, a)) in batch.iter().zip(blocks) {
            sink(*t, b, a);
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
    /// The factor the source's elevations carry over the bake's own (WDEF-017): 1 for a bake, the body's relief scale for a [`ScaledLattice`].
    /// A stored result made from the source (the coupled run) names it in its key, so a scaled source never reads an unscaled run.
    fn relief_scale(&self) -> f64 {
        1.0
    }
}

/// A lattice source with the elevation of every sample multiplied by the body's relief scale (WDEF-017): the exaggeration is part of the
/// generator's input, so heights, bounds, slopes, collision and picking all read one world. The other channels (climate, continent) are
/// the bake's own, so biomes and coasts stay where they are. The product is rounded to `f32` once; a scale of 1 passes the sample through.
pub struct ScaledLattice<'a> {
    inner: &'a dyn LatticeSource,
    scale: f64,
}

impl<'a> ScaledLattice<'a> {
    pub fn new(inner: &'a dyn LatticeSource, scale: f64) -> Self {
        Self { inner, scale }
    }
}

impl LatticeSource for ScaledLattice<'_> {
    fn res(&self) -> u32 {
        self.inner.res()
    }

    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let mut s = self.inner.at(face, i, j)?;
        if self.scale != 1.0 {
            s.elevation_m = (f64::from(s.elevation_m) * self.scale) as f32;
        }
        Ok(s)
    }

    fn block_cells(&self) -> u32 {
        self.inner.block_cells()
    }

    fn relief_scale(&self) -> f64 {
        self.scale * self.inner.relief_scale()
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
    let at = locate_direction(src.res(), map, dir)?;
    blend_cell(src, at.face, at.i, at.j, at.wu, at.wv)
}

/// Where a direction falls in the bake's lattice: the face, the cell `(i, j)` (the lower corner vertex) and the exact weights of
/// the next vertex along each axis (ADR 0027 point 2; a weight of 0 means the far vertex is not read).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatticePos {
    /// The lattice's cells per face edge.
    pub res: u32,
    pub face: Face,
    pub i: u32,
    pub j: u32,
    pub wu: f64,
    pub wv: f64,
}

impl LatticePos {
    /// The vertices that carry a non-zero weight (ADR 0041 point 1): the lower corner always (its weight `(1 - wu)(1 - wv)` is
    /// positive), the others only when their axis weight is not zero. At most four, in the order `(i, j)`, `(i + 1, j)`,
    /// `(i, j + 1)`, `(i + 1, j + 1)`; `res` is the lattice resolution (a vertex never lies beyond it).
    pub fn weighted_vertices(&self, res: u32) -> ([(u32, u32); 4], usize) {
        let mut out = [(self.i, self.j); 4];
        let mut n = 1;
        let i1 = (self.i + 1).min(res);
        let j1 = (self.j + 1).min(res);
        if self.wu != 0.0 {
            out[n] = (i1, self.j);
            n += 1;
        }
        if self.wv != 0.0 {
            out[n] = (self.i, j1);
            n += 1;
        }
        if self.wu != 0.0 && self.wv != 0.0 {
            out[n] = (i1, j1);
            n += 1;
        }
        (out, n)
    }
}

/// The position of `dir` in the lattice of `res` cells per face edge: the face and face coordinates come from `direction_to_face`,
/// the lattice position from the face coordinates in floating point. [`read_direction`] reads at this position.
pub fn locate_direction(res: u32, map: &dyn FaceMapping, dir: Vec3) -> Result<LatticePos, ReadError> {
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
    Ok(LatticePos { res, face, i, j, wu, wv })
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

/// Whether `src` has the block structure a gathered batch uses and a batch is worth gathering (`min_dirs` directions at least).
fn gatherable(src: &dyn LatticeSource, n: usize, min_dirs: usize) -> bool {
    let (res, bc) = (src.res(), src.block_cells());
    // (`cell_key` packs a cell index into 20 bits; a small batch is not worth a gather.)
    !(bc == 0 || res == 0 || bc > res || !res.is_power_of_two() || res >= 1 << 20 || n < min_dirs)
}

/// The footprint pass: the distinct cells `footprint` reads for `dirs`, as (face, Z-order block, cell key) sorted by block;
/// `None` when `stop` was set meanwhile.
fn footprint_by_block(
    src: &dyn LatticeSource,
    dirs: &[Vec3],
    stop: &dyn Fn() -> bool,
    footprint: &dyn Fn(&dyn LatticeSource, Vec3),
) -> Option<Vec<(u8, u64, u64)>> {
    let (res, bc) = (src.res(), src.block_cells());
    let gather = Gather { res, seen: Mutex::new(Vec::new()) };
    for (k, &d) in dirs.iter().enumerate() {
        if k % 64 == 0 && stop() {
            return None;
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
    Some(by_block)
}

fn read_cell(src: &dyn LatticeSource, face: u8, k: u64) -> (u64, Result<GlobalSample, ReadError>) {
    (k, src.at(Face(face), ((k >> 20) & 0xf_ffff) as u32, (k & 0xf_ffff) as u32))
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
    eval_gathered_from(src, dirs, stop, footprint, eval, 64)
}

/// [`eval_gathered`] together with `extra`, a computation that reads further lattice cells (a tile's climate, GEN-021): the cells
/// of `extra` join the footprint of the batch, so one gather reads the union grouped by block and every block of a tile is decoded
/// once however small the source's cache (GEN-020). `extra` is run once against a recording source and once against the table (or
/// against `src` when the batch is not gathered); its result is the one from the table. `stop` as in [`eval_gathered`]; when it
/// answers true the results are as there (0.0) and `extra` is run against what was read, so its result is then meaningless.
pub fn eval_gathered_with<R>(
    src: &dyn LatticeSource,
    dirs: &[Vec3],
    stop: &dyn Fn() -> bool,
    footprint: &dyn Fn(&dyn LatticeSource, Vec3),
    eval: &dyn Fn(&dyn LatticeSource, Vec3) -> f64,
    extra: &dyn Fn(&dyn LatticeSource) -> R,
) -> (Vec<f64>, R) {
    let mut out = vec![0.0; dirs.len()];
    if !gatherable(src, dirs.len(), 64) {
        for (k, &d) in dirs.iter().enumerate() {
            if k % 64 == 0 && stop() {
                break;
            }
            out[k] = eval(src, d);
        }
        return (out, extra(src));
    }
    let first = std::cell::Cell::new(true);
    let both = |g: &dyn LatticeSource, d: Vec3| {
        if first.replace(false) {
            let _ = extra(g);
        }
        footprint(g, d);
    };
    let Some(by_block) = footprint_by_block(src, dirs, stop, &both) else { return (out, extra(src)) };
    let mut cells = Vec::with_capacity(by_block.len());
    for (n, &(face, block, k)) in by_block.iter().enumerate() {
        if (n == 0 || (by_block[n - 1].0, by_block[n - 1].1) != (face, block) || n % 64 == 0) && stop() {
            return (out, extra(src));
        }
        cells.push(read_cell(src, face, k));
    }
    cells.sort_unstable_by_key(|c| c.0);
    let table = CellTable { res: src.res(), cells };
    for (k, &d) in dirs.iter().enumerate() {
        if k % 64 == 0 && stop() {
            break;
        }
        out[k] = eval(&table, d);
    }
    (out, extra(&table))
}

/// [`eval_gathered`] gathering a batch of `min_dirs` directions or more (a smaller one is the plain loop).
fn eval_gathered_from(
    src: &dyn LatticeSource,
    dirs: &[Vec3],
    stop: &dyn Fn() -> bool,
    footprint: &dyn Fn(&dyn LatticeSource, Vec3),
    eval: &dyn Fn(&dyn LatticeSource, Vec3) -> f64,
    min_dirs: usize,
) -> Vec<f64> {
    let mut out = vec![0.0; dirs.len()];
    if !gatherable(src, dirs.len(), min_dirs) {
        for (k, &d) in dirs.iter().enumerate() {
            if k % 64 == 0 && stop() {
                break;
            }
            out[k] = eval(src, d);
        }
        return out;
    }
    let Some(by_block) = footprint_by_block(src, dirs, stop, footprint) else { return out };
    let mut cells = Vec::with_capacity(by_block.len());
    for (n, &(face, block, k)) in by_block.iter().enumerate() {
        if (n == 0 || (by_block[n - 1].0, by_block[n - 1].1) != (face, block) || n % 64 == 0) && stop() {
            return out;
        }
        cells.push(read_cell(src, face, k));
    }
    cells.sort_unstable_by_key(|c| c.0);
    let table = CellTable { res: src.res(), cells };
    for (k, &d) in dirs.iter().enumerate() {
        if k % 64 == 0 && stop() {
            break;
        }
        out[k] = eval(&table, d);
    }
    out
}

/// [`eval_gathered`] on up to `threads` scoped threads (the plan builder's origin heights): the same results for every `threads`
/// (CON-14). The distinct blocks of the batch are dealt to the threads, each of which reads the cells of one block at a time
/// (so a block is still decoded once, however small the source's cache, and `threads` blocks are in flight at once); the
/// evaluation from the table is split over the threads in contiguous chunks. A batch of a few directions is gathered too (a
/// cold plan at altitude has tens of nodes whose footprints are hundreds of blocks). `threads` of 0 or 1 gathers on the calling thread.
pub fn eval_gathered_threads(
    src: &dyn LatticeSource,
    dirs: &[Vec3],
    stop: &(dyn Fn() -> bool + Sync),
    threads: usize,
    footprint: &(dyn Fn(&dyn LatticeSource, Vec3) + Sync),
    eval: &(dyn Fn(&dyn LatticeSource, Vec3) -> f64 + Sync),
) -> Vec<f64> {
    if threads <= 1 {
        return eval_gathered_from(src, dirs, stop, footprint, eval, 2);
    }
    if !gatherable(src, dirs.len(), 2) {
        return crate::height::eval_chunked(dirs, stop, threads, &|d| eval(src, d));
    }
    let out = vec![0.0; dirs.len()];
    let Some(by_block) = footprint_by_block(src, dirs, stop, footprint) else { return out };
    // Contiguous runs of one (face, block).
    let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
    for (n, b) in by_block.iter().enumerate() {
        match groups.last_mut() {
            Some(g) if (by_block[g.start].0, by_block[g.start].1) == (b.0, b.1) => g.end = n + 1,
            _ => groups.push(n..n + 1),
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let stopped = std::sync::atomic::AtomicBool::new(false);
    let parts: Vec<Vec<(u64, Result<GlobalSample, ReadError>)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads.min(groups.len()))
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let g = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if g >= groups.len() {
                            return mine;
                        }
                        if stop() {
                            stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                            return mine;
                        }
                        for (n, &(face, _, k)) in by_block[groups[g].clone()].iter().enumerate() {
                            if n % 64 == 63 && stop() {
                                stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                                return mine;
                            }
                            mine.push(read_cell(src, face, k));
                        }
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("block read thread")).collect()
    });
    // (A thread that saw the flag recorded it: the table may be partial, whatever the flag says now.)
    if stopped.load(std::sync::atomic::Ordering::Relaxed) || stop() {
        return out;
    }
    let mut cells: Vec<_> = parts.into_iter().flatten().collect();
    cells.sort_unstable_by_key(|c| c.0);
    let table = CellTable { res: src.res(), cells };
    crate::height::eval_chunked(dirs, stop, threads, &|d| eval(&table, d))
}

/// The cells a set of reads needs, fetched once from a block-structured source in block order (GEN-020): a source of its own that
/// answers exactly those cells from memory.
pub struct GatheredCells(CellTable);

impl LatticeSource for GatheredCells {
    fn res(&self) -> u32 {
        self.0.res
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        self.0.at(face, i, j)
    }
}

/// Gather the cells `reads` takes from `src`: `reads(g)` is run once against a recording source (its results are ignored), the
/// distinct cells it touched are read from `src` grouped by block, so each block is decoded once however small the source's cache, and
/// the returned source answers them. `None` when `src` has no block structure (an in-memory grid): read it directly. Every cell is
/// the source's own sample, so a read through the table is bit for bit the read from `src`.
pub fn gather_cells(src: &dyn LatticeSource, reads: &dyn Fn(&dyn LatticeSource)) -> Option<GatheredCells> {
    if !gatherable(src, 1, 1) {
        return None;
    }
    let by_block = footprint_by_block(src, &[Vec3::new(0.0, 0.0, 0.0)], &|| false, &|g, _| reads(g))?;
    let mut cells: Vec<_> = by_block.iter().map(|&(face, _, k)| read_cell(src, face, k)).collect();
    cells.sort_unstable_by_key(|c| c.0);
    Some(GatheredCells(CellTable { res: src.res(), cells }))
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

/// An equirectangular map (longitude along x, latitude up; COORD's axes, so the poles are +-z) of one channel straight from [`sample`], `width` by `width / 2`
/// pixels: the picture people expect of a planet, and independent of the face grid.
pub fn equirect_product(seed: u64, width: u32, channel: Channel) -> crate::dump::Rgba {
    let height = (width / 2).max(1);
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for py in 0..height {
        let lat = (0.5 - (f64::from(py) + 0.5) / f64::from(height)) * core::f64::consts::PI;
        let (sl, cl) = (libm::sin(lat), libm::cos(lat));
        for px in 0..width {
            let lon = ((f64::from(px) + 0.5) / f64::from(width) - 0.5) * 2.0 * core::f64::consts::PI;
            // COORD's axes: +z the north pole, longitude 0 at +x, 90 degrees east at +y.
            let d = Vec3::new(cl * libm::cos(lon), cl * libm::sin(lon), sl);
            data.extend(colour(&sample(seed, d), channel));
        }
    }
    crate::dump::Rgba { width, height, data }
}
