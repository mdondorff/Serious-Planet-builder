//! The drivers of a chunk (GEN-070, ADR 0076, SPIKE-30): physical fields made from the final raster of a chunk, one value per core cell.
//!
//! Slope, curvature, wetness, soil depth and insolation are pure functions of the cell, its eight neighbours, its drainage area and its position.
//! `libm` for every transcendental, no threads, a fixed order (CON-14); the position terms read the cell's f64 unit direction (CON-11).

use crate::height::layer_noise;
use planet_core::Vec3;

/// Domain tag of the lithology noise: the lattices of the drivers never alias those of another layer.
pub const LAYER_LITHOLOGY: u64 = 0x4452_4c49_5448;

/// The constants of the drivers (part of the stage's definition hash).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DriverParams {
    /// `tan s` is not taken below this in the wetness (flats and carved beds): 0.01 is 0.57 degrees.
    pub tan_floor: f64,
    /// The drainage area in m2 is not taken above this in the wetness (a river cell is as wet as a cell can be).
    pub area_clamp_m2: f64,
    /// The moisture is `clamp((W - moisture_lo) / moisture_span, 0, 1)` of the wetness index `W`.
    pub moisture_lo: f64,
    pub moisture_span: f64,
    /// The soil depth: the base in metres, the slope scale in degrees, the curvature scale in metres (the factor is `1 + 0.5 tanh(scale kappa)`), the area
    /// range in m2 over which the flow factor goes from 0.5 to 1, the altitude range in metres over which the soil falls to 0, and the clamp in metres.
    pub soil_base_m: f64,
    pub soil_slope_deg: f64,
    pub soil_curvature_m: f64,
    pub flow_area_lo_m2: f64,
    pub flow_area_hi_m2: f64,
    pub soil_alt_lo_m: f64,
    pub soil_alt_hi_m: f64,
    pub soil_max_m: f64,
    /// The lithology noise: its frequency per unit of direction (a wavelength of about 16 km on an Earth-sized body) and the seed it is read with.
    pub litho_freq: f64,
    pub seed: u64,
}

impl Default for DriverParams {
    fn default() -> DriverParams {
        DriverParams {
            tan_floor: 0.01,
            area_clamp_m2: 1.0e8,
            moisture_lo: 4.0,
            moisture_span: 12.0,
            soil_base_m: 2.5,
            soil_slope_deg: 18.0,
            soil_curvature_m: 100.0,
            flow_area_lo_m2: 1.0e2,
            flow_area_hi_m2: 1.0e8,
            soil_alt_lo_m: 2500.0,
            soil_alt_hi_m: 4500.0,
            soil_max_m: 8.0,
            litho_freq: 400.0,
            seed: 0,
        }
    }
}

impl DriverParams {
    /// The constants as words for a definition hash.
    pub fn hash_words(&self) -> [u64; 14] {
        [
            self.tan_floor.to_bits(),
            self.area_clamp_m2.to_bits(),
            self.moisture_lo.to_bits(),
            self.moisture_span.to_bits(),
            self.soil_base_m.to_bits(),
            self.soil_slope_deg.to_bits(),
            self.soil_curvature_m.to_bits(),
            self.flow_area_lo_m2.to_bits(),
            self.flow_area_hi_m2.to_bits(),
            self.soil_alt_lo_m.to_bits(),
            self.soil_alt_hi_m.to_bits(),
            self.soil_max_m.to_bits(),
            self.litho_freq.to_bits(),
            self.seed,
        ]
    }
}

