//! The terrain field: heights from the global bake plus local detail, a pure function of the direction (GEN-016, ADR 0028).
//!
//! `height(dir)` reads the global-coarse elevation at the direction ([`read_direction`]) and adds detail noise whose
//! amplitude follows the ground (rougher where it is high). There is one path from a direction to a height: tile samples,
//! tile centres, plan origins, camera clearance and cache-verify all call `height`, so the same world point has the same
//! bits whichever tile or consumer asks (COORD-007: shared border samples have identical direction bits). New implementation
//! id: no existing golden, plan snapshot or known answer moves.

use std::sync::Mutex;

use planet_core::cube::TangentWarp;
use planet_core::hash::hash_u64s;
use planet_core::Face;
use planet_core::Vec3;
use planet_world_def::{records_hash64, BodyId, GenContext, MissingNode, ReadRecord};

use crate::global::{blend_cell, read_direction, LatticeSource, GLOBAL_VERSION, MAX_ELEVATION_M};
use crate::height::{layer_noise, layer_noise_bounds, HeightField};

/// Bump whenever any output of this module changes (ADR 0006); it also covers [`GLOBAL_VERSION`] through the bake key.
pub const TERRAIN_VERSION: u32 = 1;
/// Names this implementation in cache keys (CON-07).
pub const TERRAIN_IMPLEMENTATION_ID: &str = "cpu-terrain-v1";

/// Domain tag of the detail lattice.
const LAYER_DETAIL: u64 = 0x5445_0001;
/// Lowest octave frequency of the detail (unit-sphere lattice cells per radius): a wavelength of about 600 km.
const DETAIL_BASE_FREQUENCY: f64 = 64.0;
const DETAIL_OCTAVES: u32 = 14;
/// Amplitude ratio between octaves; below 0.5 the slope contribution of the octaves decays instead of staying constant.
const DETAIL_PERSISTENCE: f64 = 0.45;
/// Largest detail amplitude in metres (high ground); the detail never exceeds it in magnitude.
pub const DETAIL_MAX_M: f64 = 300.0;
/// Detail amplitude over the ocean.
const DETAIL_OCEAN_M: f64 = 10.0;
/// Detail amplitude on lowland.
const DETAIL_LAND_M: f64 = 30.0;

/// Largest height the field takes in magnitude: the bake's elevation clamp plus the detail.
pub const MAX_ABS_HEIGHT_M: f64 = MAX_ELEVATION_M + DETAIL_MAX_M;

/// Face coordinates per radian, at most, over the extended region |u|, |v| <= 1.2 (face coordinates up to 1.14): the gradient
/// of the face coordinate `s = (4 / pi) atan(x / z)` on the unit sphere has magnitude `(4 / pi) / sqrt(1 - y^2)`, and `y^2`
/// is largest at the middle of the edge of the region, `1.44 / 2.44`: 1.99 there; 2 is used (ADR 0028 point 3).
const FACE_LIPSCHITZ: f64 = 2.0;
/// Most lattice cells a bounds query scans before it takes the global range.
const MAX_BOUNDS_CELLS: u64 = 4096;

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Detail amplitude in metres at a ground elevation: small over the ocean, growing over land and with height, at most
/// [`DETAIL_MAX_M`].
pub fn detail_amplitude_m(elevation_m: f64) -> f64 {
    if elevation_m <= 0.0 {
        DETAIL_OCEAN_M
    } else {
        DETAIL_LAND_M + (DETAIL_MAX_M - DETAIL_LAND_M) * smoothstep(0.0, 5000.0, elevation_m)
    }
}

/// Normalised detail noise in [-1, 1] at a direction.
fn detail_noise(seed: u64, dir: Vec3) -> f64 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, DETAIL_BASE_FREQUENCY);
    for o in 0..DETAIL_OCTAVES {
        sum += amp * layer_noise(seed, LAYER_DETAIL, o, dir * freq);
        norm += amp;
        amp *= DETAIL_PERSISTENCE;
        freq *= 2.0;
    }
    sum / norm
}

