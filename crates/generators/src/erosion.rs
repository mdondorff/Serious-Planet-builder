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

use crate::global::{
    read_direction, read_elevation_c1, read_elevation_c1_cached, LatticeSource, StencilCache, GLOBAL_VERSION, MAX_ELEVATION_M,
    MIN_ELEVATION_M, READER_C1_VERSION,
};
#[cfg(test)]
use crate::height::layer_noise;
use crate::height::{layer_noise_bounds_t, layer_noise_t, turned_noise, HeightField};
use crate::terrain::scan_base;

thread_local! {
    /// The C1 stencils this thread loaded (see [`read_elevation_c1_cached`]).
    static STENCILS: std::cell::RefCell<StencilCache> = std::cell::RefCell::new(StencilCache::new());
    /// The unpinned detail at the layer cell centres this thread read, for one field (its id): the pin's memo (GEN-187).
    static PIN_MEMO: std::cell::RefCell<(u64, std::collections::HashMap<(u8, u32, u32), f64>)> =
        std::cell::RefCell::new((0, std::collections::HashMap::new()));
}

/// The most centres a thread keeps in the pin's memo before it starts over (about 6 MB).
const PIN_MEMO_CAP: usize = 1 << 18;
static NEXT_FIELD_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Bump whenever any output of this module changes (ADR 0006); it also covers [`GLOBAL_VERSION`] through the bake key.
pub const EROSION_VERSION: u32 = 2;
/// Names this implementation in cache keys (CON-07).
pub const EROSION_IMPLEMENTATION_ID: &str = "cpu-erosion-v1";
/// Bump whenever any output of the C1 variant changes (it also covers [`READER_C1_VERSION`] through its key).
pub const EROSION_C1_VERSION: u32 = 1;
/// Names the C1 variant in cache keys (GEN-107): the same field with the bake read by `read_elevation_c1` instead of the bilinear blend.
pub const EROSION_C1_IMPLEMENTATION_ID: &str = "cpu-erosion-v2";

/// Names this implementation with the relief channels on (CON-07): another field, another key.
pub const EROSION_RELIEF_IMPLEMENTATION_ID: &str = "cpu-erosion-v1+relief";

/// Names the C1 variant with the relief channels on (CON-07): another field, another key.
pub const EROSION_C1_RELIEF_IMPLEMENTATION_ID: &str = "cpu-erosion-v2+relief";

/// Names the relief field with the crease band on (CON-07): the creases above the layer's grid dropped (GEN-109), another field, another key.
pub const EROSION_BAND_IMPLEMENTATION_ID: &str = "cpu-erosion-v1+relief+band";
/// [`EROSION_BAND_IMPLEMENTATION_ID`] over the C1 base.
pub const EROSION_C1_BAND_IMPLEMENTATION_ID: &str = "cpu-erosion-v2+relief+band";
/// Bump whenever the crease band's weights or the way they apply change (it enters the inputs hash of a field with the band on).
pub const CREASE_BAND_VERSION: u32 = 1;
/// The version of the detail pinned to the layer's samples (GEN-187, ADR 0122 point 1): it enters the keys of every field that reads the layer.
pub const PINNED_DETAIL_VERSION: u32 = 1;

/// How far, in layer cells of angle, the cell centres a pinned point reads lie from it: the reach the bounds and a batch's footprint widen by (GEN-187). A centre of the point's own
/// face with a weight lies less than one cell away along each face axis, and the arc of a step along either axis of `TangentWarp` is at most one cell per cell, so it lies within 2
/// cells; within the edge blend (2 cells of an edge) the neighbouring face's centres lie within the blend plus that, so 4.
const PIN_REACH_CELLS: f64 = 4.0;

/// The implementation id of a field that reads the erosion layer (GEN-112) over the v1 base.
pub const EROSION_LAYER_IMPLEMENTATION_ID: &str = "cpu-erosion-v1+relief+band+layer";
/// The crease band without the relief channels (`k` 1, `m` unread) over the v1 base (GEN-109).
pub const EROSION_BAND_ONLY_IMPLEMENTATION_ID: &str = "cpu-erosion-v1+band";
/// [`EROSION_BAND_ONLY_IMPLEMENTATION_ID`] over the C1 base.
pub const EROSION_C1_BAND_ONLY_IMPLEMENTATION_ID: &str = "cpu-erosion-v2+band";
/// The erosion layer without the relief channels over the v1 base (GEN-112).
pub const EROSION_LAYER_ONLY_IMPLEMENTATION_ID: &str = "cpu-erosion-v1+band+layer";
/// [`EROSION_LAYER_ONLY_IMPLEMENTATION_ID`] over the C1 base.
pub const EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID: &str = "cpu-erosion-v2+band+layer";
/// [`EROSION_LAYER_IMPLEMENTATION_ID`] over the C1 base.
pub const EROSION_C1_LAYER_IMPLEMENTATION_ID: &str = "cpu-erosion-v2+relief+band+layer";

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
/// How far from the cell of a direction the lattice reads of a height reach, in cells (GEN-020): the gradient stencil reaches [`SLOPE_CELLS`] cells (a bit
/// more on the chart, 2/sqrt(3) at most) and the bilinear read one more (the C1 read one below and two above: still within 8); a footprint taken from this hint is a superset of the exact one.
pub const GRADIENT_REACH_CELLS: u32 = 8;
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
#[cfg(test)]
fn crease_parts(seed: u64, dir: Vec3) -> (f64, f64) {
    crease_parts_weighted(seed, dir, None)
}