/// The slope (rise over run) components `(p, q)` along the raster's `i` and `j` axes and the mean curvature `kappa` in 1/m (positive in a hollow, negative
/// on a ridge) at the centre of a 3 x 3 stencil `z[(dj + 1) * 3 + (di + 1)]` of heights at the spacings `si` and `sj` in metres along the two axes
/// (Zevenbergen and Thorne). The axes are taken as orthogonal; near a cube corner they meet at 60 degrees and the curvature is approximate.
pub fn gradient_and_curvature(z: &[f64; 9], si: f64, sj: f64) -> (f64, f64, f64) {
    let at = |di: usize, dj: usize| z[dj * 3 + di];
    let p = (at(2, 1) - at(0, 1)) / (2.0 * si);
    let q = (at(1, 2) - at(1, 0)) / (2.0 * sj);
    let r = (at(2, 1) - 2.0 * at(1, 1) + at(0, 1)) / (si * si);
    let t = (at(1, 2) - 2.0 * at(1, 1) + at(1, 0)) / (sj * sj);
    let s = (at(2, 2) - at(0, 2) - at(2, 0) + at(0, 0)) / (4.0 * si * sj);
    let g = 1.0 + p * p + q * q;
    let kappa = ((1.0 + q * q) * r - 2.0 * p * q * s + (1.0 + p * p) * t) / (2.0 * g * libm::sqrt(g));
    (p, q, kappa)
}

/// The wetness index `W = ln(A / (w tan s))` for the drainage area `area_m2`, the cell width `cell_m` and `tan s` (floored, the area clamped).
pub fn wetness_index(area_m2: f64, cell_m: f64, tan_s: f64, params: &DriverParams) -> f64 {
    let a = area_m2.clamp(cell_m * cell_m, params.area_clamp_m2.max(cell_m * cell_m));
    libm::log(a / (cell_m * libm::fmax(tan_s, params.tan_floor)))
}

/// The moisture in `[0, 1]` of a wetness index.
pub fn moisture(wetness: f64, params: &DriverParams) -> f64 {
    ((wetness - params.moisture_lo) / params.moisture_span).clamp(0.0, 1.0)
}

fn smoothstep(lo: f64, hi: f64, x: f64) -> f64 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The curvature factor of the soil depth: `1 + 0.5 tanh(scale kappa)`.
pub fn curvature_factor(kappa: f64, params: &DriverParams) -> f64 {
    1.0 + 0.5 * libm::tanh(params.soil_curvature_m * kappa)
}

/// The lithology term in `[-1, 1]` at the unit direction `dir` (f64: CON-11).
pub fn lithology(dir: Vec3, params: &DriverParams) -> f64 {
    layer_noise(params.seed, LAYER_LITHOLOGY, 0, dir * params.litho_freq)
}

/// The soil depth in metres (0 to `soil_max_m`) of a cell of slope `slope_rad`, curvature `kappa`, drainage area `area_m2`, lithology `litho` in `[-1, 1]`
/// and height `height_m` over the water level (the caller passes the height over the water). The product of the factors reaches 5.6 m at most with the
/// default constants, so the clamp is a guard for other constants.
pub fn soil_depth_m(slope_rad: f64, kappa: f64, area_m2: f64, litho: f64, height_m: f64, params: &DriverParams) -> f64 {
    let slope = libm::exp(-slope_rad.to_degrees() / params.soil_slope_deg);
    let flow = 0.5
        + 0.5
            * ((libm::log10(area_m2.max(1.0)) - libm::log10(params.flow_area_lo_m2))
                / (libm::log10(params.flow_area_hi_m2) - libm::log10(params.flow_area_lo_m2)))
            .clamp(0.0, 1.0);
    let altitude = 1.0 - smoothstep(params.soil_alt_lo_m, params.soil_alt_hi_m, height_m);
    (params.soil_base_m * slope * curvature_factor(kappa, params) * flow * (1.0 + 0.5 * litho) * altitude).clamp(0.0, params.soil_max_m)
}

/// The soil depth as 8 bits, square-root coded: `round(255 sqrt(depth / soil_max))`.
pub fn encode_soil(depth_m: f64, params: &DriverParams) -> u8 {
    libm::round(255.0 * libm::sqrt((depth_m / params.soil_max_m).clamp(0.0, 1.0))) as u8
}

/// The soil depth of an 8-bit code.
pub fn decode_soil(code: u8, params: &DriverParams) -> f64 {
    let u = f64::from(code) / 255.0;
    u * u * params.soil_max_m
}

/// A value in `[0, 1]` as 8 bits.
pub fn encode_unit(v: f64) -> u8 {
    libm::round(255.0 * v.clamp(0.0, 1.0)) as u8
}

