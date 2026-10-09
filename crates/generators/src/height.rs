//! CPU reference height generator: 3D value-noise fBm evaluated on the unit direction.
//!
//! The field is a pure function of `(seed, direction)`. A tile is a regular grid of such samples whose
//! directions come from `TileId::sample_direction`, so shared borders (also across cube faces) and
//! parent/child samples are the same directions bit for bit, hence the same heights.

use planet_core::cube::FaceMapping;
use planet_core::hash::{hash_u64s, unit_f64, HASH_VERSION};
use planet_core::{TileId, Vec3};

/// Bump whenever any output of this module changes (ADR 0006).
pub const GENERATOR_VERSION: u32 = 2;
/// Names this implementation in cache keys: CPU reference, hash version, and the face-mapping id is added by callers.
pub const IMPLEMENTATION_ID: &str = "cpu-ref-height-v2";

/// Domain tag so lattices of different layers never alias for the same seed.
const LAYER_HEIGHT: u64 = 0x4845_4947_4854;
const OCTAVES: u32 = 5;
const BASE_FREQUENCY: f64 = 4.0;
/// Peak amplitude of the summed noise in metres.
const AMPLITUDE_M: f64 = 4000.0;

fn lattice_value(seed: u64, octave: u32, ix: i64, iy: i64, iz: i64) -> f64 {
    let h = hash_u64s(seed, &[LAYER_HEIGHT, u64::from(octave), ix as u64, iy as u64, iz as u64]);
    unit_f64(h) * 2.0 - 1.0
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Single-octave value noise at `p`, in [-1, 1].
fn value_noise(seed: u64, octave: u32, p: Vec3) -> f64 {
    let (fx, fy, fz) = (libm::floor(p.x), libm::floor(p.y), libm::floor(p.z));
    let (tx, ty, tz) = (smooth(p.x - fx), smooth(p.y - fy), smooth(p.z - fz));
    let (ix, iy, iz) = (fx as i64, fy as i64, fz as i64);
    let c = |dx: i64, dy: i64, dz: i64| lattice_value(seed, octave, ix + dx, iy + dy, iz + dz);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), tx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), tx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), tx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), tx);
    lerp(lerp(x00, x10, ty), lerp(x01, x11, ty), tz)
}

/// Height in metres at a unit direction.
pub fn height_at(seed: u64, dir: Vec3) -> f64 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, BASE_FREQUENCY);
    for octave in 0..OCTAVES {
        sum += amp * value_noise(seed, octave, dir * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    AMPLITUDE_M * sum / norm
}

/// One generated tile: `(res + 1)²` height samples, row `l` (t direction) major, column `k` (s direction).
#[derive(Clone, Debug, PartialEq)]
pub struct TileData {
    pub id: TileId,
    pub res: u32,
    pub heights: Vec<f32>,
}

impl TileData {
    pub fn at(&self, k: u32, l: u32) -> f32 {
        self.heights[(l * (self.res + 1) + k) as usize]
    }

    /// Hash of the tile contents (id, resolution, height bits), versioned with `HASH_VERSION`.
    pub fn content_hash(&self) -> u64 {
        let mut v = vec![u64::from(HASH_VERSION), u64::from(GENERATOR_VERSION), self.id.raw(), u64::from(self.res)];
        v.extend(self.heights.iter().map(|h| u64::from(h.to_bits())));
        hash_u64s(0x7469_6c65, &v)
    }
}

/// Generate a tile on a `res`×`res` cell grid (`res` a power of two).
pub fn generate_tile(seed: u64, map: &dyn FaceMapping, id: TileId, res: u32) -> TileData {
    let mut heights = Vec::with_capacity(((res + 1) * (res + 1)) as usize);
    for l in 0..=res {
        for k in 0..=res {
            heights.push(height_at(seed, id.sample_direction(map, res, k, l)) as f32);
        }
    }
    TileData { id, res, heights }
}

/// Generate many tiles, in the order given, using `threads` worker threads. Each tile is independent and
/// the output order is the input order, so the result does not depend on the thread count (CON-14).
pub fn generate_region(seed: u64, map: &(dyn FaceMapping + Sync), ids: &[TileId], res: u32, threads: usize) -> Vec<TileData> {
    let threads = threads.max(1).min(ids.len().max(1));
    let chunk = ids.len().div_ceil(threads).max(1);
    let mut out: Vec<TileData> = Vec::with_capacity(ids.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = ids
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|&id| generate_tile(seed, map, id, res)).collect::<Vec<_>>()))
            .collect();
        for h in handles {
            out.extend(h.join().expect("generator thread panicked"));
        }
    });
    out
}

fn fold_str(s: &str) -> Vec<u64> {
    s.bytes().map(u64::from).collect()
}

/// Key under which a generated tile may be cached: everything its content depends on (ADR 0006). Changing the
/// generator version, hash version, implementation id, face mapping, seed, tile or resolution changes the key.
pub fn cache_key(map: &dyn FaceMapping, seed: u64, id: TileId, res: u32) -> u64 {
    let mut v = vec![u64::from(GENERATOR_VERSION), u64::from(HASH_VERSION), seed, id.raw(), u64::from(res)];
    v.push(hash_u64s(1, &fold_str(IMPLEMENTATION_ID)));
    v.push(hash_u64s(2, &fold_str(map.id())));
    hash_u64s(0x006b_6579, &v)
}
