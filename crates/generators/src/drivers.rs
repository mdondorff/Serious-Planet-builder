//! The driver block of a compact tile (GEN-074, ADR 0076): the codes of the soil depth, the moisture and the insolation that the tile carries, one 8-bit
//! code each, and the neutral values the builder writes (soil 1 m, moisture 0.5, the insolation of the latitude alone). `libm` for every
//! transcendental, no threads, a fixed order (CON-14). The chunk drivers that once filled the block are removed (ADR 0086); the block stays in the slot
//! (GEN-025).

/// The constants of the 8-bit codes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DriverParams {
    /// The soil depth in metres that code 255 stands for.
    pub soil_max_m: f64,
}

impl Default for DriverParams {
    fn default() -> DriverParams {
        DriverParams { soil_max_m: 8.0 }
    }
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

/// The declination of the sun in radians on day `n` of the year (Cooper).
fn declination(n: f64) -> f64 {
    23.45_f64.to_radians() * libm::sin(core::f64::consts::TAU * (284.0 + n) / 365.0)
}

/// The neutral soil depth in metres and moisture: what the builder writes in the driver block of every tile (GEN-074).
pub const NEUTRAL_SOIL_M: f64 = 1.0;
pub const NEUTRAL_MOISTURE: f64 = 0.5;

/// The neutral insolation at a latitude: the flat plane's annual mean there (the latitude alone), in units of the flat plane at the equator. The closed
/// form of the daily mean on a horizontal plane (Duffie and Beckman, the sunset hour angle `h0 = acos(-tan f tan d)` with the polar day and night as the
/// limits) over the same 73 days as `annual_mean_irradiance` (the test reference), which it equals within 3e-4. The sines, cosines and tangents of the days and the value at
/// the equator are made once ([`NeutralInsolation::new`]), so a tile builder pays two libm calls a day for each texel.
pub struct NeutralInsolation {
    days: Vec<(f64, f64, f64)>,
    flat_equator: f64,
}

impl NeutralInsolation {
    pub fn new() -> NeutralInsolation {
        let days = (0..DAYS)
            .map(|k| {
                let d = declination((3 + DAY_STEP * k) as f64);
                (libm::sin(d), libm::cos(d), libm::tan(d))
            })
            .collect();
        let mut table = NeutralInsolation { days, flat_equator: 1.0 };
        table.flat_equator = table.mean(0.0);
        table
    }

    fn mean(&self, lat: f64) -> f64 {
        let (sf, cf, tf) = (libm::sin(lat), libm::cos(lat), libm::tan(lat));
        let mut total = 0.0;
        for &(sd, cd, td) in &self.days {
            let x = -tf * td;
            let h0 = if x >= 1.0 {
                0.0
            } else if x <= -1.0 {
                core::f64::consts::PI
            } else {
                libm::acos(x)
            };
            total += (h0 * sf * sd + cf * cd * libm::sin(h0)) / core::f64::consts::PI;
        }
        total / DAYS as f64
    }