/// The 8-bit code of a unit value.
pub fn decode_unit(code: u8) -> f64 {
    f64::from(code) / 255.0
}

/// The days of the year the insolation is integrated over: every fifth day from day 3 (73 days), and the steps of the day.
const DAY_STEP: usize = 5;
const DAYS: usize = 73;
const HOUR_STEPS: usize = 96;

/// The declination of the sun in radians on day `n` of the year (Cooper).
fn declination(n: f64) -> f64 {
    23.45_f64.to_radians() * libm::sin(core::f64::consts::TAU * (284.0 + n) / 365.0)
}

/// The sines and cosines of the declinations and the hour angles, made once for many evaluations.
struct SunGrid {
    days: Vec<(f64, f64)>,
    hours: Vec<(f64, f64)>,
}

impl SunGrid {
    fn new() -> SunGrid {
        let days = (0..DAYS)
            .map(|k| {
                let d = declination((3 + DAY_STEP * k) as f64);
                (libm::sin(d), libm::cos(d))
            })
            .collect();
        let hours = (0..HOUR_STEPS)
            .map(|h| {
                let w = core::f64::consts::TAU * (h as f64 + 0.5) / HOUR_STEPS as f64 - core::f64::consts::PI;
                (libm::sin(w), libm::cos(w))
            })
            .collect();
        SunGrid { days, hours }
    }

    /// The mean over the days and the hours of the beam irradiance on a plane of slope `slope_rad` and surface azimuth `azimuth_rad` (from north, clockwise:
    /// the direction the plane faces) at latitude `lat_rad`, in units of the flat plane (not normalised), with the sun above the horizon only and no
    /// shadowing by terrain: Duffie and Beckman's `cos theta = sin d (sin f cos b - cos f sin b cos g) + cos d cos w (cos f cos b + sin f sin b cos g)
    /// + cos d sin b sin g sin w` with `g` the azimuth from south (positive to the west).
    fn mean(&self, lat_rad: f64, slope_rad: f64, azimuth_rad: f64) -> f64 {
        let g = azimuth_rad - core::f64::consts::PI;
        let (sf, cf, sb, cb, sg, cg) =
            (libm::sin(lat_rad), libm::cos(lat_rad), libm::sin(slope_rad), libm::cos(slope_rad), libm::sin(g), libm::cos(g));
        let mut total = 0.0;
        for &(sd, cd) in &self.days {
            for &(sw, cw) in &self.hours {
                if sd * sf + cd * cf * cw <= 0.0 {
                    continue;
                }
                let c = sd * (sf * cb - cf * sb * cg) + cd * cw * (cf * cb + sf * sb * cg) + cd * sb * sg * sw;
                total += c.max(0.0);
            }
        }
        total / (DAYS * HOUR_STEPS) as f64
    }
}

/// The annual-mean irradiance on a tilted plane in units of the flat plane at the equator (see [`SunGrid::mean`]): 73 days, 24 steps of the day.
pub fn annual_mean_irradiance(lat_rad: f64, slope_rad: f64, azimuth_rad: f64) -> f64 {
    let grid = SunGrid::new();
    grid.mean(lat_rad, slope_rad, azimuth_rad) / grid.mean(0.0, 0.0, 0.0)
}

/// The slopes and aspects of the insolation table.
const TABLE_SLOPES: usize = 17;
const TABLE_ASPECTS: usize = 16;
/// The steepest slope of the table in degrees; a steeper cell reads the 60 degree value.
const TABLE_MAX_DEG: f64 = 60.0;

/// The annual-mean insolation of a chunk: a table over slope and aspect at the chunk centre's latitude, read bilinearly (the latitude changes by 0.04
/// degrees across a chunk, so two chunks may differ by one 8-bit code at their border).
pub struct InsolationTable {
    values: Vec<f64>,
}