/// The terrain field of one body over a baked global-coarse grid.
pub struct TerrainField<'a> {
    seed: u64,
    body: BodyId,
    inputs: u64,
    records: Vec<ReadRecord>,
    bake_key: [u8; 16],
    src: &'a dyn LatticeSource,
    /// The first read error of the bake, if any: `height` cannot return a `Result`, so a failed read gives NaN (never a
    /// plausible number) and the caller checks [`TerrainField::error`] after generating (a tile build or a verify does).
    error: Mutex<Option<String>>,
}

impl TerrainField<'_> {
    /// The first error reading the bake since the field was built, if any.
    pub fn error(&self) -> Option<String> {
        self.error.lock().expect("terrain error mutex").clone()
    }

    fn fail(&self, e: &crate::global::ReadError) {
        let mut slot = self.error.lock().expect("terrain error mutex");
        if slot.is_none() {
            *slot = Some(e.to_string());
        }
    }

    /// The ground elevation of the bake at a direction, without detail.
    pub fn base_elevation(&self, dir: Vec3) -> f64 {
        self.base_via(self.src, dir)
    }

    fn base_via(&self, src: &dyn LatticeSource, dir: Vec3) -> f64 {
        match read_direction(src, &TangentWarp, dir) {
            Ok(s) => f64::from(s.elevation_m),
            Err(e) => {
                self.fail(&e);
                f64::NAN
            }
        }
    }
}

impl TerrainField<'_> {
    /// [`HeightField::height`] reading the lattice through `src`.
    fn height_via(&self, src: &dyn LatticeSource, dir: Vec3) -> f64 {
        let base = self.base_via(src, dir);
        base + detail_amplitude_m(base) * detail_noise(self.seed, dir)
    }
}

impl HeightField for TerrainField<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.height_via(self.src, dir)
    }

    /// The lattice cells of the whole batch are read once, grouped by bake block (GEN-020): the same values, in the caller's order.
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        let footprint = |g: &dyn LatticeSource, d: Vec3| {
            self.base_via(g, d);
        };
        crate::global::eval_gathered(self.src, dirs, stop, &footprint, &|s, d| self.height_via(s, d))
    }

    /// [`HeightField::heights_until`] with the distinct blocks of the batch read on `threads` threads (GEN-020, LOD-009).
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        let footprint = |g: &dyn LatticeSource, d: Vec3| {
            self.base_via(g, d);
        };
        crate::global::eval_gathered_threads(self.src, dirs, stop, threads, &footprint, &|s, d| self.height_via(s, d))
    }

    fn seed(&self) -> u64 {
        self.seed
    }

    fn implementation_id(&self) -> &'static str {
        TERRAIN_IMPLEMENTATION_ID
    }

    fn version(&self) -> u32 {
        TERRAIN_VERSION
    }

    fn max_abs_height_m(&self) -> f64 {
        MAX_ABS_HEIGHT_M
    }

    fn inputs_hash(&self) -> u64 {
        self.inputs
    }

    fn read_records(&self) -> Vec<ReadRecord> {
        self.records.clone()
    }

    fn body(&self) -> BodyId {
        self.body
    }

    fn source_key(&self) -> Option<[u8; 16]> {
        Some(self.bake_key)
    }

    /// Bounds over a cap (ADR 0028 point 3): the minimum and maximum bake elevation over the lattice cells the cap can read,
    /// widened by one cell, plus and minus the largest detail. A cap that leaves its face, or covers more than
    /// [`MAX_BOUNDS_CELLS`] cells, takes the global range.
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let global = (-MAX_ABS_HEIGHT_M, MAX_ABS_HEIGHT_M);
        let scan = match scan_base(self.src, centre, angular_radius) {
            Ok(Some(scan)) => scan,
            Ok(None) => return global,
            Err(err) => {
                self.fail(&err);
                return global;
            }
        };
        let (blo, bhi) = (scan.lo, scan.hi);
        // Detail: the amplitude is a nondecreasing function of the base elevation; the normalised noise has the bounds of
        // its octaves, summed with their weights.
        let (mut nlo, mut nhi, mut amp, mut norm, mut freq) = (0.0, 0.0, 1.0, 0.0, DETAIL_BASE_FREQUENCY);
        for o in 0..DETAIL_OCTAVES {
            let (l, h) = layer_noise_bounds(self.seed, LAYER_DETAIL, o, centre, angular_radius, freq);
            nlo += amp * l;
            nhi += amp * h;
            norm += amp;
            amp *= DETAIL_PERSISTENCE;
            freq *= 2.0;
        }
        let (nlo, nhi) = ((nlo / norm - 1e-9).max(-1.0), (nhi / norm + 1e-9).min(1.0));
        let (a_min, a_max) = (detail_amplitude_m(blo), detail_amplitude_m(bhi));
        let dlo = if nlo >= 0.0 { a_min * nlo } else { a_max * nlo };
        let dhi = if nhi >= 0.0 { a_max * nhi } else { a_min * nhi };
        // Margin for rounding in the blend and the sums (1e-6 m) and for the rounding of heights to f32 (twice the half-ulp,
        // 2^-23 of the magnitude, since meshes store f32): the bounds hold the f32-rounded heights too.
        let pad = |x: f64| 1e-6 + x.abs() * 1.2e-7;
        let (l, h) = (blo + dlo, bhi + dhi);
        ((l - pad(l)).max(global.0), (h + pad(h)).min(global.1))
    }
}

