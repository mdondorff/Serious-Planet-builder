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
/// on a ridge) at the centre of a 3 x 3 stencil `z[(dj + 1) * 3 + (di + 1)]` of heights at `cell_m` spacing (Zevenbergen and Thorne).
pub fn gradient_and_curvature(z: &[f64; 9], cell_m: f64) -> (f64, f64, f64) {
    let at = |di: usize, dj: usize| z[dj * 3 + di];
    let p = (at(2, 1) - at(0, 1)) / (2.0 * cell_m);
    let q = (at(1, 2) - at(1, 0)) / (2.0 * cell_m);
    let r = (at(2, 1) - 2.0 * at(1, 1) + at(0, 1)) / (cell_m * cell_m);
    let t = (at(1, 2) - 2.0 * at(1, 1) + at(1, 0)) / (cell_m * cell_m);
    let s = (at(2, 2) - at(0, 2) - at(2, 0) + at(0, 0)) / (4.0 * cell_m * cell_m);
    let g = 1.0 + p * p + q * q;
    let kappa = ((1.0 + q * q) * r - 2.0 * p * q * s + (1.0 + p * p) * t) / (2.0 * g * libm::sqrt(g));
    (p, q, kappa)
}

/// The wetness index `W = ln(A / (w tan s))` for the drainage area `area_m2`, the cell width `cell_m` and `tan s` (floored, the area clamped).
pub fn wetness_index(area_m2: f64, cell_m: f64, tan_s: f64, params: &DriverParams) -> f64 {
    let a = area_m2.clamp(cell_m * cell_m, params.area_clamp_m2);
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
/// and height `height_m` over the water.
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

/// The representative days of the year of Klein (1977), one for each month.
const KLEIN_DAYS: [f64; 12] = [17.0, 47.0, 75.0, 105.0, 135.0, 162.0, 198.0, 228.0, 258.0, 288.0, 318.0, 344.0];
/// The steps of the day.
const HOUR_STEPS: usize = 24;

/// The declination of the sun in radians on day `n` of the year (Cooper).
fn declination(n: f64) -> f64 {
    23.45_f64.to_radians() * libm::sin(core::f64::consts::TAU * (284.0 + n) / 365.0)
}

/// The mean over the twelve days and the day's hours of the beam irradiance on a plane of slope `slope_rad` and surface azimuth `azimuth_rad` (from north,
/// clockwise: the direction the plane faces) at latitude `lat_rad`, in units of the flat plane at the equator, with the sun above the horizon only and no
/// shadowing by terrain: Duffie and Beckman's `cos theta = sin d (sin f cos b - cos f sin b cos g) + cos d cos w (cos f cos b + sin f sin b cos g)
/// + cos d sin b sin g sin w` with `g` the azimuth from south (positive to the west).
pub fn annual_mean_irradiance(lat_rad: f64, slope_rad: f64, azimuth_rad: f64) -> f64 {
    let mean = |lat: f64, slope: f64, azimuth: f64| -> f64 {
        // The azimuth from south: south-facing is 0, west-facing positive.
        let g = azimuth - core::f64::consts::PI;
        let (sf, cf, sb, cb, sg, cg) = (libm::sin(lat), libm::cos(lat), libm::sin(slope), libm::cos(slope), libm::sin(g), libm::cos(g));
        let mut total = 0.0;
        for day in KLEIN_DAYS {
            let d = declination(day);
            let (sd, cd) = (libm::sin(d), libm::cos(d));
            for h in 0..HOUR_STEPS {
                let w = core::f64::consts::TAU * (h as f64 + 0.5) / HOUR_STEPS as f64 - core::f64::consts::PI;
                let (sw, cw) = (libm::sin(w), libm::cos(w));
                let up = sd * sf + cd * cf * cw;
                if up <= 0.0 {
                    continue;
                }
                let c = sd * (sf * cb - cf * sb * cg) + cd * cw * (cf * cb + sf * sb * cg) + cd * sb * sg * sw;
                total += c.max(0.0);
            }
        }
        total / (12.0 * HOUR_STEPS as f64)
    };
    mean(lat_rad, slope_rad, azimuth_rad) / mean(0.0, 0.0, 0.0)
}

/// The slopes and aspects of the insolation table.
const TABLE_SLOPES: usize = 17;
const TABLE_ASPECTS: usize = 16;
/// The steepest slope of the table in degrees.
const TABLE_MAX_DEG: f64 = 60.0;

/// The annual-mean insolation of a chunk: a table over slope and aspect at the chunk centre's latitude, read bilinearly (the latitude changes by 0.04
/// degrees across a chunk).
pub struct InsolationTable {
    values: Vec<f64>,
}

impl InsolationTable {
    pub fn new(lat_rad: f64) -> InsolationTable {
        let mut values = Vec::with_capacity(TABLE_SLOPES * (TABLE_ASPECTS + 1));
        for s in 0..TABLE_SLOPES {
            let slope = (TABLE_MAX_DEG * s as f64 / (TABLE_SLOPES - 1) as f64).to_radians();
            for a in 0..=TABLE_ASPECTS {
                values.push(annual_mean_irradiance(lat_rad, slope, core::f64::consts::TAU * a as f64 / TABLE_ASPECTS as f64));
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

/// What the drivers need of a chunk. `heights` is the `(n + 2) x (n + 2)` raster of the core and one ring (row-major, `j` outer), `area_m2` the `n x n`
/// drainage area of the core, `dirs` the unit directions of the core's cells, `(east, north)` the local east and north unit vectors at each cell as
/// `(i-axis, j-axis)` components: the rotation from the raster's axes to the compass, `lat_rad` the latitude of the chunk's centre.
pub struct ChunkRaster<'a> {
    pub n: usize,
    pub cell_m: f64,
    pub heights: &'a [f64],
    pub area_m2: &'a [f64],
    pub dirs: &'a [Vec3],
    pub compass: &'a [[f64; 4]],
    pub lat_rad: f64,
}

/// The drivers of every core cell of `chunk`.
pub fn chunk_drivers(chunk: &ChunkRaster, params: &DriverParams) -> ChunkDrivers {
    let n = chunk.n;
    assert_eq!(chunk.heights.len(), (n + 2) * (n + 2));
    assert_eq!(chunk.area_m2.len(), n * n);
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
            let (p, q, kappa) = gradient_and_curvature(&z, chunk.cell_m);
            let tan_s = libm::sqrt(p * p + q * q);
            let slope = libm::atan(tan_s);
            let cell = j * n + i;
            let area = chunk.area_m2[cell];
            // The downhill direction on the compass: the gradient of the height rotated to (east, north); the plane faces down the gradient.
            let [ei, ej, ni, nj] = chunk.compass[cell];
            let (ge, gn) = (p * ei + q * ej, p * ni + q * nj);
            let azimuth = if ge == 0.0 && gn == 0.0 { 0.0 } else { libm::atan2(-ge, -gn) };
            let wet = wetness_index(area, chunk.cell_m, tan_s, params);
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

    // spec: GEN-070
    #[test]
    fn a_plane_has_its_slope_and_no_curvature_and_a_paraboloid_its_curvature() {
        let l = 8.0;
        let plane: Vec<f64> = (0..3).flat_map(|j| (0..3).map(move |i| 0.3 * l * i as f64 + 0.1 * l * j as f64)).collect();
        let (p, q, k) = gradient_and_curvature(plane.as_slice().try_into().unwrap(), l);
        assert!((p - 0.3).abs() < 1e-12 && (q - 0.1).abs() < 1e-12 && k.abs() < 1e-12, "{p} {q} {k}");
        // z = kappa / 2 (x^2 + y^2) about the centre: the curvature is kappa, in a hollow positive.
        let kappa = 0.01;
        let bowl: Vec<f64> = (0..3)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    0.5 * kappa * {
                        let (x, y) = ((i as f64 - 1.0) * l, (j as f64 - 1.0) * l);
                        x * x + y * y
                    }
                })
            })
            .collect();
        let (_, _, k) = gradient_and_curvature(bowl.as_slice().try_into().unwrap(), l);
        assert!((k - kappa).abs() < 1e-12, "{k}");
        let f = curvature_factor(k, &DriverParams::default());
        assert!((f - 1.3808).abs() < 1e-4, "the curvature factor of kappa 0.01 per metre is {f}");
    }

    // spec: GEN-070
    #[test]
    fn the_wetness_floors_the_slope_clamps_the_area_and_the_moisture_stays_in_the_unit_range() {
        let p = DriverParams::default();
        // A flat has the floor's value: ln(A / (w 0.01)).
        let flat = wetness_index(64.0 * 100.0, 8.0, 0.0, &p);
        assert!((flat - libm::log(6400.0 / (8.0 * 0.01))).abs() < 1e-12);
        // A hollow whose area exceeds the clamp is as wet as the clamp says.
        let max = wetness_index(1.0e12, 8.0, 0.0, &p);
        assert!((max - libm::log(1.0e8 / (8.0 * 0.01))).abs() < 1e-12, "{max}");
        assert_eq!(moisture(max, &p), 1.0);
        // Steeper is drier, a bigger area is wetter, and the moisture stays in [0, 1].
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
        // The codes round trip within their step.
        for d in [0.0, 0.2, 1.0, 4.0, 8.0] {
            let back = decode_soil(encode_soil(d, &p), &p);
            assert!((back - d).abs() < 0.07 * (1.0 + d), "{d} -> {back}");
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
            // Frame at the latitude: x east, y north, z up. The plane's normal tilts from up towards `azimuth` (from north, clockwise) by `slope`.
            let normal = [libm::sin(slope) * libm::sin(azimuth), libm::sin(slope) * libm::cos(azimuth), libm::cos(slope)];
            let (mut total, mut count) = (0.0, 0.0);
            for day in 0..365 {
                let d = declination(f64::from(day) + 1.0);
                for minute in 0..1440 {
                    let w = core::f64::consts::TAU * (f64::from(minute) + 0.5) / 1440.0 - core::f64::consts::PI;
                    // The sun's direction in the local frame from the hour angle, the declination and the latitude.
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
    fn the_insolation_is_within_a_percent_of_a_brute_force_integration_at_the_equator_and_at_45_degrees_north() {
        for lat_deg in [0.0, 45.0] {
            for (slope_deg, azimuth_deg) in [(0.0, 0.0), (20.0, 0.0), (20.0, 180.0), (40.0, 90.0), (40.0, 270.0), (30.0, 200.0)] {
                let (lat, slope, azimuth) = (f64::to_radians(lat_deg), f64::to_radians(slope_deg), f64::to_radians(azimuth_deg));
                let (fast, slow) = (annual_mean_irradiance(lat, slope, azimuth), brute_force(lat, slope, azimuth));
                assert!(
                    (fast - slow).abs() <= 0.01 * slow.max(0.05),
                    "lat {lat_deg} slope {slope_deg} azimuth {azimuth_deg}: {fast} against {slow}"
                );
            }
        }
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

    // spec: GEN-070
    #[test]
    fn a_chunk_of_a_tilted_plane_gives_the_drivers_of_that_plane_and_the_same_bits_each_time() {
        let n = 16;
        let cell_m = 8.0;
        // A plane rising to the east by 0.3 (the i axis is east, the j axis north): it faces west.
        let heights: Vec<f64> = (0..(n + 2) * (n + 2)).map(|k| 0.3 * cell_m * (k % (n + 2)) as f64 + 100.0).collect();
        let area = vec![1.0e6; n * n];
        let dirs = vec![Vec3::new(1.0, 0.0, 0.0); n * n];
        let compass = vec![[1.0, 0.0, 0.0, 1.0]; n * n];
        let raster = ChunkRaster { n, cell_m, heights: &heights, area_m2: &area, dirs: &dirs, compass: &compass, lat_rad: 0.0 };
        let params = DriverParams::default();
        let a = chunk_drivers(&raster, &params);
        let b = chunk_drivers(&raster, &params);
        assert_eq!((&a.soil, &a.moisture, &a.insolation), (&b.soil, &b.moisture, &b.insolation));
        let slope = libm::atan(0.3);
        // West-facing at the equator: the afternoon sun, the same as east-facing, below the flat plane's value of 1 by the tilt.
        let want = annual_mean_irradiance(0.0, slope, 3.0 * core::f64::consts::FRAC_PI_2);
        assert!((decode_unit(a.insolation[0]) - want).abs() < 0.02, "{} against {want}", decode_unit(a.insolation[0]));
        assert!(a.soil.iter().all(|&s| s == a.soil[0]) && a.moisture.iter().all(|&m| m == a.moisture[0]));
    }
}