impl InsolationTable {
    pub fn new(lat_rad: f64) -> InsolationTable {
        let grid = SunGrid::new();
        let flat = grid.mean(0.0, 0.0, 0.0);
        let mut values = Vec::with_capacity(TABLE_SLOPES * (TABLE_ASPECTS + 1));
        for s in 0..TABLE_SLOPES {
            let slope = (TABLE_MAX_DEG * s as f64 / (TABLE_SLOPES - 1) as f64).to_radians();
            for a in 0..=TABLE_ASPECTS {
                values.push(grid.mean(lat_rad, slope, core::f64::consts::TAU * a as f64 / TABLE_ASPECTS as f64) / flat);
            }
        }
        InsolationTable { values }
    }

    /// The insolation of a slope and an aspect (from north, clockwise), the slope clamped to the table's 60 degrees.
    pub fn at(&self, slope_rad: f64, azimuth_rad: f64) -> f64 {
        let s = (slope_rad.to_degrees() / TABLE_MAX_DEG * (TABLE_SLOPES - 1) as f64).clamp(0.0, (TABLE_SLOPES - 1) as f64);
        let turns = azimuth_rad / core::f64::consts::TAU;
        let a = (turns - libm::floor(turns)) * TABLE_ASPECTS as f64;
        let (s0, a0) = ((libm::floor(s) as usize).min(TABLE_SLOPES - 2), (libm::floor(a) as usize).min(TABLE_ASPECTS - 1));
        let (fs, fa) = (s - s0 as f64, a - a0 as f64);
        let v = |si: usize, ai: usize| self.values[si * (TABLE_ASPECTS + 1) + ai];
        let lo = v(s0, a0) * (1.0 - fa) + v(s0, a0 + 1) * fa;
        let hi = v(s0 + 1, a0) * (1.0 - fa) + v(s0 + 1, a0 + 1) * fa;
        lo * (1.0 - fs) + hi * fs
    }
}

/// The three driver channels of a chunk's core, 8 bits each, row-major over the core with `j` outer.
pub struct ChunkDrivers {
    pub soil: Vec<u8>,
    pub moisture: Vec<u8>,
    pub insolation: Vec<u8>,
}

/// What the drivers need of a chunk. `heights` is the `(n + 2) x (n + 2)` raster of the core and one ring (row-major, `j` outer, heights over the water),
/// `area_m2` the `n x n` drainage area of the core, `dirs` the unit directions of the core's cells, `spacing_m` the true spacing in metres of each core
/// cell along the raster's `i` and `j` axes (the lattice is equal-angle: the spacing is 0.7 of the face-centre cell at a face edge), `compass` for each
/// core cell the components `[ei, ej, ni, nj]` that turn the gradient `(p, q)` along the axes into its east and north parts (`ge = p ei + q ej`,
/// `gn = p ni + q nj`; the identity `[1, 0, 0, 1]` where the axes are east and north), and `lat_rad` the latitude of the chunk's centre.
pub struct ChunkRaster<'a> {
    pub n: usize,
    pub heights: &'a [f64],
    pub area_m2: &'a [f64],
    pub dirs: &'a [Vec3],
    pub spacing_m: &'a [[f64; 2]],
    pub compass: &'a [[f64; 4]],
    pub lat_rad: f64,
}