/// [`crease_parts`] with an optional weight per octave (the crease band, GEN-109): an octave of weight 0 is not evaluated, one of weight `w` adds `w` times its term, and the
/// normaliser stays the sum of every octave's amplitude, so a kept octave keeps its own amplitude. With `None`, or all weights 1, the bits are those of the plain sum.
fn crease_parts_weighted(seed: u64, dir: Vec3, weights: Option<&[f64; OCTAVES as usize]>) -> (f64, f64) {
    let turned = turned_noise();
    let (mut coarse, mut fine, mut amp, mut norm, mut freq) = (0.0, 0.0, 1.0, 0.0, BASE_FREQUENCY);
    for o in 0..OCTAVES {
        let w = weights.map_or(1.0, |w| w[o as usize]);
        if w == 0.0 {
            norm += amp;
            amp *= PERSISTENCE;
            freq *= 2.0;
            continue;
        }
        let term = if w == 1.0 {
            amp * crease(layer_noise_t(turned, seed, LAYER_CREASE, o, dir * freq))
        } else {
            w * amp * crease(layer_noise_t(turned, seed, LAYER_CREASE, o, dir * freq))
        };
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
    detail_m_weighted(seed, dir, e, carve, landform, None)
}

/// [`detail_m`] over the crease octaves of the band's weights (GEN-109).
fn detail_m_weighted(seed: u64, dir: Vec3, e: f64, carve: f64, landform: f64, weights: Option<&[f64; OCTAVES as usize]>) -> f64 {
    let (coarse, fine) = crease_parts_weighted(seed, dir, weights);
    amplitude_m(e, carve) * coarse + amplitude_m(e, fine_carve(carve, landform)) * fine
}

/// The crease band (GEN-109, ADR 0098): where the erosion layer is read, the layer alone shapes erosion at its scales, so the crease octaves whose wavelength is at most `near_cells`
/// of the layer's cells keep their weight, those of `far_cells` or more are dropped, and in between the weight falls by a smoothstep in the logarithm of the wavelength. An octave's
/// wavelength is its lattice cell, `radius / (BASE_FREQUENCY 2^o)` (about 25 km for octave 0, 12 m for octave 11). Only the ground at or above `sea_level_m` takes the band: the sea
/// floor keeps today's creases (the layer is carved on land).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CreaseBand {
    /// The layer's cell in metres (pass 2 of the bake: 101.8 m).
    pub layer_cell_m: f64,
    /// Octaves of at most this many layer cells keep their weight (2).
    pub near_cells: f64,
    /// Octaves of at least this many layer cells are dropped (4).
    pub far_cells: f64,
    /// The base elevation below which the creases stay as they are: the sea level of the definition.
    pub sea_level_m: f64,
    /// The height above the sea level over which the creases join: the detail is `(1 - s) plain + s banded` with `s` a smoothstep of the base elevation from the level to the level
    /// plus this (0: a hard step at the level, a cliff along every shoreline). Default 200 m.
    pub blend_m: f64,
}

impl CreaseBand {
    /// The band of the pass-2 cell of the bake: 101.8 m, 2 and 4 cells, over a sea at `sea_level_m`.
    pub fn of_the_bake(sea_level_m: f64) -> CreaseBand {
        CreaseBand { layer_cell_m: 101.8, near_cells: 2.0, far_cells: 4.0, sea_level_m, blend_m: 200.0 }
    }

    /// The share of the band's detail at a point of base elevation `e`: 0 below the sea level, 1 from the level plus the blend, a smoothstep between (1 at the level for a blend of 0).
    pub fn share(&self, e: f64) -> f64 {
        if e < self.sea_level_m {
            0.0
        } else if self.blend_m == 0.0 || e >= self.sea_level_m + self.blend_m {
            1.0
        } else {
            smoothstep(self.sea_level_m, self.sea_level_m + self.blend_m, e)
        }
    }

    /// The weight of an octave of lattice cell `wavelength_m`: 1 up to `near_cells` layer cells, 0 from `far_cells`, between `1 - smoothstep` of its place on the log scale.
    pub fn weight(&self, wavelength_m: f64) -> f64 {
        let (near, far) = (self.near_cells * self.layer_cell_m, self.far_cells * self.layer_cell_m);
        if wavelength_m <= near {
            return 1.0;
        }
        if wavelength_m >= far {
            return 0.0;
        }
        let t = libm::log2(wavelength_m / near) / libm::log2(far / near);
        1.0 - smoothstep(0.0, 1.0, t)
    }

    /// The weights of the field's octaves on a body of `radius_m`.
    fn octave_weights(&self, radius_m: f64) -> [f64; OCTAVES as usize] {
        let mut w = [0.0; OCTAVES as usize];
        for (o, slot) in w.iter_mut().enumerate() {
            *slot = self.weight(radius_m / (BASE_FREQUENCY * f64::from(1u32 << o)));
        }
        w
    }

    fn valid(&self) -> bool {
        self.layer_cell_m.is_finite()
            && self.layer_cell_m > 0.0
            && self.near_cells.is_finite()
            && self.near_cells > 0.0
            && self.far_cells.is_finite()
            && self.far_cells > self.near_cells
            && self.sea_level_m.is_finite()
            && self.blend_m.is_finite()
            && self.blend_m >= 0.0
    }
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
    /// Whether the base is read by the C1 interpolant ([`EROSION_C1_IMPLEMENTATION_ID`]).
    c1: bool,
    /// Names this field in the thread's stencil cache (one per built field).
    id: u64,
    /// The relief channels `k` and `m` (GEN-099): when present the height is `m + k (e - m) + k detail`, else today's `e + detail` (`k` 1, `m` not read).
    relief_channels: Option<Arc<crate::relief_lattice::ReliefLattice>>,
    /// The crease band and its octave weights (GEN-109), off unless [`ErosionField::with_crease_band`] is called.
    crease_band: Option<(CreaseBand, [f64; OCTAVES as usize])>,
    /// The erosion layer and the sea level its coast fade follows (GEN-112), off unless [`ErosionField::with_layer`] is called.
    layer: Option<(Arc<crate::erosion_layer::ErosionLayer>, f64)>,
    /// The baked channels the tile's driver block is filled from (GEN-119), off unless [`ErosionField::with_channels`] is called.
    channels: Option<Arc<dyn crate::channels::ChannelSource>>,
}

