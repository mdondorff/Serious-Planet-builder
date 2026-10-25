//! The eroded terrain field (GEN-017, ADR 0030 point 1): the global bake's elevation plus ridged creases that cut deeper
//! where the bake is steep (SPIKE-04 variant B), a pure function of the direction. It replaces the plain fBm detail of
//! [`crate::terrain`] under its own implementation id; both are one field per ADR 0022 option 1 (no same-stage reads).
//!
//! `height(dir) = E(dir) + a0 * carve * S(dir)` where `E` is the bake ([`read_direction`]), `a0` grows with the ground
//! elevation, `carve` with the bake's slope at `dir` (a rotation-free central-difference gradient over a few bake cells),
//! and `S` in [-0.75, 0.25] sums, over [`OCTAVES`] octaves, the ridged creases `0.25 - (1 - |noise|)^3` (a crease is where
//! the noise crosses zero).
//!
//! Bounds are interval arithmetic over a cap: the bake's range and Lipschitz constant from the lattice cells the cap and the
//! slope stencil can read ([`crate::terrain::scan_base`]), the slope interval from the gradient at the centre and its
//! variation over the cap, the per-octave noise bounds ([`layer_noise_bounds`]), and the product of the intervals.

use std::sync::{Arc, Mutex, OnceLock};

use planet_core::cube::TangentWarp;
use planet_core::hash::hash_u64s;
use planet_core::Vec3;
use planet_world_def::{records_hash64, BodyId, GenContext, MissingNode, ReadRecord};

use crate::global::{read_direction, LatticeSource, GLOBAL_VERSION, MAX_ELEVATION_M, MIN_ELEVATION_M};
use crate::height::{layer_noise, layer_noise_bounds, HeightField};
use crate::terrain::scan_base;

/// Bump whenever any output of this module changes (ADR 0006); it also covers [`GLOBAL_VERSION`] through the bake key.
pub const EROSION_VERSION: u32 = 2;
/// Names this implementation in cache keys (CON-07).
pub const EROSION_IMPLEMENTATION_ID: &str = "cpu-erosion-v1";

/// Domain tag of the crease lattice.
const LAYER_CREASE: u64 = 0x4552_0001;
const OCTAVES: u32 = 12;
/// Lowest octave frequency (lattice cells per unit radius): a crease spacing of about 25 km; the finest octave is 12 m.
const BASE_FREQUENCY: f64 = 256.0;
const PERSISTENCE: f64 = 0.55;
/// Amplitude in metres on the ocean floor and the extra amplitude at full land, before the slope factor.
const AMPLITUDE_OCEAN_M: f64 = 80.0;
const AMPLITUDE_LAND_M: f64 = 900.0;
/// Bake cells either side of a point over which the slope is measured.
const SLOPE_CELLS: f64 = 3.0;
/// The slope factor: this share of the amplitude on flat ground, the rest from slope 0.01 (rise over run) to 0.2.
const CARVE_FLOOR: f64 = 0.15;
const CARVE_FROM: f64 = 0.01;
const CARVE_TO: f64 = 0.2;
/// The octave the creases are split at (GEN-061): octaves below it (wavelengths over 1.6 km at 25 km spacing halving) keep the amplitude of the bake's
/// slope; the fine ones (under 1.6 km) lift it towards the full amplitude by the landform driver.
const CREASE_SPLIT: u32 = 4;
/// The crease sum lies in [-0.75, 0.25], so the detail lies in [-0.75, 0.25] times the largest amplitude.
const AMPLITUDE_MAX_M: f64 = AMPLITUDE_OCEAN_M + AMPLITUDE_LAND_M;
/// The lower bound of the crease sum.
const CREASE_LOW: f64 = -0.75;
const DETAIL_DOWN_M: f64 = 0.75 * AMPLITUDE_MAX_M;

/// Largest height the field takes in magnitude: the bake's elevation clamp plus the deepest incision.
pub const MAX_ABS_HEIGHT_M: f64 = (if MAX_ELEVATION_M > -MIN_ELEVATION_M { MAX_ELEVATION_M } else { -MIN_ELEVATION_M }) + DETAIL_DOWN_M;

use planet_core::smoothstep;

/// Crease amplitude at a ground elevation (nondecreasing) and a slope factor.
fn amplitude_m(elevation_m: f64, carve: f64) -> f64 {
    (AMPLITUDE_OCEAN_M + AMPLITUDE_LAND_M * smoothstep(-200.0, 300.0, elevation_m)) * carve
}