/// The range of the bake's elevation over a cap and the steepest lattice step on the way (see [`scan_base`]).
pub(crate) struct BaseScan {
    pub lo: f64,
    pub hi: f64,
    /// Largest difference of adjacent lattice samples along the two lattice axes, over every cell scanned (metres per cell).
    pub max_du: f64,
    pub max_dv: f64,
}

/// Bounds of the bake's elevation over the cap of `angular_radius` about `centre`, and the largest lattice steps of the
/// cells it reads (ADR 0028 point 3). `None`: the cap is too wide or leaves the scanned region, take the global range.
pub(crate) fn scan_base(src: &dyn LatticeSource, centre: Vec3, angular_radius: f64) -> Result<Option<BaseScan>, crate::global::ReadError> {
    let res = src.res();
    if res == 0 || !angular_radius.is_finite() || angular_radius < 0.0 {
        return Ok(None);
    }
    // Face coordinates are Lipschitz (FACE_LIPSCHITZ) in the angle while they stay within 1.14 (u within 1.2). With a
    // reach `rw` of at most 0.1, every direction of the cap that lies in the cone of a face F is within `rw` of the
    // centre in F's (extended) coordinates, so the faces to scan are those whose extended coordinates of the centre
    // are within 1 + rw, and in each the rectangle centre +- rw clipped to the face holds every cap direction of F.
    let rw = FACE_LIPSCHITZ * angular_radius;
    if rw > 0.1 {
        return Ok(None);
    }
    let half = f64::from(res) * 0.5;
    let reach = rw * half + 1e-9;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut max_du, mut max_dv) = (0.0f64, 0.0f64);
    let mut cells = 0u64;
    for f in 0..6u8 {
        let face = Face(f);
        let w = centre.dot(face.normal());
        if w <= 0.0 {
            continue;
        }
        // The extended coordinate (`plane_to_face` clamps beyond the face): `(4 / pi) atan(u)`.
        let coord = |axis: Vec3| libm::atan(centre.dot(axis) / w) / core::f64::consts::FRAC_PI_4;
        let (s, t) = (coord(face.u_axis()), coord(face.v_axis()));
        if s.abs() > 1.0 + rw + 1e-9 || t.abs() > 1.0 + rw + 1e-9 {
            continue;
        }
        let (pu, pv) = ((s + 1.0) * half, (t + 1.0) * half);
        let clamp = |x: f64| x.clamp(0.0, f64::from(res));
        let (u0, u1, v0, v1) = (clamp(pu - reach), clamp(pu + reach), clamp(pv - reach), clamp(pv + reach));
        // Cells read: floor(u0) ..= floor(u1) and one more for the far neighbour of the blend, clamped to the lattice.
        let (i0, i1) = (libm::floor(u0) as u32, (libm::floor(u1) as u32 + 1).min(res));
        let (j0, j1) = (libm::floor(v0) as u32, (libm::floor(v1) as u32 + 1).min(res));
        let (w, h) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
        cells += (w * h) as u64;
        if cells > MAX_BOUNDS_CELLS {
            return Ok(None);
        }
        let mut e = Vec::with_capacity(w * h);
        for j in j0..=j1 {
            for i in i0..=i1 {
                e.push(f64::from(src.at(face, i, j)?.elevation_m));
            }
        }
        let (flo, fhi) = (e.iter().copied().fold(f64::INFINITY, f64::min), e.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        // The blend is bilinear in the lattice position, so its slope along an axis is at most the largest difference of
        // adjacent samples along that axis. Every cap point of this face is within the rectangle's half-width (per
        // axis, in lattice units) of the anchor, the centre clamped to the face; the base elevation there is the
        // blend at the anchor (read from this face's own samples) plus or minus the slope times that half-width.
        let (mut du, mut dv) = (0.0f64, 0.0f64);
        for j in 0..h {
            for i in 0..w {
                if i + 1 < w {
                    du = du.max((e[j * w + i + 1] - e[j * w + i]).abs());
                }
                if j + 1 < h {
                    dv = dv.max((e[(j + 1) * w + i] - e[j * w + i]).abs());
                }
            }
        }
        let (au, av) = (clamp(pu), clamp(pv));
        let (ci, cj) = (libm::floor(au).min(f64::from(res)) as u32, libm::floor(av).min(f64::from(res)) as u32);
        let anchor = f64::from(blend_cell(src, face, ci, cj, au - f64::from(ci), av - f64::from(cj))?.elevation_m);
        // The anchor and the base at a cap point are both rounded to f32 by the blend: one ulp (2^-23 relative) apart at most
        // beyond the slope term.
        let spread = (du + dv) * reach * (1.0 + 1e-9) + 1e-6 + anchor.abs() * 2.4e-7;
        max_du = max_du.max(du);
        max_dv = max_dv.max(dv);
        lo = lo.min(flo.max(anchor - spread));
        hi = hi.max(fhi.min(anchor + spread));
    }
    if lo > hi {
        return Ok(None);
    }
    Ok(Some(BaseScan { lo, hi, max_du, max_dv }))
}