    /// The neutral insolation at latitude `lat_rad` (at the poles `tan` of the latitude is huge, which the clamp turns into the polar day or night).
    pub fn at(&self, lat_rad: f64) -> f64 {
        let limit = core::f64::consts::FRAC_PI_2 - 1.0e-4;
        self.mean(lat_rad.clamp(-limit, limit)) / self.flat_equator
    }
}

impl Default for NeutralInsolation {
    fn default() -> NeutralInsolation {
        NeutralInsolation::new()
    }
}

/// [`NeutralInsolation::at`] for one latitude (a tile builder makes the table once and calls `at`).
pub fn neutral_insolation(lat_rad: f64) -> f64 {
    NeutralInsolation::new().at(lat_rad)
}

/// Texels of the driver block of a tile: 9 x 9 like the climate block.
pub const DRIVER_TEXELS: usize = 81;

/// The word of one texel of the tile's driver block (GEN-074, GEN-025): the soil depth code in bits 0-7, the moisture in 8-15, the insolation in 16-23
/// and the neutral insolation of the texel's latitude in 24-31 (what the shader fades the insolation from).
pub fn pack_driver_word(codes: [u8; 3], neutral_insolation: u8) -> u32 {
    u32::from(codes[0]) | u32::from(codes[1]) << 8 | u32::from(codes[2]) << 16 | u32::from(neutral_insolation) << 24
}

/// The codes `[soil, moisture, insolation]` and the neutral insolation code of a driver word.
pub fn unpack_driver_word(word: u32) -> ([u8; 3], u8) {
    ([word as u8, (word >> 8) as u8, (word >> 16) as u8], (word >> 24) as u8)
}

/// The codes of the neutral drivers at latitude `lat_rad`: soil 1 m, moisture 0.5 and the neutral insolation.
pub fn neutral_codes(lat_rad: f64, params: &DriverParams, insolation: &NeutralInsolation) -> [u8; 3] {
    [encode_soil(NEUTRAL_SOIL_M, params), encode_unit(NEUTRAL_MOISTURE), encode_unit(insolation.at(lat_rad))]
}

/// The CPU mirror of the shader's read of a tile's driver block (GEN-074): at the tile coordinates `(u, v)` in `[0, 1]` the four neighbouring words of the 9 x 9
/// block decoded to (soil depth in metres, moisture, insolation, neutral insolation), interpolated bilinearly in that order (decode, then interpolate), and
/// blended with the weight `w` as the fragment stage does: soil `1 + w (d - 1)`, moisture `0.5 + w (d - 0.5)`, insolation `n + w (d - n)`.
pub fn tile_drivers_at(words: &[u32; DRIVER_TEXELS], u: f64, v: f64, w: f64, params: &DriverParams) -> [f64; 3] {
    let (x, y) = (u.clamp(0.0, 1.0) * 8.0, v.clamp(0.0, 1.0) * 8.0);
    let (a, b) = ((x as usize).min(7), (y as usize).min(7));
    let (wx, wy) = (x - a as f64, y - b as f64);
    let texel = |i: usize| -> [f64; 4] {
        let (codes, neutral) = unpack_driver_word(words[i]);
        [decode_soil(codes[0], params), decode_unit(codes[1]), decode_unit(codes[2]), decode_unit(neutral)]
    };
    let (t00, t10, t01, t11) = (texel(b * 9 + a), texel(b * 9 + a + 1), texel((b + 1) * 9 + a), texel((b + 1) * 9 + a + 1));
    let d: Vec<f64> = (0..4).map(|k| (t00[k] * (1.0 - wx) + t10[k] * wx) * (1.0 - wy) + (t01[k] * (1.0 - wx) + t11[k] * wx) * wy).collect();
    [blend_driver(NEUTRAL_SOIL_M, d[0], w), blend_driver(NEUTRAL_MOISTURE, d[1], w), blend_driver(d[3], d[2], w)]
}

/// A driver as a consumer reads it: `neutral + w (chunk - neutral)`, written `neutral (1 - w) + chunk w` so that both ends are exact (monotone
/// up to a rounding step), with `w` in `[0, 1]` the weight of the block's value; the fragment stage reads the block at weight 1.
pub fn blend_driver(neutral: f64, chunk: f64, w: f64) -> f64 {
    let w = w.clamp(0.0, 1.0);
    neutral * (1.0 - w) + chunk * w
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR_STEPS: usize = 96;

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

    /// The annual-mean irradiance on a tilted plane in units of the flat plane at the equator (see [`SunGrid::mean`]): 73 days, 96 steps of the day.
    fn annual_mean_irradiance(lat_rad: f64, slope_rad: f64, azimuth_rad: f64) -> f64 {
        let grid = SunGrid::new();
        grid.mean(lat_rad, slope_rad, azimuth_rad) / grid.mean(0.0, 0.0, 0.0)
    }

    // spec: GEN-074
    #[test]
    fn the_neutral_insolation_is_the_flat_plane_of_the_integration_at_every_latitude_and_rises_to_the_equator() {
        let mut last = f64::INFINITY;
        for deg in (0..=90).step_by(5) {
            let lat = f64::from(deg).to_radians();
            let (closed, integrated) = (neutral_insolation(lat), annual_mean_irradiance(lat, 0.0, 0.0));
            assert!((closed - integrated).abs() < 1.0e-3, "latitude {deg}: {closed} against {integrated}");
            assert!(closed <= last + 1e-12, "latitude {deg}: not falling towards the pole");
            last = closed;
        }
        assert!((neutral_insolation(0.0) - 1.0).abs() < 1e-12);
        assert!((neutral_insolation(0.7) - neutral_insolation(-0.7)).abs() < 0.03, "the hemispheres are near symmetric over the year");
    }

    // spec: GEN-071, GEN-074
    #[test]
    fn the_cpu_mirror_of_the_shaders_read_decodes_before_it_interpolates_and_blends_from_the_neutral_values() {
        let params = DriverParams::default();
        // A block whose soil code runs 0 to 255 along a, so the decode-then-interpolate order shows.
        let mut words = [0u32; DRIVER_TEXELS];
        for b in 0..9 {
            for a in 0..9 {
                words[b * 9 + a] = pack_driver_word([(a * 255 / 8) as u8, (b * 255 / 8) as u8, 255], 100);
            }
        }
        let at = |u: f64, v: f64, w: f64| tile_drivers_at(&words, u, v, w, &params);
        // At a texel the decode is exact; at weight 0 the neutral values.
        assert!((at(1.0, 0.0, 1.0)[0] - decode_soil(255, &params)).abs() < 1e-9);
        assert_eq!(at(0.3, 0.6, 0.0), [NEUTRAL_SOIL_M, NEUTRAL_MOISTURE, decode_unit(100)]);
        // Halfway between the first two soil codes 0 and 31 the decoded values 0 and (31/255)^2 8 average (not the decode of the mean code).
        let half = at(1.0 / 16.0, 0.0, 1.0)[0];
        assert!((half - 0.5 * decode_soil(31, &params)).abs() < 1e-9, "{half}");
        // A weight between blends linearly from the neutral value.
        let (full, mid) = (at(0.5, 0.5, 1.0), at(0.5, 0.5, 0.5));
        assert!((mid[0] - 0.5 * (NEUTRAL_SOIL_M + full[0])).abs() < 1e-9 && (mid[2] - 0.5 * (decode_unit(100) + full[2])).abs() < 1e-9);
    }
}