/// Share of the amplitude at a slope (rise over run; nondecreasing).
fn carve(slope: f64) -> f64 {
    CARVE_FLOOR + (1.0 - CARVE_FLOOR) * smoothstep(CARVE_FROM, CARVE_TO, slope)
}

/// The deepest cut (m, positive) the erosion makes into flat ground (slope below `CARVE_FROM`: the slope factor is `CARVE_FLOOR`) of
/// bake elevation `elevation_m`: `0.75 A(E)`, 0.75 being the lower bound of the crease sum.
pub fn flat_ground_incision_m(elevation_m: f64) -> f64 {
    -CREASE_LOW * amplitude_m(elevation_m, carve(0.0))
}

/// The highest bake elevation (m) at which flat ground can still lie below the level `level_m`: the fixed point of
/// `E = level + flat_ground_incision_m(E)`. Found by iterating from the level (the map is increasing and contracting, so the root is
/// unique and the result is the same bits everywhere, CON-14); `level_m = 0` gives about 64 m. The coastal band of the water field reads
/// it (ADR 0056).
pub fn flat_ground_band_elevation_m(level_m: f64) -> f64 {
    let mut e = level_m;
    for _ in 0..64 {
        e = level_m + flat_ground_incision_m(e);
    }
    e
}

/// One octave's term `0.25 - (1 - |n|)^3` for a noise value.
fn crease(n: f64) -> f64 {
    let t = (1.0 - n.abs()).max(0.0);
    0.25 - t * t * t
}

/// Range of [`crease`] over noise values in `[nlo, nhi]`.
fn crease_range(nlo: f64, nhi: f64) -> (f64, f64) {
    let (mut a, mut b) = if nlo >= 0.0 {
        (nlo, nhi)
    } else if nhi <= 0.0 {
        (-nhi, -nlo)
    } else {
        (0.0, (-nlo).max(nhi))
    };
    a = a.clamp(0.0, 1.0);
    b = b.clamp(0.0, 1.0);
    // The term grows with |n|.
    (crease(a), crease(b))
}

/// The normalised crease sum at a direction in two parts, the coarse octaves (below [`CREASE_SPLIT`]) and the fine ones: their sum lies in [-0.75, 0.25].
fn crease_parts(seed: u64, dir: Vec3) -> (f64, f64) {
    let (mut coarse, mut fine, mut amp, mut norm, mut freq) = (0.0, 0.0, 1.0, 0.0, BASE_FREQUENCY);
    for o in 0..OCTAVES {
        let term = amp * crease(layer_noise(seed, LAYER_CREASE, o, dir * freq));
        if o < CREASE_SPLIT {
            coarse += term;
        } else {
            fine += term;
        }
        norm += amp;
        amp *= PERSISTENCE;
        freq *= 2.0;
    }
    (coarse / norm, fine / norm)
}

/// The slope factor of the fine creases (GEN-061): the factor of the bake's slope lifted towards 1 by the landform driver `l` in `[0, 1]`
/// (`l` 0 on a plain or at the coast: the factor of the slope alone; `l` 1 deep inland in a range zone: the full amplitude whatever the slope).
fn fine_carve(carve: f64, landform: f64) -> f64 {
    carve + (1.0 - carve) * landform
}

/// The detail in metres at a point of ground elevation `e`, bake slope factor `carve` and landform `landform`: the coarse creases at the amplitude of the
/// slope, the fine ones at the amplitude lifted by the landform.
fn detail_m(seed: u64, dir: Vec3, e: f64, carve: f64, landform: f64) -> f64 {
    let (coarse, fine) = crease_parts(seed, dir);
    amplitude_m(e, carve) * coarse + amplitude_m(e, fine_carve(carve, landform)) * fine
}

/// The eroded terrain field of one body over a baked global-coarse grid.
pub struct ErosionField<'a> {
    seed: u64,
    body: BodyId,
    radius_m: f64,
    inputs: u64,
    records: Vec<ReadRecord>,
    bake_key: [u8; 16],
    src: crate::global::ScaledLattice<'a>,
    /// The relief scale of the body (WDEF-017): the factor on the bake's elevation, which the bounds follow.
    relief: f64,
    /// The first read error of the bake, if any (see [`crate::terrain::TerrainField`]): a failed read gives NaN.
    error: Mutex<Option<String>>,
    /// The landform lattice of the seed (GEN-062), taken from the shared cache on first use.
    landform: OnceLock<Arc<crate::global::LandformLattice>>,
}