impl ErosionField<'_> {
    /// Turns the relief channels on (GEN-099, the m9-bake switch, off by default): the field reads `k` and `m` once at each point and its keys change (the channels' content
    /// hash, their resolution and the channels' version enter the inputs hash, and the implementation id changes). The channels are of the bake as it is, so the body's relief scale must be 1:
    /// an error naming the scale otherwise.
    pub fn with_relief(mut self, channels: Arc<crate::relief_lattice::ReliefLattice>) -> Result<Self, String> {
        if self.crease_band.is_some() {
            return Err("turn the relief channels on before the crease band: the keys follow the order of the calls".into());
        }
        if self.relief != 1.0 {
            return Err(format!("the relief channels are of the bake at relief scale 1, the body's is {}", self.relief));
        }
        self.inputs = hash_u64s(
            0x7265_6c66,
            &[self.inputs, channels.id(), channels.cells() as u64, u64::from(crate::relief_channels::RELIEF_CHANNELS_VERSION)],
        );
        self.relief_channels = Some(channels);
        Ok(self)
    }

    /// Turns the crease band on (GEN-109, ADR 0098): the creases above the layer's grid are dropped where the base is at or above the band's sea level. The band is what the runtime
    /// layer read calls when it attaches the layer: without this call the field is today's field, whatever the relief switch says. It works with or without the relief channels
    /// (without them `k` is 1 and `m` is not read) and needs a valid band, an error naming the problem otherwise; its parameters and [`CREASE_BAND_VERSION`] enter the keys and the implementation id changes.
    pub fn with_crease_band(mut self, band: CreaseBand) -> Result<Self, String> {
        if !band.valid() {
            return Err(format!("the crease band needs a positive layer cell and 0 < near < far cells, got {band:?}"));
        }
        self.inputs = hash_u64s(
            0x6372_6273,
            &[
                self.inputs,
                band.layer_cell_m.to_bits(),
                band.near_cells.to_bits(),
                band.far_cells.to_bits(),
                band.sea_level_m.to_bits(),
                band.blend_m.to_bits(),
                u64::from(CREASE_BAND_VERSION),
            ],
        );
        let weights = band.octave_weights(self.radius_m);
        self.crease_band = Some((band, weights));
        Ok(self)
    }

    /// Turns the erosion layer on (GEN-112 to GEN-114, ADR 0088 point 5, m9-bake PR 6): the height is the relief base plus the layer's change (read C1, [`ErosionLayer::change_at`]) faded to 0 within
    /// [`crate::erosion_layer::COAST_FADE_M`] above `sea_level_m`, and the crease band of the bake ([`CreaseBand::of_the_bake`] over the same sea level) is switched on with it (ADR 0098).
    /// It works with or without the relief channels (without them the base is the lattice read at `k` 1) and needs the band off (the layer switches it on), an error naming the problem otherwise; the layer's identity, the sea level and the read's version enter
    /// the keys, and the implementation id changes. Without the call the field is today's, bit for bit.
    pub fn with_layer(self, layer: Arc<crate::erosion_layer::ErosionLayer>, sea_level_m: f64) -> Result<Self, String> {
        if self.crease_band.is_some() {
            return Err("the erosion layer switches the crease band on itself: call with_layer without with_crease_band".into());
        }
        if !sea_level_m.is_finite() {
            return Err(format!("the erosion layer needs a finite sea level, got {sea_level_m}"));
        }
        let mut f = self.with_crease_band(CreaseBand::of_the_bake(sea_level_m))?;
        f.inputs = hash_u64s(
            0x6c61_7972,
            &[
                f.inputs,
                layer.id(),
                sea_level_m.to_bits(),
                crate::erosion_layer::COAST_FADE_M.to_bits(),
                u64::from(crate::erosion_layer::LAYER_READ_VERSION),
                u64::from(PINNED_DETAIL_VERSION),
            ],
        );
        f.layer = Some((layer, sea_level_m));
        Ok(f)
    }

    /// This field with the driver block of its tiles filled from the baked channels `src` (GEN-119): the deposited depth into the soil code, the drainage area into the moisture code. The heights, their keys
    /// and the climate are unchanged; the field's climate version word changes (and with it every mesh key), so a tile made without the channels is never mixed with one made with them.
    pub fn with_channels(mut self, src: Arc<dyn crate::channels::ChannelSource>) -> Self {
        self.channels = Some(src);
        self
    }

    /// Whether the erosion layer is on.
    pub fn layer_on(&self) -> bool {
        self.layer.is_some()
    }

    /// Whether the crease band is on.
    pub fn crease_band_on(&self) -> bool {
        self.crease_band.is_some()
    }

    /// Whether the relief channels are on.
    pub fn relief_on(&self) -> bool {
        self.relief_channels.is_some()
    }

    /// The first error reading the bake since the field was built, if any.
    pub fn error(&self) -> Option<String> {
        self.error.lock().expect("erosion error mutex").clone()
    }

    fn landform(&self) -> &crate::global::LandformLattice {
        self.landform.get_or_init(|| crate::global::landform_lattice(self.seed))
    }

    fn fail_text(&self, e: &str) {
        let mut slot = self.error.lock().expect("erosion error mutex");
        if slot.is_none() {
            *slot = Some(e.to_string());
        }
    }

    fn fail(&self, e: &crate::global::ReadError) {
        let mut slot = self.error.lock().expect("erosion error mutex");
        if slot.is_none() {
            *slot = Some(e.to_string());
        }
    }

    /// The largest height the field takes in magnitude at its relief scale (WDEF-017): the elevation clamp times the scale plus the deepest incision; with the relief channels on,
    /// `max|1 - k| max|m| + k_max` times that (`m + k (e - m) + k detail = (1 - k) m + k (e + detail)`).
    fn max_abs_height(&self) -> f64 {
        // The detail lies in [-0.75 A, 0.25 A]; pinned where the layer is read (GEN-187) it is the difference of two such values, within [-A, A].
        let detail = if self.layer.is_some() { AMPLITUDE_MAX_M } else { DETAIL_DOWN_M };
        let today = MAX_ELEVATION_M.max(-MIN_ELEVATION_M) * self.relief + detail;
        // The layer adds its change times a fade of at most 1: the largest magnitude of its planet-wide range, a sound bound.
        let layer = self.layer.as_ref().map_or(0.0, |(l, _)| {
            let (lo, hi) = l.global_range();
            lo.abs().max(hi.abs())
        });
        layer
            + match &self.relief_channels {
                None => today,
                // `m + k (e - m) + k detail = (1 - k) m + k (e + detail)`: with `|e + detail|` inside `today` and `k`, `m` inside the channels' planet range, `|1 - k| m + k today`.
                Some(r) => {
                    let g = r.global_range();
                    let (k, m) = (g[1].max(0.0), g[2].abs().max(g[3].abs()));
                    let one_minus_k = (1.0 - g[0]).abs().max((g[1] - 1.0).abs());
                    one_minus_k * m + k * today
                }
            }
    }

    /// The bounds of the bake's elevation read over a cap (the base alone, no creases): `None` where the scan declines (a cap too wide, or the C1 read below res 4). For tests.
    #[doc(hidden)]
    pub fn base_bounds(&self, centre: Vec3, angular_radius: f64) -> Option<(f64, f64)> {
        scan_base(&self.src, centre, angular_radius, self.c1).ok().flatten().map(|s| (s.lo, s.hi))
    }

    /// The fine-detail term at a direction as ADR 0098 sets it, before any pinning (GEN-187). For tests.
    #[doc(hidden)]
    pub fn detail_unpinned(&self, dir: Vec3) -> f64 {
        self.detail_via_c(&self.src, dir, false).1
    }

    /// The ground elevation of the bake at a direction, without creases.
    pub fn base_elevation(&self, dir: Vec3) -> f64 {
        self.base_via(&self.src, dir)
    }

    fn base_via(&self, src: &dyn LatticeSource, dir: Vec3) -> f64 {
        self.base_via_c(src, dir, false)
    }

    /// [`Self::base_via`], with `cached` the C1 stencils kept in the thread's cache (a batch revisits the cells of its neighbours; one read does not).
    fn base_via_c(&self, src: &dyn LatticeSource, dir: Vec3, cached: bool) -> f64 {
        let read = if self.c1 && cached {
            // One cache a thread: the same field's stencils are the same values from any source of the same bake.
            STENCILS.with(|c| read_elevation_c1_cached(src, &TangentWarp, dir, &mut c.borrow_mut(), self.id))
        } else if self.c1 {
            read_elevation_c1(src, &TangentWarp, dir)
        } else {
            read_direction(src, &TangentWarp, dir).map(|s| s.elevation_m)
        };
        match read {
            Ok(e) => f64::from(e),
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
        self.gradient_via_c(src, d, false)
    }

    fn gradient_via_c(&self, src: &dyn LatticeSource, d: Vec3, cached: bool) -> f64 {
        let delta = self.delta();
        let mut g = Vec3::new(0.0, 0.0, 0.0);
        for a in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
            let t = a - d * a.dot(d);
            let rise =
                self.base_via_c(src, (d + t * delta).normalized(), cached) - self.base_via_c(src, (d - t * delta).normalized(), cached);
            g = g + t * (rise / (2.0 * delta));
        }
        g.length()
    }
}

