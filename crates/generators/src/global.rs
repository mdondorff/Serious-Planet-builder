//! The global-coarse stage: continents, elevation and climate of a whole body (GEN-013, GEN-007, CON-08, ADR 0022).
//!
//! One field with four channels, because under ADR 0022 option 1 a chain of steps is one field: the continent mask feeds
//! the elevation and the elevation feeds the temperature, all inside [`sample`]. It is a pure function of `(seed,
//! unit direction)`: integer hashing, IEEE `+ - * /`, `libm::floor`, `libm::sqrt`, `libm::asin`, `libm::exp`, so results
//! are bit-identical across platforms and thread counts (CON-14). The bake evaluates it at the vertices of a cube-face
//! grid (the same directions as the tile samples of the same level), so a shared border sample is one value, and the
//! result does not depend on the order or the number of threads that computed it.
//!
//! This is a new implementation ([`IMPLEMENTATION_ID`]) beside the reference height field: existing goldens, plan
//! snapshots and known answers do not move. Hydrology is not here (SPIKE-03).

use planet_core::cube::{face_to_direction, FaceMapping};
use planet_core::{Face, Vec3};

use crate::height::layer_noise;

/// Bump whenever any output of this module changes (ADR 0006).
pub const GLOBAL_VERSION: u32 = 1;
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
pub const SEA_LEVEL_C: f64 = 0.10;
/// Lowest and highest elevation in metres (clamps).
pub const MIN_ELEVATION_M: f64 = -9000.0;
pub const MAX_ELEVATION_M: f64 = 9000.0;

/// The four channels of the global-coarse field at one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalSample {
    /// Continentalness in about [-1, 1]; above [`SEA_LEVEL_C`] is land.
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
    let continent = fbm(seed, LAYER_CONTINENT, warped, 1.1, 5) * 1.7;

    // Elevation: ocean basins and shelves from the continent value, mountain ranges from ridged noise inside range
    // zones, rising inland.
    let land_depth = continent - SEA_LEVEL_C;
    let base = if land_depth < 0.0 {
        // Shelf then abyss: -200 m at the coast down to -4500 m far from it.
        -200.0 - 4300.0 * smoothstep(0.0, 0.9, -land_depth)
    } else {
        // Lowlands rising slowly to plateaus.
        60.0 + 900.0 * smoothstep(0.0, 0.9, land_depth)
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
        continent: continent as f32,
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
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::BadResolution(r) => {
                write!(f, "a global bake needs a power-of-two resolution from 1 to {MAX_BAKE_RES} cells per face edge, got {r}")
            }
        }
    }
}

impl std::error::Error for BakeError {}

/// The largest in-memory bake: 1024 cells per face edge is about 10 km at Earth's radius. Finer global grids (1 km is 8192
/// per face edge) are baked as tiles and never held whole.
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

/// A 64-bit hash of every bit of every channel of a grid, in face and row-major order: the determinism manifest entry
/// of the bake (GEN-003 style) and the quick equality check of tests.
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