/// Builds the [`TerrainField`] of a body from a definition and a baked grid.
#[derive(Clone, Copy, Debug, Default)]
pub struct TerrainGenerator;

impl TerrainGenerator {
    pub fn implementation_id(&self) -> &'static str {
        TERRAIN_IMPLEMENTATION_ID
    }

    pub fn version(&self) -> u32 {
        TERRAIN_VERSION
    }

    /// The field of `body` over `bake`: the seed is read through `ctx` (the world seed and the body's derived seed are
    /// recorded). `bake_key` is the 128-bit stage key of the bake (STRM-011): it enters the field's `inputs_hash` and is the
    /// source of its persistent height key, so a different bake re-keys every tile made from it. `ctx` must be fresh.
    pub fn field<'a>(
        &self,
        ctx: &GenContext,
        body: BodyId,
        bake: &'a dyn LatticeSource,
        bake_key: [u8; 16],
    ) -> Result<TerrainField<'a>, MissingNode> {
        assert!(ctx.reads().is_empty(), "a field is built from a fresh context: its key covers the whole read set of the context");
        let seed = ctx.body_seed(body)?;
        let records = ctx.read_records();
        let half = |b: &[u8]| u64::from_le_bytes(b.try_into().expect("8 bytes"));
        let inputs = hash_u64s(
            0x7465_7272,
            &[records_hash64(&records), half(&bake_key[..8]), half(&bake_key[8..]), u64::from(GLOBAL_VERSION), u64::from(TERRAIN_VERSION)],
        );
        Ok(TerrainField { seed, body, inputs, records, bake_key, src: bake, error: Mutex::new(None) })
    }
}