impl ErosionField<'_> {
    /// [`HeightField::height`] reading the lattice through `src`.
    fn height_via(&self, src: &dyn LatticeSource, dir: Vec3) -> f64 {
        self.height_via_c(src, dir, false)
    }

    fn height_via_c(&self, src: &dyn LatticeSource, dir: Vec3, cached: bool) -> f64 {
        let (e, detail) = self.detail_via_c(src, dir, cached);
        // GEN-187 (ADR 0122 point 1): where the layer is read the detail is pinned to the stored samples: minus its bilinear interpolant from the four
        // surrounding cell centres of the layer, so it is 0 at every centre and the ground there is the stored base plus the change.
        let detail = match &self.layer {
            None => detail,
            Some((layer, _)) => {
                let stencil = layer.centres_around(dir);
                if stencil.at_centre {
                    0.0
                } else {
                    let mut interpolant = 0.0;
                    for ((centre, w), cell) in stencil.centres.iter().zip(&stencil.cells) {
                        if *w > 0.0 {
                            // The centres' reads are in a gathered batch's footprint (`gather_footprint`). A centre's detail is a pure function of the field and
                            // the cell, so this thread keeps it for the points around it (a neighbouring sample reads the same four).
                            interpolant += w * self.centre_detail(src, *centre, *cell, cached);
                        }
                    }
                    detail - interpolant
                }
            }
        };
        self.compose(dir, e, detail)
    }

    /// The unpinned detail at a layer cell centre, from this thread's memo of the field's centres (GEN-187): the same value as evaluating it, bit for bit.
    fn centre_detail(&self, src: &dyn LatticeSource, centre: Vec3, cell: (u8, u32, u32), cached: bool) -> f64 {
        if let Some(v) = PIN_MEMO.with(|m| {
            let m = m.borrow();
            if m.0 == self.id {
                m.1.get(&cell).copied()
            } else {
                None
            }
        }) {
            return v;
        }
        let v = self.detail_via_c(src, centre, cached).1;
        // A failed read (NaN, recorded) is not kept: the next read tries again.
        if v.is_finite() {
            PIN_MEMO.with(|m| {
                let mut m = m.borrow_mut();
                if m.0 != self.id || m.1.len() >= PIN_MEMO_CAP {
                    m.0 = self.id;
                    m.1.clear();
                }
                m.1.insert(cell, v);
            });
        }
        v
    }

    /// The lattice reads of a height at `dir` for a gathered batch (GEN-020): the base and its gradient at the point and, where the detail is pinned (GEN-187), at the layer's
    /// cell centres the pin reads.
    fn gather_footprint(&self, g: &dyn LatticeSource, dir: Vec3) {
        self.base_via(g, dir);
        self.gradient_via(g, dir);
        if let Some((layer, _)) = &self.layer {
            let stencil = layer.centres_around(dir);
            if !stencil.at_centre {
                for (centre, w) in stencil.centres {
                    if w > 0.0 {
                        self.base_via(g, centre);
                        self.gradient_via(g, centre);
                    }
                }
            }
        }
    }

    /// The reach in bake cells of a height's reads around its point: the gradient's, and the pin's centres where the layer is read (GEN-187).
    fn gather_reach(&self) -> u32 {
        match &self.layer {
            None => GRADIENT_REACH_CELLS,
            Some((layer, _)) => {
                let pin = libm::ceil(PIN_REACH_CELLS * f64::from(self.src.res()) / layer.cells() as f64) as u32;
                GRADIENT_REACH_CELLS + pin
            }
        }
    }

    /// The base elevation at `dir` and the fine-detail term there as ADR 0098 sets it (the crease octaves, banded where the band is on).
    fn detail_via_c(&self, src: &dyn LatticeSource, dir: Vec3, cached: bool) -> (f64, f64) {
        let e = self.base_via_c(src, dir, cached);
        let slope = self.gradient_via_c(src, dir, cached) / self.radius_m;
        let c = carve(slope);
        // Where the slope factor is 1 the landform changes nothing (`fine_carve(1, L)` is 1), so it is not evaluated.
        let l = if c >= 1.0 { 0.0 } else { self.landform().at(dir) };
        let detail = match &self.crease_band {
            Some((band, weights)) => {
                let s = band.share(e);
                if s <= 0.0 {
                    detail_m(self.seed, dir, e, c, l)
                } else if s >= 1.0 {
                    detail_m_weighted(self.seed, dir, e, c, l, Some(weights))
                } else {
                    (1.0 - s) * detail_m(self.seed, dir, e, c, l) + s * detail_m_weighted(self.seed, dir, e, c, l, Some(weights))
                }
            }
            None => detail_m(self.seed, dir, e, c, l),
        };
        (e, detail)
    }

    /// The height from the base elevation `e` and the detail term: the relief base, then the layer's change.
    fn compose(&self, dir: Vec3, e: f64, detail: f64) -> f64 {
        let base = match &self.relief_channels {
            None => e + detail,
            // GEN-099: each term one read at the point.
            Some(r) => {
                let (k, m) = r.at(dir);
                m + k * (e - m) + k * detail
            }
        };
        match &self.layer {
            None => base,
            // GEN-112: the layer's change, faded at the coast, added after the relief base (not scaled by `k`).
            Some((layer, sea)) => match layer.change_at(dir) {
                Ok(change) => base + crate::erosion_layer::coast_fade(e, *sea) * change,
                Err(e) => {
                    self.fail_text(&e);
                    f64::NAN
                }
            },
        }
    }
}