impl ErosionField<'_> {
    /// The first error reading the bake since the field was built, if any.
    pub fn error(&self) -> Option<String> {
        self.error.lock().expect("erosion error mutex").clone()
    }

    fn landform(&self) -> &crate::global::LandformLattice {
        self.landform.get_or_init(|| crate::global::landform_lattice(self.seed))
    }

    fn fail(&self, e: &crate::global::ReadError) {
        let mut slot = self.error.lock().expect("erosion error mutex");
        if slot.is_none() {
            *slot = Some(e.to_string());
        }
    }

    /// The largest height the field takes in magnitude at its relief scale (WDEF-017): the elevation clamp times the scale plus the deepest incision.
    fn max_abs_height(&self) -> f64 {
        MAX_ELEVATION_M.max(-MIN_ELEVATION_M) * self.relief + DETAIL_DOWN_M
    }

    /// The ground elevation of the bake at a direction, without creases.
    pub fn base_elevation(&self, dir: Vec3) -> f64 {
        self.base_via(&self.src, dir)
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

    /// Half the width, in radians, of the slope stencil.
    fn delta(&self) -> f64 {
        SLOPE_CELLS * core::f64::consts::FRAC_PI_2 / f64::from(self.src.res().max(1))
    }

    /// Magnitude of the bake's gradient at a direction, in metres per radian. The gradient is the sum over the three
    /// coordinate axes `a` of the central difference of the elevation along the tangent part `t` of `a` times `t`
    /// (`sum t t^T` is the tangent projector), so it does not depend on a chosen tangent basis and is smooth in the direction.
    pub fn gradient(&self, d: Vec3) -> f64 {
        self.gradient_via(&self.src, d)
    }

    fn gradient_via(&self, src: &dyn LatticeSource, d: Vec3) -> f64 {
        let delta = self.delta();
        let mut g = Vec3::new(0.0, 0.0, 0.0);
        for a in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
            let t = a - d * a.dot(d);
            let rise = self.base_via(src, (d + t * delta).normalized()) - self.base_via(src, (d - t * delta).normalized());
            g = g + t * (rise / (2.0 * delta));
        }
        g.length()
    }
}

impl ErosionField<'_> {
    /// [`HeightField::height`] reading the lattice through `src`.
    fn height_via(&self, src: &dyn LatticeSource, dir: Vec3) -> f64 {
        let e = self.base_via(src, dir);
        let slope = self.gradient_via(src, dir) / self.radius_m;
        let c = carve(slope);
        // Where the slope factor is 1 the landform changes nothing (`fine_carve(1, L)` is 1), so it is not evaluated.
        let l = if c >= 1.0 { 0.0 } else { self.landform().at(dir) };
        e + detail_m(self.seed, dir, e, c, l)
    }
}

impl ErosionField<'_> {
    /// The height as version 1 made it (the landform taken as 0): for the cost comparison of the landform term (`examples/erosion_cost.rs`), not a field.
    #[doc(hidden)]
    pub fn height_without_landform(&self, dir: Vec3) -> f64 {
        let e = self.base_via(&self.src, dir);
        let slope = self.gradient_via(&self.src, dir) / self.radius_m;
        e + detail_m(self.seed, dir, e, carve(slope), 0.0)
    }
}