/// The drivers of every core cell of `chunk`. Heights or areas that are not finite, a cell wider than 10 km and constants out of order are refused
/// (a panic): the stage hands over finite rasters or does not run (GEN-070).
pub fn chunk_drivers(chunk: &ChunkRaster, params: &DriverParams) -> ChunkDrivers {
    let n = chunk.n;
    assert_eq!(chunk.heights.len(), (n + 2) * (n + 2));
    for len in [chunk.area_m2.len(), chunk.dirs.len(), chunk.spacing_m.len(), chunk.compass.len()] {
        assert_eq!(len, n * n);
    }
    assert!(chunk.heights.iter().all(|h| h.is_finite()), "a driver raster with a height that is not a number");
    assert!(chunk.area_m2.iter().all(|a| a.is_finite()), "a driver raster with an area that is not a number");
    assert!(chunk.spacing_m.iter().all(|s| s[0] > 0.0 && s[0] < 1.0e4 && s[1] > 0.0 && s[1] < 1.0e4), "a cell spacing out of range");
    assert!(params.soil_alt_hi_m > params.soil_alt_lo_m && params.flow_area_hi_m2 > params.flow_area_lo_m2 && params.moisture_span > 0.0);
    let table = InsolationTable::new(chunk.lat_rad);
    let mut out =
        ChunkDrivers { soil: Vec::with_capacity(n * n), moisture: Vec::with_capacity(n * n), insolation: Vec::with_capacity(n * n) };
    for j in 0..n {
        for i in 0..n {
            let mut z = [0.0; 9];
            for dj in 0..3 {
                for di in 0..3 {
                    z[dj * 3 + di] = chunk.heights[(j + dj) * (n + 2) + (i + di)];
                }
            }
            let cell = j * n + i;
            let [si, sj] = chunk.spacing_m[cell];
            let (p, q, kappa) = gradient_and_curvature(&z, si, sj);
            // The downhill direction on the compass: the gradient turned to (east, north); the plane faces down the gradient. The slope is the true one.
            let [ei, ej, ni, nj] = chunk.compass[cell];
            let (ge, gn) = (p * ei + q * ej, p * ni + q * nj);
            let tan_s = libm::sqrt(ge * ge + gn * gn);
            let slope = libm::atan(tan_s);
            let azimuth = if ge == 0.0 && gn == 0.0 { 0.0 } else { libm::atan2(-ge, -gn) };
            let area = chunk.area_m2[cell];
            let wet = wetness_index(area, libm::sqrt(si * sj), tan_s, params);
            let depth = soil_depth_m(slope, kappa, area, lithology(chunk.dirs[cell], params), z[4], params);
            out.soil.push(encode_soil(depth, params));
            out.moisture.push(encode_unit(moisture(wet, params)));
            out.insolation.push(encode_unit(table.at(slope, azimuth)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: f64 = 8.0;

    fn stencil(f: impl Fn(f64, f64) -> f64, si: f64, sj: f64) -> [f64; 9] {
        let mut z = [0.0; 9];
        for dj in 0..3 {
            for di in 0..3 {
                z[dj * 3 + di] = f((di as f64 - 1.0) * si, (dj as f64 - 1.0) * sj);
            }
        }
        z
    }

    // spec: GEN-070
    #[test]
    fn the_gradient_and_the_mean_curvature_are_the_surfaces_for_a_plane_a_bowl_a_saddle_and_a_tilted_cone() {
        let (p, q, k) = gradient_and_curvature(&stencil(|x, y| 0.3 * x + 0.1 * y, L, L), L, L);
        assert!((p - 0.3).abs() < 1e-12 && (q - 0.1).abs() < 1e-12 && k.abs() < 1e-12, "{p} {q} {k}");
        let kappa = 0.01;
        let (_, _, k) = gradient_and_curvature(&stencil(|x, y| 0.5 * kappa * (x * x + y * y), L, L), L, L);
        assert!((k - kappa).abs() < 1e-12, "{k}");
        let f = curvature_factor(k, &DriverParams::default());
        assert!((f - 1.3808).abs() < 1e-4, "the curvature factor of kappa 0.01 per metre is {f}");
        // A ridge is negative, a saddle z = c x y has the mean curvature -2 p q s / g^3/2 = 0 at its centre and a tilted one gets the gradient terms.
        let (_, _, k) = gradient_and_curvature(&stencil(|x, y| -0.5 * kappa * (x * x + y * y), L, L), L, L);
        assert!((k + kappa).abs() < 1e-12);
        let (a, b, c) = (0.2, -0.1, 0.004);
        let z = stencil(|x, y| a * x + b * y + c * x * y, L, L);
        let (p, q, k) = gradient_and_curvature(&z, L, L);
        let g = 1.0 + a * a + b * b;
        let want = -2.0 * a * b * c / (2.0 * g * libm::sqrt(g));
        assert!((p - a).abs() < 1e-12 && (q - b).abs() < 1e-12 && (k - want).abs() < 1e-12, "{k} against {want}");
        // A cone of slope 0.5 about the stencil's centre (sampled off the apex, at distance 3 cells): the curvature is 1 / (2 r) of a cone, over the slope term.
        let (cx, cy) = (3.0 * L, 0.0);
        let cone = stencil(|x, y| 0.5 * libm::sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy)), L, L);
        let (_, _, k) = gradient_and_curvature(&cone, L, L);
        assert!(k > 0.0 && (k - 0.5 / (2.0 * 3.0 * L * libm::sqrt(1.25))).abs() < 5e-4, "the cone's curvature {k}");
        // The spacing along the axes can differ: a plane of true slope 0.3 along an axis whose cells are 0.707 as wide reads 0.3.
        let (p, _, _) = gradient_and_curvature(&stencil(|x, _| 0.3 * x, 0.707 * L, L), 0.707 * L, L);
        assert!((p - 0.3).abs() < 1e-12);
    }

    // spec: GEN-070
    #[test]
    fn the_wetness_floors_the_slope_clamps_the_area_and_the_moisture_stays_in_the_unit_range() {
        let p = DriverParams::default();
        let flat = wetness_index(64.0 * 100.0, 8.0, 0.0, &p);
        assert!((flat - libm::log(6400.0 / (8.0 * 0.01))).abs() < 1e-12);
        let max = wetness_index(1.0e12, 8.0, 0.0, &p);
        assert!((max - libm::log(1.0e8 / (8.0 * 0.01))).abs() < 1e-12, "{max}");
        assert_eq!(moisture(max, &p), 1.0);
        assert!(wetness_index(1.0e5, 8.0, 0.5, &p) < wetness_index(1.0e5, 8.0, 0.05, &p));
        assert!(wetness_index(1.0e4, 8.0, 0.05, &p) < wetness_index(1.0e6, 8.0, 0.05, &p));
        for w in [-5.0, 0.0, 4.0, 10.0, 25.0] {
            assert!((0.0..=1.0).contains(&moisture(w, &p)));
        }
    }

    // spec: GEN-070
    #[test]
    fn the_soil_is_deeper_in_a_hollow_on_flat_ground_and_with_more_flow_and_thin_on_steep_ground_and_high_up() {
        let p = DriverParams::default();
        let base = soil_depth_m(0.0, 0.0, 1.0e6, 0.0, 100.0, &p);
        assert!(base > 1.5 && base <= p.soil_max_m, "{base}");
        assert!(soil_depth_m(1.0, 0.0, 1.0e6, 0.0, 100.0, &p) < base * 0.1, "steep ground is thin");
        assert!(soil_depth_m(0.0, 0.02, 1.0e6, 0.0, 100.0, &p) > base && soil_depth_m(0.0, -0.02, 1.0e6, 0.0, 100.0, &p) < base);
        assert!(soil_depth_m(0.0, 0.0, 1.0e8, 0.0, 100.0, &p) > soil_depth_m(0.0, 0.0, 1.0e3, 0.0, 100.0, &p));
        assert_eq!(soil_depth_m(0.0, 0.0, 1.0e6, 0.0, 5000.0, &p), 0.0, "above the altitude range there is no soil");
        assert!(soil_depth_m(0.0, 0.0, 1.0e6, 1.0, 100.0, &p) > soil_depth_m(0.0, 0.0, 1.0e6, -1.0, 100.0, &p));
        // The square-root code is exact to half a code step: |sqrt(back / max) - sqrt(d / max)| <= 0.5 / 255.
        for k in 0..=800 {
            let d = p.soil_max_m * f64::from(k) / 800.0;
            let back = decode_soil(encode_soil(d, &p), &p);
            assert!((libm::sqrt(back / p.soil_max_m) - libm::sqrt(d / p.soil_max_m)).abs() <= 0.5 / 255.0 + 1e-12, "{d} -> {back}");
        }
    }

    // spec: GEN-070
    #[test]
    fn the_lithology_reads_the_f64_direction_and_is_a_function_of_it_and_the_seed_alone() {
        let p = DriverParams { seed: 7, ..DriverParams::default() };
        let d = Vec3::new(0.3, -0.5, 0.8).normalized();
        assert_eq!(lithology(d, &p).to_bits(), lithology(d, &p).to_bits());
        assert_ne!(lithology(d, &p), lithology(d, &DriverParams { seed: 8, ..p }));
        let mut seen = (f64::INFINITY, f64::NEG_INFINITY);
        for k in 0..200 {
            let v = lithology((d + Vec3::new(0.01 * k as f64, 0.0, 0.0)).normalized(), &p);
            assert!((-1.0..=1.0).contains(&v));
            seen = (seen.0.min(v), seen.1.max(v));
        }
        assert!(seen.1 - seen.0 > 0.2, "the noise varies: {seen:?}");
    }

    /// The mean irradiance on a plane by the sun's vector, every minute of 365 days: independent of the cosine formula (a frame of east, north and up).
    fn brute_force(lat: f64, slope: f64, azimuth: f64) -> f64 {
        let mean = |lat: f64, slope: f64, azimuth: f64| {
            let normal = [libm::sin(slope) * libm::sin(azimuth), libm::sin(slope) * libm::cos(azimuth), libm::cos(slope)];
            let (mut total, mut count) = (0.0, 0.0);
            for day in 0..365 {
                let d = declination(f64::from(day) + 1.0);
                for minute in 0..1440 {
                    let w = core::f64::consts::TAU * (f64::from(minute) + 0.5) / 1440.0 - core::f64::consts::PI;
                    let east = -libm::cos(d) * libm::sin(w);
                    let north = libm::cos(lat) * libm::sin(d) - libm::sin(lat) * libm::cos(d) * libm::cos(w);
                    let up = libm::sin(lat) * libm::sin(d) + libm::cos(lat) * libm::cos(d) * libm::cos(w);
                    if up > 0.0 {
                        total += (east * normal[0] + north * normal[1] + up * normal[2]).max(0.0);
                    }
                    count += 1.0;
                }
            }
            total / count
        };
        mean(lat, slope, azimuth) / mean(0.0, 0.0, 0.0)
    }

    // spec: GEN-070
    #[test]
    fn the_insolation_is_within_a_percent_of_a_brute_force_integration_at_the_equator_and_at_45_degrees_north_up_to_the_tables_60_degrees()
    {
        let mut worst = 0.0f64;
        for lat_deg in [0.0, 45.0] {
            for (slope_deg, azimuth_deg) in
                [(0.0, 0.0), (20.0, 0.0), (20.0, 180.0), (40.0, 90.0), (40.0, 270.0), (30.0, 200.0), (60.0, 0.0), (60.0, 180.0)]
            {
                let (lat, slope, azimuth) = (f64::to_radians(lat_deg), f64::to_radians(slope_deg), f64::to_radians(azimuth_deg));
                let (fast, slow) = (annual_mean_irradiance(lat, slope, azimuth), brute_force(lat, slope, azimuth));
                worst = worst.max((fast - slow).abs() / slow.max(0.05));
                assert!(
                    (fast - slow).abs() <= 0.01 * slow.max(0.05),
                    "lat {lat_deg} slope {slope_deg} azimuth {azimuth_deg}: {fast} against {slow}"
                );
            }
        }
        assert!(worst > 0.0);
        // The north and south faces differ at 45 degrees north: the sun is in the south.
        let (n, s) = (annual_mean_irradiance(0.785, 0.5, 0.0), annual_mean_irradiance(0.785, 0.5, core::f64::consts::PI));
        assert!(s > n * 1.3, "south {s}, north {n}");
    }

    // spec: GEN-070
    #[test]
    fn the_table_reads_the_function_it_tabulates_and_wraps_the_aspect() {
        let lat = 0.6;
        let table = InsolationTable::new(lat);
        for (slope, azimuth) in [(0.0, 0.0), (0.3, 1.0), (0.5, 5.0), (0.9, 6.2)] {
            let (t, f) = (table.at(slope, azimuth), annual_mean_irradiance(lat, slope, azimuth));
            assert!((t - f).abs() < 0.02, "slope {slope} azimuth {azimuth}: {t} against {f}");
        }
        assert!((table.at(0.4, 0.0) - table.at(0.4, core::f64::consts::TAU)).abs() < 1e-12);
    }

    fn plane_chunk(n: usize, heights: Vec<f64>, lat_rad: f64, compass: [f64; 4], spacing: [f64; 2]) -> ChunkDrivers {
        let area = vec![1.0e6; n * n];
        let dirs = vec![Vec3::new(1.0, 0.0, 0.0); n * n];
        let compass = vec![compass; n * n];
        let spacing = vec![spacing; n * n];
        let raster = ChunkRaster { n, heights: &heights, area_m2: &area, dirs: &dirs, spacing_m: &spacing, compass: &compass, lat_rad };
        chunk_drivers(&raster, &DriverParams::default())
    }

    fn ramp(n: usize, f: impl Fn(f64, f64) -> f64) -> Vec<f64> {
        (0..(n + 2) * (n + 2)).map(|k| f((k % (n + 2)) as f64 * L, (k / (n + 2)) as f64 * L)).collect()
    }

    // spec: GEN-070
    #[test]
    fn a_chunk_of_a_tilted_plane_gives_the_drivers_of_that_plane_with_the_compass_and_the_spacing_and_the_same_bits_on_another_thread() {
        let n = 16;
        let id = [1.0, 0.0, 0.0, 1.0];
        let params = DriverParams::default();
        let slope = libm::atan(0.3);
        // Rising to the east it faces west; rising to the north, south; at 45 degrees north the south face has the sun.
        let west = plane_chunk(n, ramp(n, |x, _| 0.3 * x), 0.0, id, [L, L]);
        let want = annual_mean_irradiance(0.0, slope, 3.0 * core::f64::consts::FRAC_PI_2);
        assert!((decode_unit(west.insolation[0]) - want).abs() < 0.01, "{} against {want}", decode_unit(west.insolation[0]));
        let lat = 45.0_f64.to_radians();
        let south = plane_chunk(n, ramp(n, |_, y| 0.3 * y), lat, id, [L, L]);
        let north = plane_chunk(n, ramp(n, |_, y| -0.3 * y), lat, id, [L, L]);
        let (s, t) = (decode_unit(south.insolation[0]), decode_unit(north.insolation[0]));
        assert!((s - annual_mean_irradiance(lat, slope, core::f64::consts::PI)).abs() < 0.01, "south face {s}");
        assert!((t - annual_mean_irradiance(lat, slope, 0.0)).abs() < 0.01 && s > t * 1.3, "north face {t}");
        // A compass turned a quarter (the raster's i axis is north, its j axis west): a plane rising along i faces south all the same.
        let turned = plane_chunk(n, ramp(n, |x, _| 0.3 * x), lat, [0.0, -1.0, 1.0, 0.0], [L, L]);
        assert_eq!(turned.insolation[0], south.insolation[0]);
        // Cells 0.707 as wide along i: a plane of true slope 0.3 has heights rising 0.3 x 0.707 L per cell and reads the same moisture and soil.
        let narrow = plane_chunk(n, ramp(n, |x, _| 0.3 * 0.707 * x), 0.0, id, [0.707 * L, L]);
        assert_eq!((narrow.soil[0], narrow.insolation[0]), (west.soil[0], west.insolation[0]));
        // The same bits on a second thread; a cell is equal to every other on a plane.
        let again = std::thread::spawn(move || plane_chunk(16, ramp(16, |x, _| 0.3 * x), 0.0, [1.0, 0.0, 0.0, 1.0], [L, L]))
            .join()
            .expect("thread");
        assert_eq!((&west.soil, &west.moisture, &west.insolation), (&again.soil, &again.moisture, &again.insolation));
        assert!(west.soil.iter().all(|&v| v == west.soil[0]) && west.moisture.iter().all(|&m| m == west.moisture[0]));
        let _ = params;
    }

    // spec: GEN-070
    #[test]
    #[should_panic(expected = "height that is not a number")]
    fn a_raster_with_a_height_that_is_not_a_number_is_refused() {
        let n = 8;
        let mut heights = ramp(n, |x, _| 0.1 * x);
        heights[40] = f64::NAN;
        plane_chunk(n, heights, 0.0, [1.0, 0.0, 0.0, 1.0], [L, L]);
    }
}