impl HeightField for ErosionField<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.height_via(&self.src, dir)
    }

    /// The lattice cells of the whole batch are read once, grouped by bake block (GEN-020): the same values, in the caller's order.
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        let footprint = |g: &dyn LatticeSource, d: Vec3| self.gather_footprint(g, d);
        crate::global::eval_gathered(&self.src, dirs, stop, &footprint, Some(self.gather_reach()), &|s, d| self.height_via_c(s, d, true))
    }

    /// [`HeightField::heights_until`] with the distinct blocks of the batch read on `threads` threads (GEN-020, LOD-009).
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), par: &dyn planet_core::par::Par) -> Vec<f64> {
        let footprint = |g: &dyn LatticeSource, d: Vec3| self.gather_footprint(g, d);
        crate::global::eval_gathered_threads(&self.src, dirs, stop, par, &footprint, Some(self.gather_reach()), &|s, d| {
            self.height_via_c(s, d, true)
        })
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
        // The turned lattices of the creases (GEN-140) enter the word only while they are on.
        crate::height::turned_noise_word(match &self.channels {
            Some(c) => planet_core::hash::mix64(crate::climate::climate_version_word() ^ crate::drivers::channel_word(c.identity())),
            None => crate::climate::climate_version_word(),
        })
    }

    fn driver_channels(&self, dirs: &[Vec3]) -> Option<Vec<crate::drivers::ChannelSample>> {
        let src = self.channels.as_ref()?;
        // A failed read is recorded like a height's and leaves the texel without a channel (the neutral value).
        let ok = |r: Result<Option<f32>, String>| match r {
            Ok(v) => v,
            Err(e) => {
                self.fail_text(&e);
                None
            }
        };
        Some(
            dirs.iter()
                .map(|&d| crate::drivers::ChannelSample {
                    soil_m: ok(src.deposited_depth_m(d)),
                    area_log2_cells: ok(src.area_log2_cells(d)),
                    fill_m: ok(src.fill_depth_m(d)),
                    erosion_m: ok(src.erosion_depth_m(d)),
                })
                .collect(),
        )
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
        let footprint = |g: &dyn LatticeSource, d: Vec3| self.gather_footprint(g, d);
        let climate = |s: &dyn LatticeSource| match crate::climate::climate_tile_direct(s, id) {
            Ok(c) => c,
            Err(e) => {
                self.fail(&e);
                crate::climate::ClimateTexture::default()
            }
        };
        crate::global::eval_gathered_with(
            &self.src,
            dirs,
            stop,
            &footprint,
            Some(self.gather_reach()),
            &|s, d| self.height_via_c(s, d, true),
            &climate,
        )
    }

    fn seed(&self) -> u64 {
        self.seed
    }

    fn implementation_id(&self) -> &'static str {
        match (self.relief_channels.is_some(), self.c1) {
            (false, true) if self.layer.is_some() => EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID,
            (false, false) if self.layer.is_some() => EROSION_LAYER_ONLY_IMPLEMENTATION_ID,
            (false, true) if self.crease_band.is_some() => EROSION_C1_BAND_ONLY_IMPLEMENTATION_ID,
            (false, false) if self.crease_band.is_some() => EROSION_BAND_ONLY_IMPLEMENTATION_ID,
            (true, true) if self.layer.is_some() => EROSION_C1_LAYER_IMPLEMENTATION_ID,
            (true, false) if self.layer.is_some() => EROSION_LAYER_IMPLEMENTATION_ID,
            (true, true) if self.crease_band.is_some() => EROSION_C1_BAND_IMPLEMENTATION_ID,
            (true, false) if self.crease_band.is_some() => EROSION_BAND_IMPLEMENTATION_ID,
            (true, true) => EROSION_C1_RELIEF_IMPLEMENTATION_ID,
            (true, false) => EROSION_RELIEF_IMPLEMENTATION_ID,
            (false, true) => EROSION_C1_IMPLEMENTATION_ID,
            (false, false) => EROSION_IMPLEMENTATION_ID,
        }
    }

    fn version(&self) -> u32 {
        if self.c1 {
            EROSION_C1_VERSION
        } else {
            EROSION_VERSION
        }
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
    /// bounds), and the detail is the product of the last two. A cap too wide for the scan takes the global range. With the relief channels on, the base is `m + k (e - m) + k detail`
    /// over the intervals of the four terms (the range of the channels over the cap from [`crate::relief_lattice::ReliefLattice::range_over_cap`]).
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let global = (-self.max_abs_height(), self.max_abs_height());
        let Some((blo, bhi, dlo, dhi)) = self.base_and_detail_range(centre, angular_radius) else {
            return global;
        };
        // GEN-187: where the layer is read the detail is pinned: the detail at the point minus its interpolant from the layer's cell centres around it, which lie within
        // `PIN_REACH_CELLS` cells of the point; the interpolant is a convex combination of the detail over the cap widened by that reach.
        let (dlo, dhi) = match &self.layer {
            None => (dlo, dhi),
            Some((layer, _)) => {
                let reach = PIN_REACH_CELLS * core::f64::consts::FRAC_PI_2 / layer.cells() as f64;
                let (wlo, whi) = match self.base_and_detail_range(centre, (angular_radius + reach) * (1.0 + 1e-9)) {
                    Some((_, _, wlo, whi)) => (wlo, whi),
                    None => (-DETAIL_DOWN_M, 0.25 * AMPLITUDE_MAX_M),
                };
                (dlo - whi, dhi - wlo)
            }
        };
        let pad = |x: f64| 1e-6 + x.abs() * 1.2e-7;
        let (l, h) = match &self.relief_channels {
            None => (blo + dlo, bhi + dhi),
            // `m + k (e - m) + k detail` over the intervals of `e`, `m`, `k` and the detail: the products of intervals (GEN-099, GEN-102).
            Some(r) => {
                let [klo, khi, mlo, mhi] = r.range_over_cap(centre, angular_radius);
                let product = |alo: f64, ahi: f64, blo_: f64, bhi_: f64| {
                    let p = [alo * blo_, alo * bhi_, ahi * blo_, ahi * bhi_];
                    (p.iter().cloned().fold(f64::INFINITY, f64::min), p.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
                };
                let (rise_lo, rise_hi) = product(klo, khi, blo - mhi, bhi - mlo);
                let (det_lo, det_hi) = product(klo, khi, dlo, dhi);
                (mlo + rise_lo + det_lo, mhi + rise_hi + det_hi)
            }
        };
        // GEN-102, GEN-112: the layer's change over the cap (from its pyramid, or the global range for a cap too wide) times the fade over the cap's base interval.
        let (l, h) = match &self.layer {
            None => (l, h),
            Some((layer, sea)) => {
                let (clo, chi) = match layer.range_over_cap(centre, angular_radius) {
                    Ok(Some(r)) => r,
                    Ok(None) => layer.global_range(),
                    Err(e) => {
                        self.fail_text(&e);
                        return global;
                    }
                };
                let (flo, fhi) = (crate::erosion_layer::coast_fade(blo, *sea), crate::erosion_layer::coast_fade(bhi, *sea));
                let p = [flo * clo, flo * chi, fhi * clo, fhi * chi];
                (l + p.iter().cloned().fold(f64::INFINITY, f64::min), h + p.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
            }
        };
        ((l - pad(l)).max(global.0), (h + pad(h)).min(global.1))
    }
}

impl ErosionField<'_> {
    /// The base range `(blo, bhi)` over a cap and the range `(dlo, dhi)` of the unpinned detail term over it (the parts of [`HeightField::height_bounds`] before the relief channels and the
    /// layer), or `None` where the cap takes the global range (a cap too wide for the scan, or a read error, recorded).
    fn base_and_detail_range(&self, centre: Vec3, angular_radius: f64) -> Option<(f64, f64, f64, f64)> {
        let base = match scan_base(&self.src, centre, angular_radius, self.c1) {
            Ok(Some(s)) => s,
            Ok(None) => return None,
            Err(e) => {
                self.fail(&e);
                return None;
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
            match scan_base(&self.src, centre, (angular_radius + delta) * (1.0 + 1e-9), self.c1) {
                Ok(Some(wide)) => {
                    let lip = f64::from(self.src.res()) * (wide.max_du + wide.max_dv);
                    let g_centre = self.gradient(centre);
                    if g_centre.is_nan() {
                        return None;
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
                    return None;
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
        // The two parts of the crease sum, each from its octaves' noise bounds (over the band's octaves and weights when the band applies).
        let turned = turned_noise();
        let crease_bounds = |weights: Option<&[f64; OCTAVES as usize]>| {
            let (mut clo_sum, mut chi_sum, mut flo_sum, mut fhi_sum, mut amp, mut norm, mut freq) =
                (0.0, 0.0, 0.0, 0.0, 1.0, 0.0, BASE_FREQUENCY);
            for o in 0..OCTAVES {
                let w = weights.map_or(1.0, |w| w[o as usize]);
                if w != 0.0 {
                    let (nlo, nhi) = layer_noise_bounds_t(turned, self.seed, LAYER_CREASE, o, centre, angular_radius, freq);
                    let (clo, chi) = crease_range(nlo.max(-1.0), nhi.min(1.0));
                    if o < CREASE_SPLIT {
                        clo_sum += w * amp * clo;
                        chi_sum += w * amp * chi;
                    } else {
                        flo_sum += w * amp * clo;
                        fhi_sum += w * amp * chi;
                    }
                }
                norm += amp;
                amp *= PERSISTENCE;
                freq *= 2.0;
            }
            ((clo_sum / norm - 1e-9, chi_sum / norm + 1e-9), (flo_sum / norm - 1e-9, fhi_sum / norm + 1e-9))
        };
        let part = |amp_lo: f64, amp_hi: f64, lo: f64, hi: f64| {
            (if lo >= 0.0 { amp_lo * lo } else { amp_hi * lo }, if hi >= 0.0 { amp_hi * hi } else { amp_lo * hi })
        };
        let detail_range = |weights: Option<&[f64; OCTAVES as usize]>| {
            let ((clo, chi), (flo, fhi)) = crease_bounds(weights);
            let ((c1, c2), (f1, f2)) = (part(a_lo, a_hi, clo, chi), part(fa_lo, fa_hi, flo, fhi));
            ((c1 + f1).max(-0.75 * AMPLITUDE_MAX_M), (c2 + f2).min(0.25 * AMPLITUDE_MAX_M))
        };
        let (dlo, dhi) = match &self.crease_band {
            // Every point of the cap is below the band's sea level (today's creases), or at or above the level plus the blend (the band's), or the cap reaches the blend zone: the
            // blended detail is a convex combination of the two, so the union holds it.
            Some((band, weights)) => {
                if bhi < band.sea_level_m {
                    detail_range(None)
                } else if blo >= band.sea_level_m + band.blend_m {
                    detail_range(Some(weights))
                } else {
                    let (a, b) = (detail_range(None), detail_range(Some(weights)));
                    (a.0.min(b.0), a.1.max(b.1))
                }
            }
            None => detail_range(None),
        };
        Some((blo, bhi, dlo, dhi))
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
        self.field_with(ctx, body, bake, bake_key, false)
    }

    /// [`ErosionGenerator::field`] with the bake read by the C1 interpolant (GEN-107): `cpu-erosion-v2`.
    pub fn field_c1<'a>(
        &self,
        ctx: &GenContext,
        body: BodyId,
        bake: &'a dyn LatticeSource,
        bake_key: [u8; 16],
    ) -> Result<ErosionField<'a>, MissingNode> {
        self.field_with(ctx, body, bake, bake_key, true)
    }

    fn field_with<'a>(
        &self,
        ctx: &GenContext,
        body: BodyId,
        bake: &'a dyn LatticeSource,
        bake_key: [u8; 16],
        c1: bool,
    ) -> Result<ErosionField<'a>, MissingNode> {
        assert!(ctx.reads().is_empty(), "a field is built from a fresh context: its key covers the whole read set of the context");
        let seed = ctx.body_seed(body)?;
        let relief = ctx.relief_scale(body)?;
        let radius_m = ctx.body_radius_m(body)?;
        let records = ctx.read_records();
        let half = |b: &[u8]| u64::from_le_bytes(b.try_into().expect("8 bytes"));
        let inputs = if c1 {
            hash_u64s(
                0x6572_6f74,
                &[
                    records_hash64(&records),
                    half(&bake_key[..8]),
                    half(&bake_key[8..]),
                    u64::from(GLOBAL_VERSION),
                    u64::from(EROSION_VERSION),
                    u64::from(EROSION_C1_VERSION),
                    u64::from(READER_C1_VERSION),
                ],
            )
        } else {
            hash_u64s(
                0x6572_6f73,
                &[
                    records_hash64(&records),
                    half(&bake_key[..8]),
                    half(&bake_key[8..]),
                    u64::from(GLOBAL_VERSION),
                    u64::from(EROSION_VERSION),
                ],
            )
        };
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
            c1,
            id: NEXT_FIELD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            relief_channels: None,
            crease_band: None,
            layer: None,
            channels: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::hash::SplitMix64;

    const EARTH_R: f64 = 6_371_000.0;

    // spec: GEN-109
    #[test]
    fn the_band_weights_are_one_up_to_two_cells_zero_from_four_and_half_at_the_geometric_mean() {
        let b = CreaseBand::of_the_bake(0.0);
        let c = b.layer_cell_m;
        assert_eq!((b.weight(0.5 * c), b.weight(2.0 * c), b.weight(4.0 * c), b.weight(10.0 * c)), (1.0, 1.0, 0.0, 0.0));
        let mid = b.weight(2.0 * core::f64::consts::SQRT_2 * c);
        assert!((mid - 0.5).abs() < 1e-12, "the geometric mean of 2 and 4 cells weighs {mid}");
        // Non-increasing, in [0, 1], and continuous across the two ends.
        let mut prev = 1.0;
        for k in 0..=2000 {
            let lambda = 2.0 * c * libm::pow(2.0, f64::from(k) / 1000.0);
            let w = b.weight(lambda);
            assert!((0.0..=1.0).contains(&w) && w <= prev + 1e-15, "weight {w} after {prev} at {lambda} m");
            prev = w;
        }
        assert!(b.weight(2.0 * c * 1.000_001) > 0.999_999 && b.weight(4.0 * c * 0.999_999) < 1e-6);
        // The octaves of the Earth: 24.9 km, halving; the 194.5 m octave keeps its weight, the 389 m one is nearly gone, the coarser ones are dropped, the finer ones kept.
        let w = b.octave_weights(EARTH_R);
        assert!(w[..6].iter().all(|x| *x == 0.0), "octaves 0 to 5 (780 m and more): {w:?}");
        assert!(w[6] > 0.0 && w[6] < 0.02, "octave 6 (389 m) is nearly gone: {}", w[6]);
        assert!(w[7..].iter().all(|x| *x == 1.0), "octaves 7 to 11 (195 m and less) keep their weight: {w:?}");
    }

    // spec: GEN-109
    #[test]
    fn weights_of_one_are_the_plain_sum_to_the_bit_and_dropped_octaves_are_removed_exactly() {
        let ones = [1.0; OCTAVES as usize];
        let mut rng = SplitMix64(109);
        let mut weights = [0.0; OCTAVES as usize];
        weights[6] = 0.25;
        weights[7] = 1.0;
        weights[8] = 0.5;
        weights[9] = 1.0;
        weights[10] = 1.0;
        weights[11] = 1.0;
        for _ in 0..200 {
            let d = random_dir(&mut rng);
            let (c0, f0) = crease_parts(11, d);
            let (c1, f1) = crease_parts_weighted(11, d, Some(&ones));
            assert_eq!((c0.to_bits(), f0.to_bits()), (c1.to_bits(), f1.to_bits()), "all weights 1 are the plain sum");
            let (c2, f2) = crease_parts_weighted(11, d, None);
            assert_eq!((c0.to_bits(), f0.to_bits()), (c2.to_bits(), f2.to_bits()));
            // The reference: the same octaves summed by hand with their weights over the full normaliser.
            let (mut coarse, mut fine, mut amp, mut norm, mut freq) = (0.0, 0.0, 1.0, 0.0, BASE_FREQUENCY);
            for o in 0..OCTAVES {
                let term = weights[o as usize] * amp * crease(layer_noise(11, LAYER_CREASE, o, d * freq));
                if o < CREASE_SPLIT {
                    coarse += term;
                } else {
                    fine += term;
                }
                norm += amp;
                amp *= PERSISTENCE;
                freq *= 2.0;
            }
            let (cb, fb) = crease_parts_weighted(11, d, Some(&weights));
            assert!(
                (cb - coarse / norm).abs() < 1e-15 && (fb - fine / norm).abs() < 1e-15,
                "the band's sum is the weighted octaves over the full normaliser"
            );
            // The dropped coarse octaves leave nothing in the coarse part, and a kept octave has the amplitude it had.
            assert_eq!(cb, 0.0, "every coarse octave is dropped");
            let kept_octave_10 = {
                let w10 = {
                    let mut w = [0.0; OCTAVES as usize];
                    w[10] = 1.0;
                    w
                };
                crease_parts_weighted(11, d, Some(&w10)).1
            };
            let (mut amp10, mut norm_all) = (1.0, 0.0);
            for o in 0..OCTAVES {
                if o == 10 {
                    amp10 = libm::pow(PERSISTENCE, 10.0);
                }
                norm_all += libm::pow(PERSISTENCE, f64::from(o));
            }
            let want = amp10 * crease(layer_noise(11, LAYER_CREASE, 10, d * (BASE_FREQUENCY * 1024.0))) / norm_all;
            assert!((kept_octave_10 - want).abs() < 1e-14, "octave 10 alone: {kept_octave_10} against {want}");
        }
    }

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