impl HeightField for ErosionField<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.height_via(&self.src, dir)
    }

    /// The lattice cells of the whole batch are read once, grouped by bake block (GEN-020): the same values, in the caller's order.
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        let footprint = |g: &dyn LatticeSource, d: Vec3| {
            self.base_via(g, d);
            self.gradient_via(g, d);
        };
        crate::global::eval_gathered(&self.src, dirs, stop, &footprint, &|s, d| self.height_via(s, d))
    }

    /// [`HeightField::heights_until`] with the distinct blocks of the batch read on `threads` threads (GEN-020, LOD-009).
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        let footprint = |g: &dyn LatticeSource, d: Vec3| {
            self.base_via(g, d);
            self.gradient_via(g, d);
        };
        crate::global::eval_gathered_threads(&self.src, dirs, stop, threads, &footprint, &|s, d| self.height_via(s, d))
    }

    /// The climate of the tile from the bake with the integer reader, as the terrain field (GEN-021); a failed read is recorded like a
    /// height's and gives the zero texture.
    fn climate(&self, _map: &dyn planet_core::cube::FaceMapping, id: planet_core::TileId) -> crate::climate::ClimateTexture {
        match crate::climate::climate_tile(&self.src, id) {
            Ok(c) => c,
            Err(e) => {
                self.fail(&e);
                crate::climate::ClimateTexture::default()
            }
        }
    }

    fn climate_version_word(&self) -> u64 {
        crate::climate::climate_version_word()
    }

    /// The heights and the climate of a tile in one gather: the cells of both are read once, grouped by bake block (GEN-020), so a
    /// tile decodes each block it touches once however small the cache.
    fn heights_and_climate(
        &self,
        _map: &dyn planet_core::cube::FaceMapping,
        id: planet_core::TileId,
        dirs: &[Vec3],
        stop: &dyn Fn() -> bool,
    ) -> (Vec<f64>, crate::climate::ClimateTexture) {
        let footprint = |g: &dyn LatticeSource, d: Vec3| {
            self.base_via(g, d);
            self.gradient_via(g, d);
        };
        let climate = |s: &dyn LatticeSource| match crate::climate::climate_tile_direct(s, id) {
            Ok(c) => c,
            Err(e) => {
                self.fail(&e);
                crate::climate::ClimateTexture::default()
            }
        };
        crate::global::eval_gathered_with(&self.src, dirs, stop, &footprint, &|s, d| self.height_via(s, d), &climate)
    }

    fn seed(&self) -> u64 {
        self.seed
    }

    fn implementation_id(&self) -> &'static str {
        EROSION_IMPLEMENTATION_ID
    }

    fn version(&self) -> u32 {
        EROSION_VERSION
    }

    fn max_abs_height_m(&self) -> f64 {
        self.max_abs_height()
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

    /// The bounds of nested caps nest (LOD-012; `tests/bounds_monotone.rs`): the scan range, the slope and amplitude intervals and the noise
    /// bounds are each monotone in the cap, and a cap too wide takes the global range, which contains every other.
    fn bounds_monotone(&self) -> bool {
        true
    }

    /// Bounds over a cap (GEN-017). The bake's range over the cap is the terrain field's. The slope at a cap point differs
    /// from the slope at the centre by at most `delta_g` (below), so the slope factor lies in an interval, the amplitude in
    /// another (it grows with the elevation and the slope factor), the crease sum in a third (from the per-octave noise
    /// bounds), and the detail is the product of the last two. A cap too wide for the scan takes the global range.
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let global = (-self.max_abs_height(), self.max_abs_height());
        let base = match scan_base(&self.src, centre, angular_radius) {
            Ok(Some(s)) => s,
            Ok(None) => return global,
            Err(e) => {
                self.fail(&e);
                return global;
            }
        };
        let (blo, bhi) = (base.lo, base.hi);
        let delta = self.delta();
        // The slope interval. A stencil point of a cap point lies within `K r` of the stencil point of the centre (the map
        // `x -> normalise(x + delta t(x))` is `K`-Lipschitz), and both within `r + delta` of the centre, where the bake is
        // `lip`-Lipschitz in metres per radian (`res (du + dv)`: a lattice step per cell, `res / 2` cells per face
        // coordinate, at most 2 face coordinates per radian: ADR 0028 point 3).
        let k = (1.0 + 2.0 * delta) / (1.0 - delta);
        let (carve_lo, carve_hi) = if delta < 0.4 {
            match scan_base(&self.src, centre, (angular_radius + delta) * (1.0 + 1e-9)) {
                Ok(Some(wide)) => {
                    let lip = f64::from(self.src.res()) * (wide.max_du + wide.max_dv);
                    let g_centre = self.gradient(centre);
                    if g_centre.is_nan() {
                        return global;
                    }
                    // The 2.5 (the exact worst case of the sum is 2.45) also absorbs the gap between chord and arc at a very fine
                    // bake: do not tighten it without redoing that argument.
                    // Sum over the three axes of |t| <= 2.5; |t(d) - t(c)| <= 2 |d - c|; one f32 rounding of the bake
                    // (2.4e-7 of the largest elevation) in each of the four values a difference of two stencils spans.
                    let round = 2.0 * 2.4e-7 * MAX_ELEVATION_M.max(-MIN_ELEVATION_M) * self.relief;
                    let spread = (2.5 * lip * k * angular_radius * (1.0 / delta + 2.0) + 5.0 * round / delta) * (1.0 + 1e-9) + 1e-6;
                    let steepest = 2.5 * lip * k + 5.0 * round / delta;
                    let lo = (g_centre - spread).max(0.0);
                    let hi = (g_centre + spread).min(steepest.max(g_centre));
                    (carve(lo / self.radius_m), carve(hi / self.radius_m))
                }
                Ok(None) => (carve(0.0), 1.0),
                Err(e) => {
                    self.fail(&e);
                    return global;
                }
            }
        } else {
            (carve(0.0), 1.0)
        };
        // The landform over the cap: the range of the lattice cells it touches (GEN-062).
        let (l_lo, l_hi) = self.landform().bounds(centre, angular_radius);
        let (f_lo, f_hi) = (fine_carve(carve_lo, l_lo), fine_carve(carve_hi, l_hi));
        let (a_lo, a_hi) = (amplitude_m(blo, carve_lo), amplitude_m(bhi, carve_hi));
        let (fa_lo, fa_hi) = (amplitude_m(blo, f_lo), amplitude_m(bhi, f_hi));
        // The two parts of the crease sum, each from its octaves' noise bounds.
        let (mut clo_sum, mut chi_sum, mut flo_sum, mut fhi_sum, mut amp, mut norm, mut freq) =
            (0.0, 0.0, 0.0, 0.0, 1.0, 0.0, BASE_FREQUENCY);
        for o in 0..OCTAVES {
            let (nlo, nhi) = layer_noise_bounds(self.seed, LAYER_CREASE, o, centre, angular_radius, freq);
            let (clo, chi) = crease_range(nlo.max(-1.0), nhi.min(1.0));
            if o < CREASE_SPLIT {
                clo_sum += amp * clo;
                chi_sum += amp * chi;
            } else {
                flo_sum += amp * clo;
                fhi_sum += amp * chi;
            }
            norm += amp;
            amp *= PERSISTENCE;
            freq *= 2.0;
        }
        let (clo, chi) = (clo_sum / norm - 1e-9, chi_sum / norm + 1e-9);
        let (flo, fhi) = (flo_sum / norm - 1e-9, fhi_sum / norm + 1e-9);
        let part = |amp_lo: f64, amp_hi: f64, lo: f64, hi: f64| {
            (if lo >= 0.0 { amp_lo * lo } else { amp_hi * lo }, if hi >= 0.0 { amp_hi * hi } else { amp_lo * hi })
        };
        let ((c1, c2), (f1, f2)) = (part(a_lo, a_hi, clo, chi), part(fa_lo, fa_hi, flo, fhi));
        let (dlo, dhi) = ((c1 + f1).max(-0.75 * AMPLITUDE_MAX_M), (c2 + f2).min(0.25 * AMPLITUDE_MAX_M));
        let pad = |x: f64| 1e-6 + x.abs() * 1.2e-7;
        let (l, h) = (blo + dlo, bhi + dhi);
        ((l - pad(l)).max(global.0), (h + pad(h)).min(global.1))
    }
}

/// Builds the [`ErosionField`] of a body from a definition and a baked grid.
#[derive(Clone, Copy, Debug, Default)]
pub struct ErosionGenerator;

impl ErosionGenerator {
    pub fn implementation_id(&self) -> &'static str {
        EROSION_IMPLEMENTATION_ID
    }

    pub fn version(&self) -> u32 {
        EROSION_VERSION
    }

    /// The field of `body` over `bake`: the seed and the radius (the slope is rise over run) are read through `ctx`, so a
    /// different seed or radius re-keys the field. `bake_key` is the 128-bit stage key of the bake (STRM-011): it enters the
    /// field's `inputs_hash` and is the source of its persistent height key. `ctx` must be fresh.
    pub fn field<'a>(
        &self,
        ctx: &GenContext,
        body: BodyId,
        bake: &'a dyn LatticeSource,
        bake_key: [u8; 16],
    ) -> Result<ErosionField<'a>, MissingNode> {
        assert!(ctx.reads().is_empty(), "a field is built from a fresh context: its key covers the whole read set of the context");
        let seed = ctx.body_seed(body)?;
        let relief = ctx.relief_scale(body)?;
        let radius_m = ctx.body_radius_m(body)?;
        let records = ctx.read_records();
        let half = |b: &[u8]| u64::from_le_bytes(b.try_into().expect("8 bytes"));
        let inputs = hash_u64s(
            0x6572_6f73,
            &[records_hash64(&records), half(&bake_key[..8]), half(&bake_key[8..]), u64::from(GLOBAL_VERSION), u64::from(EROSION_VERSION)],
        );
        Ok(ErosionField {
            seed,
            body,
            radius_m,
            inputs,
            records,
            bake_key,
            src: crate::global::ScaledLattice::new(bake, relief),
            relief,
            error: Mutex::new(None),
            landform: OnceLock::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::hash::SplitMix64;

    fn random_dir(rng: &mut SplitMix64) -> Vec3 {
        Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized()
    }

    // spec: GEN-061
    #[test]
    fn without_the_landform_the_detail_is_the_old_one_and_with_it_only_the_fine_creases_are_lifted() {
        let mut rng = SplitMix64(5);
        let (mut lifted, mut full) = (0, 0);
        for _ in 0..400 {
            let dir = random_dir(&mut rng);
            let e = rng.next_f64() * 3000.0 - 200.0;
            let c = carve(rng.next_f64() * 0.3);
            let (coarse, fine) = crease_parts(7, dir);
            // The two parts are the old normalised sum.
            let old = amplitude_m(e, c) * (coarse + fine);
            assert!((detail_m(7, dir, e, c, 0.0) - old).abs() < 1e-9 * (1.0 + old.abs()), "landform 0 is the old detail");
            // Landform 1: the fine creases at the full factor, the coarse ones unchanged.
            let full_fine = amplitude_m(e, c) * coarse + amplitude_m(e, 1.0) * fine;
            assert!((detail_m(7, dir, e, c, 1.0) - full_fine).abs() < 1e-9 * (1.0 + full_fine.abs()));
            if (detail_m(7, dir, e, c, 1.0) - old).abs() > 1.0 {
                lifted += 1;
            }
            if c >= 1.0 - 1e-12 {
                full += 1;
                assert!(
                    (detail_m(7, dir, e, c, 1.0) - old).abs() < 1e-9 * (1.0 + old.abs()),
                    "on the steepest ground the landform adds nothing"
                );
            }
        }
        assert!(lifted > 100, "{lifted} of 400 points change by over a metre");
        assert!(full > 20, "{full} of 400 points stand on the steepest ground");
    }

    // spec: GEN-062
    #[test]
    fn the_landform_lattice_is_continuous_bounded_and_close_to_the_driver() {
        let lat = crate::global::landform_lattice(7);
        let mut rng = SplitMix64(9);
        let (mut worst, mut worst_step, mut inside) = (0.0f64, 0.0f64, 0);
        for _ in 0..20_000 {
            let a = random_dir(&mut rng);
            let la = lat.at(a);
            assert!((0.0..=1.0).contains(&la));
            worst = worst.max((la - crate::global::landform(7, a)).abs());
            // Continuous: a pair 1e-6 apart differs by far less than a lattice step does (cube edges and corners included in the random sample).
            let b = (a + random_dir(&mut rng) * 1e-6).normalized();
            worst_step = worst_step.max((la - lat.at(b)).abs());
            // The bounds of a cap hold its centre and a point on its rim.
            let rho = [1e-6, 1e-4, 1e-3, 3e-3, 1e-2][(rng.next_f64() * 5.0) as usize % 5];
            let rim = (a + random_dir(&mut rng) * rho).normalized();
            let ang = libm::acos(a.dot(rim).clamp(-1.0, 1.0));
            let (lo, hi) = lat.bounds(a, ang * (1.0 + 1e-9));
            let lr = lat.at(rim);
            assert!(lo <= la && la <= hi && lo <= lr && lr <= hi, "cap of {ang} rad: [{lo}, {hi}] misses {la} or {lr}");
            inside += usize::from(hi - lo < 1.0);
        }
        assert!(worst < 0.03, "the lattice differs from the driver by {worst}");
        assert!(worst_step < 1e-3, "a pair 1e-6 apart differs by {worst_step}");
        // The seams: two points 1e-12 either side of a cube edge (x = y) and of a corner agree.
        for k in 0..200 {
            let w = -0.9 + 1.8 * f64::from(k) / 199.0;
            let (a, b) = (Vec3::new(1.0, 1.0 - 1e-12, w).normalized(), Vec3::new(1.0 - 1e-12, 1.0, w).normalized());
            assert!((lat.at(a) - lat.at(b)).abs() < 1e-9, "seam at z {w}");
        }
        let (c1, c2) = (Vec3::new(1.0, 1.0, 1.0 - 1e-12).normalized(), Vec3::new(1.0 - 1e-12, 1.0, 1.0).normalized());
        assert!((lat.at(c1) - lat.at(c2)).abs() < 1e-9, "corner seam");
        assert!(inside > 10_000, "{inside} caps with a range under [0, 1]");
    }
}
