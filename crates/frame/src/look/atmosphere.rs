//! The atmosphere (ADR 0040, M5 item 5): Hillaire's 2020 model with its static look-up tables computed here **on the CPU in f64**, the
//! f64 reference of every per-frame pass the GPU runs (sky view, aerial perspective, the orbit ray march), and the maths that
//! decides the per-frame inputs of those passes (CON-02, CON-03: the renderer executes, the tests compare).
//!
//! Class and bound (ADR 0035, ADR 0043): **presentation data**. Radiance is in units of the top-of-atmosphere solar illuminance
//! (= 1; a clear daylight sky is about 0.05 in these units). The GPU passes match this module's reference within
//! `|GPU - ref| <= 1 % * |ref| + 1e-4` per channel (ATMO-001), evaluated on the f32-rounded inputs the GPU receives
//! ([`Atmosphere::rounded_f32`], [`FrameGeometry::rounded_f32`], [`StaticLuts::rounded_f32`]) with the same texel-to-direction
//! mapping, sample count and sample placement, so the bound measures arithmetic and not discretisation; the discretisation (the
//! LUTs against the converged integral) is a separate bound (ATMO-007).
//!
//! **Altitude-relative primitives at every altitude** (ADR 0040 point 5, SPIKE-09). A ray from altitude `h` (`r = R + h`,
//! `c = r^2 - R^2 = h (2R + h)`) in direction `mu` (cosine to the local up) never forms `r^2 - R^2`, `r - R` or a ray-sphere root as
//! a difference of planet-sized numbers: the ground distance is `c / (-r mu + sqrt((r mu)^2 - c))`, the exit through the top
//! (`c_top = (h - H)(2R + h + H)`) is `-c_top / (r mu + sqrt(D))` for `mu >= 0` and `-r mu + sqrt(D)` for `mu < 0`, and the altitude
//! of the sample at distance `s` is `h_s = (c + s (s + 2 r mu)) / (r_s + R)`. The CPU supplies `h`, `c`, `c_top` and `r` from f64.
//! From outside the atmosphere the march starts at the entry point, whose altitude, `c` and `r mu = -sqrt(D_top)` are exact, so
//! the samples are measured from there and not from a camera 20,000 km away.
//!
//! Static tables (computed once per body, never cached, never keyed: presentation data):
//! * the **transmittance LUT** (256 x 64) stores the optical depth `tau` (RGB) from a point to the top of the atmosphere along a ray
//!   (`T = exp(-tau)`; interpolating `tau` linearly and exponentiating after is far more accurate than interpolating `T`). The
//!   horizontal axis is Hillaire's `x_mu = (d - d_min) / (d_max - d_min)` with `d` the distance to the top, the vertical one is
//!   `x_r = sqrt(c) / sqrt(c_top_of_ground)`; texel `i` sits at `x = i / (N - 1)` and lookups use that exact mapping with a manual
//!   bilinear interpolation (no hardware filter weights);
//! * the **multiple-scattering LUT** (32 x 32) stores Hillaire's `psi_ms(mu_sun, h)` (RGB): the second-and-higher-order isotropic
//!   in-scattered radiance at a point, `L_2 / (1 - f_ms)`, with the ground bounce included. Axes: `(mu_sun + 1) / 2` and `h / H`.
//!
//! Per-frame products (computed on the GPU, referenced here): the **sky-view LUT** (192 x 108, radiance by view direction relative to
//! the local up and the sun's azimuth, for cameras inside the atmosphere), the **aerial-perspective volume** (32^3: in-scattered
//! radiance and mean transmittance to a distance along the froxel's ray) and the **orbit ray march** (cameras outside the atmosphere,
//! per pixel from the entry point). The ground's own radiance (the bounce) uses the same expression as the lit terrain.
//!
//! Licences: Hillaire, *A Scalable and Production Ready Sky and Atmosphere Rendering Technique*, EGSR 2020 (the technique; its
//! sample code is not adopted). The Earth coefficients are the paper's published table; the Cornette-Shanks phase function is the
//! published formula.

#![allow(clippy::needless_range_loop)] // the RGB channel loops index several parallel arrays

use crate::camera::Camera;
use planet_core::hash::hash_u64s;

pub const PI: f64 = std::f64::consts::PI;

pub const TRANSMITTANCE_W: usize = 256;
pub const TRANSMITTANCE_H: usize = 64;
pub const MS_W: usize = 32;
pub const MS_H: usize = 32;
pub const SKY_W: usize = 192;
pub const SKY_H: usize = 108;
pub const AP_W: usize = 32;
pub const AP_H: usize = 32;
pub const AP_D: usize = 32;

/// Midpoint steps of the optical depth of one transmittance texel (CPU, f64: the table is computed once, so it can afford many).
pub const TRANSMITTANCE_STEPS: usize = 512;
/// Directions (`8 x 8`, uniform on the sphere) and midpoint steps of one multiple-scattering texel.
pub const MS_DIRECTIONS: (usize, usize) = (8, 8);
pub const MS_STEPS: usize = 40;
/// Steps of a sky-view texel's ray (quadratic spacing, dense near the camera).
pub const SKY_STEPS: usize = 32;
/// Steps of the orbit ray march (uniform from the entry point).
pub const ORBIT_STEPS: usize = 40;
/// Most steps of an aerial-perspective texel (`2 (slice + 1)`, at most this).
pub const AP_MAX_STEPS: usize = 64;
/// The distance along the ray of the last aerial-perspective slice; slice `k` is at `AP_MAX_DISTANCE_M * ((k + 1) / 32)^2`.
pub const AP_MAX_DISTANCE_M: f64 = 256_000.0;
/// From this camera altitude (metres) a pixel with geometry is hazed by a march from the camera to the geometry, mixed in with the
/// aerial-perspective volume up to [`CAMERA_MARCH_BLEND_TO_M`], and not by the volume above it: the volume's 32 slices and 32 rows are
/// too coarse for the rays that end hundreds of kilometres away (ATMO-021, SPIKE-20).
pub const CAMERA_MARCH_BLEND_FROM_M: f64 = 6_000.0;
/// Above this altitude the march alone hazes the geometry; between the two the march and the volume are mixed by the smoothstep of the
/// altitude, so the haze does not step when the camera crosses (the volume is 7 to 19 % off the march there). The GPU declares the same
/// values (`CAM_MARCH_BLEND_FROM`, `CAM_MARCH_BLEND_TO`).
pub const CAMERA_MARCH_BLEND_TO_M: f64 = 10_000.0;
/// Steps of that march, dense towards the geometry's end (where the air is thickest): [`CAMERA_MARCH_STEPS_DAY`] while the sun's cosine to
/// the up is at least [`CAMERA_MARCH_TWILIGHT_MU`], [`CAMERA_MARCH_STEPS_TWILIGHT`] below it, where the light crosses the planet's shadow
/// along the ray and fewer steps leave the 5 % of ATMO-007 (the sweep over suns and azimuths, `tests/atmosphere.rs`). The GPU declares
/// the same values (`CAM_MARCH_STEPS_DAY`, `CAM_MARCH_STEPS_TWILIGHT`, `CAM_MARCH_TWILIGHT_MU`).
pub const CAMERA_MARCH_STEPS_DAY: usize = 8;
pub const CAMERA_MARCH_STEPS_TWILIGHT: usize = 12;
pub const CAMERA_MARCH_TWILIGHT_MU: f64 = 0.1;

/// Valid ranges of the look values (the spec LOOK-001 states them).
pub const COEFFICIENT_LIMITS: (f64, f64) = (0.0, 1.0e-3);
pub const SCALE_HEIGHT_LIMITS_M: (f64, f64) = (1.0, 1.0e6);
pub const MIE_G_LIMIT: f64 = 0.99;
pub const OZONE_CENTER_LIMITS_M: (f64, f64) = (0.0, 1.0e6);
pub const OZONE_HALF_WIDTH_LIMITS_M: (f64, f64) = (1.0, 1.0e6);
pub const TOP_HEIGHT_LIMITS_M: (f64, f64) = (1.0e3, 1.0e6);
pub const SUN_RADIUS_LIMITS_RAD: (f64, f64) = (1.0e-5, 0.5);
/// Body radii the atmosphere accepts (metres).
pub const RADIUS_LIMITS_M: (f64, f64) = (1.0e4, 1.0e8);

// ---------------------------------------------------------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------------------------------------------------------

/// The atmosphere of a body as look data (ADR 0040 point 1, ADR 0042 point 6): coefficients per metre, scale heights, the ozone
/// layer, the top of the atmosphere above the surface, the ground albedo and the sun's angular radius. The body radius is world data
/// and arrives with [`Atmosphere::new`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmosphereParams {
    /// Rayleigh scattering at sea level, per metre, RGB.
    pub rayleigh_scattering: [f64; 3],
    pub rayleigh_scale_height_m: f64,
    /// Mie scattering and absorption at sea level, per metre, RGB.
    pub mie_scattering: [f64; 3],
    pub mie_absorption: [f64; 3],
    pub mie_scale_height_m: f64,
    /// Asymmetry of the Cornette-Shanks phase function.
    pub mie_g: f64,
    /// Ozone absorption per metre at the layer's peak, RGB, and the tent profile (peak altitude, half width).
    pub ozone_absorption: [f64; 3],
    pub ozone_center_m: f64,
    pub ozone_half_width_m: f64,
    /// Height of the top of the atmosphere above the reference surface.
    pub top_height_m: f64,
    /// Albedo of the ground the sky bounces light off (the multiple-scattering table and below-horizon rays).
    pub ground_albedo: [f64; 3],
    /// Angular radius of the sun's disc, radians.
    pub sun_angular_radius_rad: f64,
}

impl AtmosphereParams {
    /// Earth, from Hillaire 2020 (table of the paper): Rayleigh 5.802, 13.558, 33.1 per Mm with scale height 8 km; Mie scattering 3.996
    /// and extinction 4.44 per Mm (absorption 0.444) with scale height 1.2 km and g = 0.8; ozone 0.650, 1.881, 0.085 per Mm in a tent
    /// layer from 10 to 40 km peaking at 25 km; top at 100 km; ground albedo 0.3; the sun's disc 0.5357 degrees across.
    pub const EARTH: AtmosphereParams = AtmosphereParams {
        rayleigh_scattering: [5.802e-6, 13.558e-6, 33.1e-6],
        rayleigh_scale_height_m: 8000.0,
        mie_scattering: [3.996e-6, 3.996e-6, 3.996e-6],
        mie_absorption: [0.444e-6, 0.444e-6, 0.444e-6],
        mie_scale_height_m: 1200.0,
        mie_g: 0.8,
        ozone_absorption: [0.650e-6, 1.881e-6, 0.085e-6],
        ozone_center_m: 25_000.0,
        ozone_half_width_m: 15_000.0,
        top_height_m: 100_000.0,
        ground_albedo: [0.3, 0.3, 0.3],
        sun_angular_radius_rad: 0.004_675,
    };

    /// Check every value against its valid range: the first violation gives the key (under `atmosphere.`), the value and why.
    pub fn validate(&self) -> Result<(), (&'static str, String, String)> {
        let vec3 = |key: &'static str, v: [f64; 3], lo: f64, hi: f64, what: &str| -> Result<(), (&'static str, String, String)> {
            for x in v {
                if !x.is_finite() || x < lo || x > hi {
                    return Err((
                        key,
                        format!("{v:?}"),
                        format!("every channel of {what} must be a finite number between {lo:?} and {hi:?}"),
                    ));
                }
            }
            Ok(())
        };
        let one = |key: &'static str, v: f64, lo: f64, hi: f64, what: &str| -> Result<(), (&'static str, String, String)> {
            if !v.is_finite() || v < lo || v > hi {
                return Err((key, format!("{v:?}"), format!("{what} must be a finite number between {lo:?} and {hi:?}")));
            }
            Ok(())
        };
        let (cl, ch) = COEFFICIENT_LIMITS;
        vec3("atmosphere.rayleigh_scattering", self.rayleigh_scattering, cl, ch, "the Rayleigh scattering per metre")?;
        vec3("atmosphere.mie_scattering", self.mie_scattering, cl, ch, "the Mie scattering per metre")?;
        vec3("atmosphere.mie_absorption", self.mie_absorption, cl, ch, "the Mie absorption per metre")?;
        vec3("atmosphere.ozone_absorption", self.ozone_absorption, cl, ch, "the ozone absorption per metre")?;
        vec3("atmosphere.ground_albedo", self.ground_albedo, 0.0, 1.0, "the ground albedo")?;
        let (sl, sh) = SCALE_HEIGHT_LIMITS_M;
        one("atmosphere.rayleigh_scale_height_m", self.rayleigh_scale_height_m, sl, sh, "the Rayleigh scale height in metres")?;
        one("atmosphere.mie_scale_height_m", self.mie_scale_height_m, sl, sh, "the Mie scale height in metres")?;
        one("atmosphere.mie_g", self.mie_g, -MIE_G_LIMIT, MIE_G_LIMIT, "the Mie asymmetry")?;
        one(
            "atmosphere.ozone_center_m",
            self.ozone_center_m,
            OZONE_CENTER_LIMITS_M.0,
            OZONE_CENTER_LIMITS_M.1,
            "the ozone peak altitude in metres",
        )?;
        one(
            "atmosphere.ozone_half_width_m",
            self.ozone_half_width_m,
            OZONE_HALF_WIDTH_LIMITS_M.0,
            OZONE_HALF_WIDTH_LIMITS_M.1,
            "the ozone half width in metres",
        )?;
        one(
            "atmosphere.top_height_m",
            self.top_height_m,
            TOP_HEIGHT_LIMITS_M.0,
            TOP_HEIGHT_LIMITS_M.1,
            "the height of the top of the atmosphere in metres",
        )?;
        one(
            "atmosphere.sun_angular_radius_rad",
            self.sun_angular_radius_rad,
            SUN_RADIUS_LIMITS_RAD.0,
            SUN_RADIUS_LIMITS_RAD.1,
            "the sun's angular radius in radians",
        )
    }

    /// The 22 values as bit patterns in a fixed order (the plan text and the known answers use it).
    fn values(&self) -> [f64; 22] {
        let [a, b, c] = self.rayleigh_scattering;
        let [d, e, f] = self.mie_scattering;
        let [g, h, i] = self.mie_absorption;
        let [j, k, l] = self.ozone_absorption;
        let [m, n, o] = self.ground_albedo;
        [
            a,
            b,
            c,
            self.rayleigh_scale_height_m,
            d,
            e,
            f,
            g,
            h,
            i,
            self.mie_scale_height_m,
            self.mie_g,
            j,
            k,
            l,
            self.ozone_center_m,
            self.ozone_half_width_m,
            self.top_height_m,
            m,
            n,
            o,
            self.sun_angular_radius_rad,
        ]
    }

    /// The values as `f64` bit patterns, in the plan text's order.
    pub fn bits(&self) -> [u64; 22] {
        self.values().map(f64::to_bits)
    }

    pub fn from_bits(b: &[u64; 22]) -> AtmosphereParams {
        let v = b.map(f64::from_bits);
        AtmosphereParams {
            rayleigh_scattering: [v[0], v[1], v[2]],
            rayleigh_scale_height_m: v[3],
            mie_scattering: [v[4], v[5], v[6]],
            mie_absorption: [v[7], v[8], v[9]],
            mie_scale_height_m: v[10],
            mie_g: v[11],
            ozone_absorption: [v[12], v[13], v[14]],
            ozone_center_m: v[15],
            ozone_half_width_m: v[16],
            top_height_m: v[17],
            ground_albedo: [v[18], v[19], v[20]],
            sun_angular_radius_rad: v[21],
        }
    }
}

impl Default for AtmosphereParams {
    fn default() -> Self {
        Self::EARTH
    }
}

fn r32(v: f64) -> f64 {
    f64::from(v as f32)
}

fn r32x3(v: [f64; 3]) -> [f64; 3] {
    v.map(r32)
}

/// The atmosphere of a body: the look parameters and the body radius from the world definition (never from the look file).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub params: AtmosphereParams,
    /// Radius of the reference surface (metres).
    pub radius_m: f64,
    /// `sqrt(H (2R + H))`: the horizon distance of a point at the top of the atmosphere (derived; decided in f64).
    pub rho_top: f64,
}

/// What the medium looks like at one altitude: scattering and extinction coefficients (RGB, per metre), split by species.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Medium {
    pub rayleigh: [f64; 3],
    pub mie: [f64; 3],
    pub scattering: [f64; 3],
    pub extinction: [f64; 3],
}

impl Atmosphere {
    pub fn new(params: AtmosphereParams, radius_m: f64) -> Result<Atmosphere, String> {
        params.validate().map_err(|(k, v, why)| format!("`{k}` = {v}: {why}"))?;
        if !radius_m.is_finite() || radius_m < RADIUS_LIMITS_M.0 || radius_m > RADIUS_LIMITS_M.1 {
            return Err(format!("body radius {radius_m:?} m is outside {:?} to {:?} m", RADIUS_LIMITS_M.0, RADIUS_LIMITS_M.1));
        }
        if params.top_height_m >= radius_m {
            return Err(format!(
                "`atmosphere.top_height_m` = {:?}: the atmosphere must be thinner than the body's radius ({radius_m:?} m)",
                params.top_height_m
            ));
        }
        let h = params.top_height_m;
        Ok(Atmosphere { params, radius_m, rho_top: libm::sqrt(h * (2.0 * radius_m + h)) })
    }

    /// The Earth default on a body of this radius.
    pub fn earth(radius_m: f64) -> Atmosphere {
        Atmosphere::new(AtmosphereParams::EARTH, radius_m).expect("the Earth default is valid")
    }

    pub fn top_m(&self) -> f64 {
        self.params.top_height_m
    }

    /// Every field rounded once to `f32` and widened: the atmosphere as the GPU receives it. The oracle runs on this.
    pub fn rounded_f32(&self) -> Atmosphere {
        let p = self.params;
        Atmosphere {
            params: AtmosphereParams {
                rayleigh_scattering: r32x3(p.rayleigh_scattering),
                rayleigh_scale_height_m: r32(p.rayleigh_scale_height_m),
                mie_scattering: r32x3(p.mie_scattering),
                mie_absorption: r32x3(p.mie_absorption),
                mie_scale_height_m: r32(p.mie_scale_height_m),
                mie_g: r32(p.mie_g),
                ozone_absorption: r32x3(p.ozone_absorption),
                ozone_center_m: r32(p.ozone_center_m),
                ozone_half_width_m: r32(p.ozone_half_width_m),
                top_height_m: r32(p.top_height_m),
                ground_albedo: r32x3(p.ground_albedo),
                sun_angular_radius_rad: r32(p.sun_angular_radius_rad),
            },
            radius_m: r32(self.radius_m),
            rho_top: r32(self.rho_top),
        }
    }

    /// The first six `vec4<f32>` of the GPU block (24 words), see the WGSL `Atmo` struct: Rayleigh coefficients and scale height;
    /// Mie scattering and scale height; Mie absorption and g; ozone absorption and peak altitude; ground albedo and ozone half width;
    /// then `R`, `H`, `rho_top` and the sun's angular radius.
    pub fn static_words(&self) -> [f32; 24] {
        let p = self.params;
        let f = |v: f64| v as f32;
        [
            f(p.rayleigh_scattering[0]),
            f(p.rayleigh_scattering[1]),
            f(p.rayleigh_scattering[2]),
            f(p.rayleigh_scale_height_m),
            f(p.mie_scattering[0]),
            f(p.mie_scattering[1]),
            f(p.mie_scattering[2]),
            f(p.mie_scale_height_m),
            f(p.mie_absorption[0]),
            f(p.mie_absorption[1]),
            f(p.mie_absorption[2]),
            f(p.mie_g),
            f(p.ozone_absorption[0]),
            f(p.ozone_absorption[1]),
            f(p.ozone_absorption[2]),
            f(p.ozone_center_m),
            f(p.ground_albedo[0]),
            f(p.ground_albedo[1]),
            f(p.ground_albedo[2]),
            f(p.ozone_half_width_m),
            f(self.radius_m),
            f(p.top_height_m),
            f(self.rho_top),
            f(p.sun_angular_radius_rad),
        ]
    }

    /// The medium at altitude `h` (below the surface the surface's own values hold).
    pub fn medium(&self, h: f64) -> Medium {
        let p = &self.params;
        let h = h.max(0.0);
        let dr = libm::exp(-h / p.rayleigh_scale_height_m);
        let dm = libm::exp(-h / p.mie_scale_height_m);
        let dz = (1.0 - (h - p.ozone_center_m).abs() / p.ozone_half_width_m).max(0.0);
        let mut rayleigh = [0.0; 3];
        let mut mie = [0.0; 3];
        let mut scattering = [0.0; 3];
        let mut extinction = [0.0; 3];
        for k in 0..3 {
            rayleigh[k] = p.rayleigh_scattering[k] * dr;
            mie[k] = p.mie_scattering[k] * dm;
            scattering[k] = rayleigh[k] + mie[k];
            extinction[k] = scattering[k] + p.mie_absorption[k] * dm + p.ozone_absorption[k] * dz;
        }
        Medium { rayleigh, mie, scattering, extinction }
    }

    /// Smooth shadow of the planet on a point at altitude `h` whose sun has cosine `mu_sun` to the local up: 0 once the sun is below the
    /// horizon by more than its angular radius, 1 once above it by more (a smoothstep across the penumbra, so the terminator is
    /// continuous and a camera moving by a millimetre never makes a sample jump).
    pub fn earth_shadow(&self, h: f64, mu_sun: f64) -> f64 {
        let h = h.clamp(0.0, self.top_m());
        let c = h * (2.0 * self.radius_m + h);
        let mu_h = -libm::sqrt(c) / (self.radius_m + h);
        // The penumbra is the sun's angular radius; in cosine terms at the horizon of a point at radius r it is w R / r.
        let w = self.params.sun_angular_radius_rad * self.radius_m / (self.radius_m + h);
        smoothstep(-w, w, mu_sun - mu_h)
    }
}

pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rayleigh phase function (per steradian).
pub fn rayleigh_phase(cos_gamma: f64) -> f64 {
    3.0 / (16.0 * PI) * (1.0 + cos_gamma * cos_gamma)
}

/// Cornette-Shanks phase function with asymmetry `g` (per steradian; integrates to 1 over the sphere).
pub fn cornette_shanks_phase(g: f64, cos_gamma: f64) -> f64 {
    let k = 3.0 / (8.0 * PI) * (1.0 - g * g) / (2.0 + g * g);
    let d = 1.0 + g * g - 2.0 * g * cos_gamma;
    k * (1.0 + cos_gamma * cos_gamma) / (d * libm::sqrt(d))
}

// ---------------------------------------------------------------------------------------------------------------------------
// Ray geometry: the altitude-relative primitives (ADR 0040 point 5)
// ---------------------------------------------------------------------------------------------------------------------------

/// `r^2 - R^2` of a point at altitude `h`, formed without the difference.
pub fn c_of(radius: f64, h: f64) -> f64 {
    h * (2.0 * radius + h)
}

/// `r^2 - (R + H)^2` of a point at altitude `h` (negative inside the atmosphere), formed without the difference.
pub fn c_top_of(radius: f64, top: f64, h: f64) -> f64 {
    (h - top) * (2.0 * radius + h + top)
}

/// Distance from a point inside the atmosphere (`c_top <= 0`) to the top along a ray with `r mu = rmu`.
pub fn distance_to_top(rmu: f64, c_top: f64) -> f64 {
    let sq = libm::sqrt((rmu * rmu - c_top).max(0.0));
    if rmu >= 0.0 {
        let den = rmu + sq;
        if den > 0.0 {
            -c_top / den
        } else {
            0.0
        }
    } else {
        -rmu + sq
    }
}

/// Distance to the ground (the reference sphere) along a downward ray, or `None` when it passes above it. `c = r^2 - R^2`.
pub fn distance_to_ground(rmu: f64, c: f64) -> Option<f64> {
    if rmu >= 0.0 {
        return None;
    }
    let d = rmu * rmu - c;
    if d < 0.0 {
        return None;
    }
    Some(c / (-rmu + libm::sqrt(d)))
}

/// The start of a ray for the march: altitude `h`, `r`, `c = r^2 - R^2`, `r mu` (the cosine of the ray to the local up times `r`) and
/// `r mu_sun` (the same for the sun). A sample at distance `s` along the ray has `r_s^2 = r^2 + s (s + 2 r mu)`,
/// `h_s = (c + s (s + 2 r mu)) / (r_s + R)` and `mu_sun_s = (r mu_sun + s cos(ray, sun)) / r_s`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayOrigin {
    pub h: f64,
    pub r: f64,
    pub c: f64,
    pub rmu: f64,
    pub r_musun: f64,
}

impl RayOrigin {
    /// Altitude and radius of the sample at distance `s`.
    pub fn at(&self, radius: f64, s: f64) -> (f64, f64) {
        let q = s * (s + 2.0 * self.rmu);
        let r_s = libm::sqrt((self.r * self.r + q).max(0.0));
        (((self.c + q) / (r_s + radius)).max(0.0), r_s)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Static look-up tables
// ---------------------------------------------------------------------------------------------------------------------------

/// The static tables of one body: optical depth (transmittance LUT) and the multiple-scattering factor. Texel `(x, y)` is at
/// `y * width + x`.
#[derive(Clone, Debug, PartialEq)]
pub struct StaticLuts {
    pub optical_depth: Vec<[f64; 3]>,
    pub multiscatter: Vec<[f64; 3]>,
}

fn bilerp(tex: &[[f64; 3]], w: usize, h: usize, px: f64, py: f64) -> [f64; 3] {
    let px = px.clamp(0.0, (w - 1) as f64);
    let py = py.clamp(0.0, (h - 1) as f64);
    let x0 = (libm::floor(px) as usize).min(w - 2);
    let y0 = (libm::floor(py) as usize).min(h - 2);
    let (fx, fy) = (px - x0 as f64, py - y0 as f64);
    let at = |x: usize, y: usize| tex[y * w + x];
    let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
    let mut out = [0.0; 3];
    for k in 0..3 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bot = c[k] + (d[k] - c[k]) * fx;
        out[k] = top + (bot - top) * fy;
    }
    out
}

impl Atmosphere {
    /// The transmittance LUT coordinates `(x_mu, x_r)` of a point at altitude `h` and a ray with cosine `mu` to the local up.
    pub fn transmittance_coords(&self, h: f64, mu: f64) -> (f64, f64) {
        let top = self.top_m();
        let h = h.clamp(0.0, top);
        let r = self.radius_m + h;
        let rho = libm::sqrt(c_of(self.radius_m, h));
        let d = distance_to_top(r * mu.clamp(-1.0, 1.0), c_top_of(self.radius_m, top, h));
        let d_min = top - h;
        let d_max = rho + self.rho_top;
        (((d - d_min) / (d_max - d_min)).clamp(0.0, 1.0), (rho / self.rho_top).clamp(0.0, 1.0))
    }

    /// The altitude and ray cosine a transmittance texel stands for.
    pub fn transmittance_texel(&self, ix: usize, iy: usize) -> (f64, f64) {
        let x_mu = ix as f64 / (TRANSMITTANCE_W - 1) as f64;
        let x_r = iy as f64 / (TRANSMITTANCE_H - 1) as f64;
        let (big_r, top) = (self.radius_m, self.top_m());
        let rho = x_r * self.rho_top;
        let c = rho * rho;
        let h = (c / (big_r + libm::sqrt(big_r * big_r + c))).min(top);
        let r = big_r + h;
        let d_min = top - h;
        let d_max = rho + self.rho_top;
        let d = d_min + x_mu * (d_max - d_min);
        let mu = if d <= 0.0 { 1.0 } else { ((-c_top_of(big_r, top, h) - d * d) / (2.0 * r * d)).clamp(-1.0, 1.0) };
        (h, mu)
    }

    /// The optical depth (RGB) from altitude `h` along `mu` to the top of the atmosphere, bilinear in the table.
    pub fn optical_depth(&self, luts: &StaticLuts, h: f64, mu: f64) -> [f64; 3] {
        let (x_mu, x_r) = self.transmittance_coords(h, mu);
        bilerp(
            &luts.optical_depth,
            TRANSMITTANCE_W,
            TRANSMITTANCE_H,
            x_mu * (TRANSMITTANCE_W - 1) as f64,
            x_r * (TRANSMITTANCE_H - 1) as f64,
        )
    }

    /// The transmittance from altitude `h` along `mu` to the top of the atmosphere.
    pub fn transmittance(&self, luts: &StaticLuts, h: f64, mu: f64) -> [f64; 3] {
        self.optical_depth(luts, h, mu).map(|t| libm::exp(-t))
    }

    /// The multiple-scattering factor `psi_ms` (RGB) at altitude `h` with the sun at cosine `mu_sun`, bilinear in the table.
    pub fn multiscatter(&self, luts: &StaticLuts, h: f64, mu_sun: f64) -> [f64; 3] {
        let x = (mu_sun.clamp(-1.0, 1.0) + 1.0) * 0.5;
        let y = (h / self.top_m()).clamp(0.0, 1.0);
        bilerp(&luts.multiscatter, MS_W, MS_H, x * (MS_W - 1) as f64, y * (MS_H - 1) as f64)
    }

    /// The optical depth of the ray of a transmittance texel by `steps` midpoint steps; `steps` large gives the converged integral
    /// that the table is measured against (ATMO-007).
    pub fn optical_depth_integral(&self, h: f64, mu: f64, steps: usize) -> [f64; 3] {
        let (big_r, top) = (self.radius_m, self.top_m());
        let r = big_r + h;
        let origin = RayOrigin { h, r, c: c_of(big_r, h), rmu: r * mu, r_musun: 0.0 };
        let d = distance_to_top(origin.rmu, c_top_of(big_r, top, h));
        let ds = d / steps as f64;
        let mut tau = [0.0; 3];
        for i in 0..steps {
            let (h_s, _) = origin.at(big_r, (i as f64 + 0.5) * ds);
            let m = self.medium(h_s);
            for k in 0..3 {
                tau[k] += m.extinction[k] * ds;
            }
        }
        tau
    }
}

/// Run `f(row, out_row)` for every row of `out`, on `threads` scoped threads over contiguous chunks. Each row is computed by a pure
/// function of its index, so the table does not depend on the thread count (CON-14).
fn par_rows<F: Fn(usize, &mut [[f64; 3]]) + Sync>(out: &mut [[f64; 3]], row_len: usize, threads: usize, f: F) {
    let rows = out.len() / row_len;
    let threads = threads.clamp(1, rows.max(1));
    let per = rows.div_ceil(threads);
    std::thread::scope(|s| {
        for (chunk_index, chunk) in out.chunks_mut(per * row_len).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (k, row) in chunk.chunks_mut(row_len).enumerate() {
                    f(chunk_index * per + k, row);
                }
            });
        }
    });
}

impl StaticLuts {
    /// Compute both tables on `threads` threads (the result is independent of the count).
    pub fn generate_with_threads(atm: &Atmosphere, threads: usize) -> StaticLuts {
        let mut optical_depth = vec![[0.0; 3]; TRANSMITTANCE_W * TRANSMITTANCE_H];
        par_rows(&mut optical_depth, TRANSMITTANCE_W, threads, |iy, row| {
            for (ix, texel) in row.iter_mut().enumerate() {
                let (h, mu) = atm.transmittance_texel(ix, iy);
                *texel = atm.optical_depth_integral(h, mu, TRANSMITTANCE_STEPS);
            }
        });
        let mut luts = StaticLuts { optical_depth, multiscatter: vec![[0.0; 3]; MS_W * MS_H] };
        let mut ms = vec![[0.0; 3]; MS_W * MS_H];
        par_rows(&mut ms, MS_W, threads, |iy, row| {
            for (ix, texel) in row.iter_mut().enumerate() {
                let mu_sun = -1.0 + 2.0 * ix as f64 / (MS_W - 1) as f64;
                let h = atm.top_m() * iy as f64 / (MS_H - 1) as f64;
                *texel = atm.multiscatter_texel(&luts, h, mu_sun);
            }
        });
        luts.multiscatter = ms;
        luts
    }

    /// Both tables on all available threads.
    pub fn generate(atm: &Atmosphere) -> StaticLuts {
        Self::generate_with_threads(atm, std::thread::available_parallelism().map_or(1, |n| n.get()))
    }

    /// Hash of every value's bit pattern (the known answer of the tables).
    pub fn hash(&self) -> u64 {
        let bits: Vec<u64> = self.optical_depth.iter().chain(&self.multiscatter).flat_map(|t| t.map(f64::to_bits)).collect();
        hash_u64s(0x6c75_7473, &bits)
    }

    /// The tables as the GPU holds them (every value rounded once to `f32`) read back as `f64`.
    pub fn rounded_f32(&self) -> StaticLuts {
        StaticLuts {
            optical_depth: self.optical_depth.iter().map(|t| r32x3(*t)).collect(),
            multiscatter: self.multiscatter.iter().map(|t| r32x3(*t)).collect(),
        }
    }

    /// `Rgba32Float` texels of a table for the upload (alpha 1).
    pub fn rgba_f32(table: &[[f64; 3]]) -> Vec<f32> {
        table.iter().flat_map(|t| [t[0] as f32, t[1] as f32, t[2] as f32, 1.0]).collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The ray march
// ---------------------------------------------------------------------------------------------------------------------------

/// How a march of length `s_end` is cut into steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepLayout {
    /// `n` equal steps.
    Uniform(usize),
    /// `n` steps whose boundaries are `s_end (i / n)^2`: dense near the origin.
    Quadratic(usize),
    /// `n` steps whose boundaries are `s_end (1 - (1 - i / n)^2)`: dense near the far end of the march.
    EndDense(usize),
}

impl StepLayout {
    pub fn count(self) -> usize {
        match self {
            StepLayout::Uniform(n) | StepLayout::Quadratic(n) | StepLayout::EndDense(n) => n,
        }
    }

    /// The start and end of step `i`.
    pub fn bounds(self, s_end: f64, i: usize) -> (f64, f64) {
        match self {
            StepLayout::Uniform(n) => (s_end * i as f64 / n as f64, s_end * (i + 1) as f64 / n as f64),
            StepLayout::Quadratic(n) => {
                let f = |k: usize| {
                    let x = k as f64 / n as f64;
                    s_end * x * x
                };
                (f(i), f(i + 1))
            }
            StepLayout::EndDense(n) => {
                let f = |k: usize| {
                    let x = 1.0 - k as f64 / n as f64;
                    s_end * (1.0 - x * x)
                };
                (f(i), f(i + 1))
            }
        }
    }
}

/// Which scattering a march adds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scatter {
    /// The real phase functions for the angle between the ray and the sun, and the multiple-scattering term.
    Physical { cos_gamma: f64 },
    /// An isotropic phase and no multiple-scattering term (the second-order estimate of the multiple-scattering table itself).
    Isotropic,
}

/// What a march returns: radiance in-scattered along the ray, the transmittance of the whole ray and `f_ms`, the part of the radiance
/// that a unit isotropic source along the ray would scatter (the multiple-scattering table's denominator).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct March {
    pub radiance: [f64; 3],
    pub transmittance: [f64; 3],
    pub transfer: [f64; 3],
}

impl Atmosphere {
    /// March from `o` along a ray whose cosine to the sun is `cos_vs`, for `s_end` metres, in the layout's steps. Sun illuminance is 1.
    pub fn march(&self, luts: &StaticLuts, o: &RayOrigin, cos_vs: f64, s_end: f64, layout: StepLayout, scatter: Scatter) -> March {
        let (big_r, top) = (self.radius_m, self.top_m());
        let p = &self.params;
        let mut radiance = [0.0; 3];
        let mut transfer = [0.0; 3];
        let mut thr = [1.0; 3];
        let (phase_r, phase_m) = match scatter {
            Scatter::Physical { cos_gamma } => (rayleigh_phase(cos_gamma), cornette_shanks_phase(p.mie_g, cos_gamma)),
            Scatter::Isotropic => (1.0 / (4.0 * PI), 1.0 / (4.0 * PI)),
        };
        for i in 0..layout.count() {
            let (a, b) = layout.bounds(s_end, i);
            let ds = b - a;
            if ds <= 0.0 {
                continue;
            }
            let s = 0.5 * (a + b);
            let (h_s, r_s) = o.at(big_r, s);
            let medium = self.medium(h_s);
            let h_c = h_s.min(top);
            let mu_sun_s = if r_s > 0.0 { ((o.r_musun + s * cos_vs) / r_s).clamp(-1.0, 1.0) } else { 1.0 };
            let t_sun = self.transmittance(luts, h_c, mu_sun_s);
            let shadow = self.earth_shadow(h_c, mu_sun_s);
            let ms = match scatter {
                Scatter::Physical { .. } => self.multiscatter(luts, h_c, mu_sun_s),
                Scatter::Isotropic => [0.0; 3],
            };
            for k in 0..3 {
                let single = (medium.rayleigh[k] * phase_r + medium.mie[k] * phase_m) * t_sun[k] * shadow;
                let s_in = single + ms[k] * medium.scattering[k];
                let ext = medium.extinction[k];
                let tr = libm::exp(-ext * ds);
                // (1 - exp(-x)) / x without cancellation for the thin steps of a thin medium.
                let x = ext * ds;
                let phi = if x < 1.0e-3 { 1.0 - x / 2.0 + x * x / 6.0 } else { (1.0 - tr) / x };
                let (s_int, f_int) = (s_in * ds * phi, medium.scattering[k] * ds * phi);
                radiance[k] += thr[k] * s_int;
                transfer[k] += thr[k] * f_int;
                thr[k] *= tr;
            }
        }
        March { radiance, transmittance: thr, transfer }
    }

    /// The radiance leaving a ground point (the reference surface) whose sun has cosine `mu_sun_g` to the local up: albedo times the
    /// direct sunlight (transmitted, shadowed, Lambert) and the multiple-scattering ambient. The terrain shader's atmosphere lighting
    /// is this expression with the surface's own albedo and normal.
    pub fn ground_radiance(&self, luts: &StaticLuts, mu_sun_g: f64) -> [f64; 3] {
        let t = self.transmittance(luts, 0.0, mu_sun_g);
        let shadow = self.earth_shadow(0.0, mu_sun_g);
        let ms = self.multiscatter(luts, 0.0, mu_sun_g);
        let direct = shadow * mu_sun_g.max(0.0) / PI;
        let a = self.params.ground_albedo;
        [a[0] * (t[0] * direct + ms[0]), a[1] * (t[1] * direct + ms[1]), a[2] * (t[2] * direct + ms[2])]
    }

    /// The sun's light on a surface with `normal` at altitude `h_alt` where the local `up` and the `sun` are planet-fixed unit vectors:
    /// the transmitted, shadowed Lambert term plus the multiple-scattering ambient on the sky hemisphere (RGB, a unit of illuminance
    /// 1). The lit terrain is the surface's albedo times this; the terrain shader's `atm_terrain_lighting` is this expression.
    pub fn terrain_lighting(&self, luts: &StaticLuts, normal: [f64; 3], up: [f64; 3], h_alt: f64, sun: [f64; 3]) -> [f64; 3] {
        let n = normalize3(normal);
        let ndl = dot3(n, sun).max(0.0);
        let mu_s = dot3(up, sun).clamp(-1.0, 1.0);
        let h = h_alt.clamp(0.0, self.top_m());
        let t = self.transmittance(luts, h, mu_s);
        let shadow = self.earth_shadow(h, mu_s);
        let ms = self.multiscatter(luts, h, mu_s);
        let hemi = 0.5 + 0.5 * dot3(n, up).clamp(-1.0, 1.0);
        let direct = shadow * ndl / PI;
        [t[0] * direct + ms[0] * hemi, t[1] * direct + ms[1] * hemi, t[2] * direct + ms[2] * hemi]
    }

    /// `psi_ms` at one texel: Hillaire's isotropic second-order radiance over `1 - f_ms`, by `MS_DIRECTIONS` uniform directions and
    /// `MS_STEPS` steps (the ground bounce included).
    pub fn multiscatter_texel(&self, luts: &StaticLuts, h: f64, mu_sun: f64) -> [f64; 3] {
        let big_r = self.radius_m;
        let top = self.top_m();
        let r = big_r + h;
        let c = c_of(big_r, h);
        let c_top = c_top_of(big_r, top, h);
        let mu_h = -libm::sqrt(c) / r;
        let sin_sun = libm::sqrt((1.0 - mu_sun * mu_sun).max(0.0));
        let (nt, np) = MS_DIRECTIONS;
        let mut l2 = [0.0; 3];
        let mut f_ms = [0.0; 3];
        for i in 0..nt {
            let mu = 1.0 - 2.0 * (i as f64 + 0.5) / nt as f64;
            let sin_t = libm::sqrt((1.0 - mu * mu).max(0.0));
            for j in 0..np {
                let phi = 2.0 * PI * (j as f64 + 0.5) / np as f64;
                let cos_vs = sin_t * libm::cos(phi) * sin_sun + mu * mu_sun;
                let o = RayOrigin { h, r, c, rmu: r * mu, r_musun: r * mu_sun };
                let ground = if mu < mu_h { distance_to_ground(o.rmu, c) } else { None };
                let s_end = ground.unwrap_or_else(|| distance_to_top(o.rmu, c_top));
                let m = self.march(luts, &o, cos_vs, s_end, StepLayout::Uniform(MS_STEPS), Scatter::Isotropic);
                let mut l = m.radiance;
                if ground.is_some() {
                    let (_, r_g) = o.at(big_r, s_end);
                    let mu_g = ((o.r_musun + s_end * cos_vs) / r_g.max(1.0)).clamp(-1.0, 1.0);
                    let t_sun = self.transmittance(luts, 0.0, mu_g);
                    let shadow = self.earth_shadow(0.0, mu_g);
                    let direct = shadow * mu_g.max(0.0) / PI;
                    for k in 0..3 {
                        l[k] += m.transmittance[k] * self.params.ground_albedo[k] * t_sun[k] * direct;
                    }
                }
                let weight = 1.0 / (nt * np) as f64;
                for k in 0..3 {
                    l2[k] += l[k] * weight;
                    f_ms[k] += m.transfer[k] * weight;
                }
            }
        }
        // A medium that scatters and never absorbs and is optically thick has f_ms -> 1: the factor is capped at 1000.
        let g = |k: usize| l2[k] / (1.0 - f_ms[k]).max(1.0e-3);
        [g(0), g(1), g(2)]
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Per-frame geometry (decided on the CPU in f64, rounded once)
// ---------------------------------------------------------------------------------------------------------------------------

/// What the GPU passes need to know about the camera and the sun for one frame, decided in f64 from the plan (CON-03): altitude `h`,
/// `r`, `c`, `c_top`, the horizon (its cosine and angles), the local up and the sun's cosine to it, and the camera's ray frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameGeometry {
    pub h: f64,
    pub r: f64,
    pub c: f64,
    pub c_top: f64,
    /// `sqrt(c)`: the distance to the horizon.
    pub rho: f64,
    /// Cosine of the horizon's zenith angle (`-rho / r`; ray below it hit the ground).
    pub mu_h: f64,
    /// The zenith angle of the horizon and the angle from there to the nadir.
    pub theta_h: f64,
    pub beta: f64,
    pub up: [f64; 3],
    pub mu_sun: f64,
    pub sun: [f64; 3],
    pub sin_sun: f64,
    /// The horizontal axes of the sky-view azimuth: `e1` points from the up towards the sun (the sun's horizontal direction), `e2`
    /// is `up x e1`. A view direction's azimuth from the sun is `atan2(|dir . e2|, dir . e1)`, well conditioned near the sun
    /// where `cos(azimuth)` is not.
    pub sun_e1: [f64; 3],
    pub sun_e2: [f64; 3],
    /// The camera's right and up vectors scaled by the tangent of half the field of view, and its forward vector.
    pub right: [f64; 3],
    pub upv: [f64; 3],
    pub fwd: [f64; 3],
    pub near: f64,
    /// Whether the camera is inside the atmosphere (sky view and aerial perspective) or outside (the orbit march).
    pub inside: bool,
    /// `sqrt(c_top)` outside the atmosphere (0 inside): with `rho` and `r` it gives the orbit march's discriminants in the factored,
    /// limb-stable forms `(r mu - q)(r mu + q)` and `(r mu - rho)(r mu + rho)`. The GPU receives each of `r`, `rho` and `q_top` as a
    /// high and a low `f32` (a double-single value) so the factor `r mu + q` near the tangent keeps its digits.
    pub q_top: f64,
    /// The distance of the last aerial-perspective slice for this frame: `max(AP_MAX_DISTANCE_M, rho)`, the distance to the horizon
    /// when that is farther (ATMO-006): terrain at the horizon of a camera high in the atmosphere is hazed by the volume to its own
    /// distance, as the orbit march hazes it from outside. Exactly `AP_MAX_DISTANCE_M` below about 5 km altitude.
    pub ap_range: f64,
}

/// A value as the GPU holds it when sent as a high and a low `f32`: their sum.
fn double_single(v: f64) -> f64 {
    let hi = v as f32;
    f64::from(hi) + f64::from((v - f64::from(hi)) as f32)
}

fn low_part(v: f64) -> f32 {
    (v - f64::from(v as f32)) as f32
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize3(a: [f64; 3]) -> [f64; 3] {
    let l = libm::sqrt(dot3(a, a));
    [a[0] / l, a[1] / l, a[2] / l]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

impl FrameGeometry {
    /// The frame's geometry for `camera` and the sun's direction (unit, planet-fixed). The camera's altitude is measured above the
    /// reference surface and clamped at 0 (a seabed below it is under the water level).
    pub fn new(atm: &Atmosphere, camera: &Camera, sun: [f64; 3]) -> FrameGeometry {
        let p = camera.position.0;
        let dist = libm::sqrt(p.x * p.x + p.y * p.y + p.z * p.z);
        let up = [p.x / dist, p.y / dist, p.z / dist];
        let h = (dist - atm.radius_m).max(0.0);
        let (rt, ut, ft) = camera.ray_frame();
        let sun = normalize3(sun);
        FrameGeometry::from_parts(atm, h, up, sun, [rt.x, rt.y, rt.z], [ut.x, ut.y, ut.z], [ft.x, ft.y, ft.z], camera.near_m)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        atm: &Atmosphere,
        h: f64,
        up: [f64; 3],
        sun: [f64; 3],
        right: [f64; 3],
        upv: [f64; 3],
        fwd: [f64; 3],
        near: f64,
    ) -> FrameGeometry {
        let (big_r, top) = (atm.radius_m, atm.top_m());
        let r = big_r + h;
        let c = c_of(big_r, h);
        let rho = libm::sqrt(c);
        let mu_sun = dot3(up, sun).clamp(-1.0, 1.0);
        let beta = libm::acos((rho / r).clamp(0.0, 1.0));
        let sin_sun = libm::sqrt((1.0 - mu_sun * mu_sun).max(0.0));
        // The azimuth axes: towards the sun's horizontal direction, or (sun at the zenith or nadir, where the azimuth means nothing)
        // towards the world axis least aligned with the up.
        let horizontal = if sin_sun > 1.0e-9 {
            [sun[0] - mu_sun * up[0], sun[1] - mu_sun * up[1], sun[2] - mu_sun * up[2]]
        } else {
            let a = [up[0].abs(), up[1].abs(), up[2].abs()];
            let axis = if a[0] <= a[1] && a[0] <= a[2] {
                [1.0, 0.0, 0.0]
            } else if a[1] <= a[2] {
                [0.0, 1.0, 0.0]
            } else {
                [0.0, 0.0, 1.0]
            };
            let d = dot3(axis, up);
            [axis[0] - d * up[0], axis[1] - d * up[1], axis[2] - d * up[2]]
        };
        let sun_e1 = normalize3(horizontal);
        let sun_e2 = cross3(up, sun_e1);
        FrameGeometry {
            h,
            r,
            c,
            c_top: c_top_of(big_r, top, h),
            rho,
            mu_h: -rho / r,
            theta_h: PI - beta,
            beta,
            up,
            mu_sun,
            sun,
            sin_sun,
            sun_e1,
            sun_e2,
            right,
            upv,
            fwd,
            near,
            inside: h < top,
            q_top: libm::sqrt(c_top_of(big_r, top, h).max(0.0)),
            ap_range: if h < top { AP_MAX_DISTANCE_M.max(rho) } else { AP_MAX_DISTANCE_M },
        }
    }

    /// Every value rounded once to `f32` and widened: the frame as the GPU receives it.
    pub fn rounded_f32(&self) -> FrameGeometry {
        let v3 = |a: [f64; 3]| a.map(r32);
        FrameGeometry {
            h: r32(self.h),
            r: double_single(self.r),
            c: r32(self.c),
            c_top: r32(self.c_top),
            rho: double_single(self.rho),
            q_top: double_single(self.q_top),
            mu_h: r32(self.mu_h),
            theta_h: r32(self.theta_h),
            beta: r32(self.beta),
            up: v3(self.up),
            mu_sun: r32(self.mu_sun),
            sun: v3(self.sun),
            sin_sun: r32(self.sin_sun),
            sun_e1: v3(self.sun_e1),
            sun_e2: v3(self.sun_e2),
            right: v3(self.right),
            upv: v3(self.upv),
            fwd: v3(self.fwd),
            near: r32(self.near),
            inside: self.inside,
            ap_range: r32(self.ap_range),
        }
    }

    /// The ten `vec4<f32>` of the per-frame part of the GPU block (40 words) after [`Atmosphere::static_words`]: `(h, r, c, c_top)`,
    /// `(rho, mu_h, theta_h, beta)`, `(up, mu_sun)`, `(sun, sin_sun)`, `(e1)`, `(e2)`, `(right)`, `(up of the camera)`,
    /// `(forward, near)` and `(pre-exposure factor, inside, aerial-perspective range, debug view)`.
    pub fn words(&self, pre_exposure_factor: f32, debug_view: u32) -> [f32; 40] {
        let f = |v: f64| v as f32;
        [
            f(self.h),
            f(self.r),
            f(self.c),
            f(self.c_top),
            f(self.rho),
            f(self.mu_h),
            f(self.theta_h),
            f(self.beta),
            f(self.up[0]),
            f(self.up[1]),
            f(self.up[2]),
            f(self.mu_sun),
            f(self.sun[0]),
            f(self.sun[1]),
            f(self.sun[2]),
            f(self.sin_sun),
            f(self.sun_e1[0]),
            f(self.sun_e1[1]),
            f(self.sun_e1[2]),
            low_part(self.r),
            f(self.sun_e2[0]),
            f(self.sun_e2[1]),
            f(self.sun_e2[2]),
            low_part(self.rho),
            f(self.right[0]),
            f(self.right[1]),
            f(self.right[2]),
            f(self.q_top),
            f(self.upv[0]),
            f(self.upv[1]),
            f(self.upv[2]),
            low_part(self.q_top),
            f(self.fwd[0]),
            f(self.fwd[1]),
            f(self.fwd[2]),
            f(self.near),
            pre_exposure_factor,
            if self.inside { 1.0 } else { 0.0 },
            f(self.ap_range),
            f32::from_bits(debug_view),
        ]
    }

    fn origin(&self, mu: f64) -> RayOrigin {
        RayOrigin { h: self.h, r: self.r, c: self.c, rmu: self.r * mu, r_musun: self.r * self.mu_sun }
    }

    /// The direction cosines of a view ray in planet-fixed axes: to the local up and to the sun.
    pub fn view_cosines(&self, dir: [f64; 3]) -> (f64, f64) {
        (dot3(dir, self.up).clamp(-1.0, 1.0), dot3(dir, self.sun).clamp(-1.0, 1.0))
    }

    /// How far a ray from the camera (inside the atmosphere) goes before it leaves the atmosphere or reaches the ground, and whether
    /// it reaches the ground.
    pub fn inside_ray_end(&self, mu: f64) -> (f64, bool) {
        let rmu = self.r * mu;
        if mu < self.mu_h {
            if let Some(t) = distance_to_ground(rmu, self.c) {
                return (t, true);
            }
        }
        (distance_to_top(rmu, self.c_top), false)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Sky-view LUT
// ---------------------------------------------------------------------------------------------------------------------------

impl FrameGeometry {
    /// The view direction a sky-view texel stands for, as `(mu, cos(view, sun), reaches the ground)`: rows run from the zenith (0)
    /// through the horizon (0.5, the rows are finest there) to the nadir (1), columns from the sun's azimuth (0) to the opposite
    /// azimuth (1, finest near the sun). Texel `i` sits at `i / (N - 1)`.
    pub fn sky_view_texel_dir(&self, ix: usize, iy: usize) -> (f64, f64, bool) {
        let u = ix as f64 / (SKY_W - 1) as f64;
        let v = iy as f64 / (SKY_H - 1) as f64;
        let phi = PI * u * u;
        let (ground, theta) = if v < 0.5 {
            let q = 1.0 - 2.0 * v;
            (false, (1.0 - q * q) * self.theta_h)
        } else {
            let q = 2.0 * v - 1.0;
            (true, self.theta_h + self.beta * q * q)
        };
        let mu = libm::cos(theta).clamp(-1.0, 1.0);
        let sin_t = libm::sin(theta).abs();
        let cos_vs = sin_t * libm::cos(phi) * self.sin_sun + mu * self.mu_sun;
        (mu, cos_vs.clamp(-1.0, 1.0), ground)
    }

    /// The planet-fixed unit direction of a sky-view texel and whether it reaches the ground (the texel's `mu` and azimuth made into a
    /// vector with the frame's axes).
    pub fn sky_view_texel_ray(&self, ix: usize, iy: usize) -> ([f64; 3], bool) {
        let (mu, _, ground) = self.sky_view_texel_dir(ix, iy);
        let u = ix as f64 / (SKY_W - 1) as f64;
        let phi = PI * u * u;
        let sin_t = libm::sqrt((1.0 - mu * mu).max(0.0));
        let (c, s) = (libm::cos(phi), libm::sin(phi));
        let mut d = [0.0; 3];
        for k in 0..3 {
            d[k] = mu * self.up[k] + sin_t * (c * self.sun_e1[k] + s * self.sun_e2[k]);
        }
        (d, ground)
    }

    /// The sky-view coordinates `(u, v)` of a view direction (planet-fixed, unit).
    pub fn sky_view_coords(&self, dir: [f64; 3]) -> (f64, f64) {
        let mu = dot3(dir, self.up).clamp(-1.0, 1.0);
        let phi = libm::atan2(dot3(dir, self.sun_e2).abs(), dot3(dir, self.sun_e1));
        let u = libm::sqrt(phi / PI);
        let theta = libm::acos(mu.clamp(-1.0, 1.0));
        let v = if mu > self.mu_h {
            0.5 * (1.0 - libm::sqrt((1.0 - theta / self.theta_h).max(0.0)))
        } else {
            0.5 + 0.5 * libm::sqrt(((theta - self.theta_h) / self.beta).clamp(0.0, 1.0))
        };
        (u, v)
    }

    /// The radiance of a sky-view texel by the march of `SKY_STEPS` quadratic steps, with the ground's radiance for texels below the
    /// horizon. Sun illuminance 1.
    pub fn sky_view_texel(&self, atm: &Atmosphere, luts: &StaticLuts, ix: usize, iy: usize) -> [f64; 3] {
        let (mu, cos_vs, ground) = self.sky_view_texel_dir(ix, iy);
        let o = self.origin(mu);
        let (s_end, hits) = if ground {
            match distance_to_ground(o.rmu, self.c) {
                Some(t) => (t, true),
                None => (distance_to_top(o.rmu, self.c_top), false),
            }
        } else {
            (distance_to_top(o.rmu, self.c_top), false)
        };
        let m = atm.march(luts, &o, cos_vs, s_end, StepLayout::Quadratic(SKY_STEPS), Scatter::Physical { cos_gamma: cos_vs });
        let mut l = m.radiance;
        if hits {
            let (_, r_g) = o.at(atm.radius_m, s_end);
            let mu_g = ((o.r_musun + s_end * cos_vs) / r_g.max(1.0)).clamp(-1.0, 1.0);
            let g = atm.ground_radiance(luts, mu_g);
            for k in 0..3 {
                l[k] += m.transmittance[k] * g[k];
            }
        }
        l
    }

    /// The sky radiance in a view direction from the sky-view table (manual bilinear).
    pub fn sky_view_lookup(&self, table: &[[f64; 3]], dir: [f64; 3]) -> [f64; 3] {
        let (u, v) = self.sky_view_coords(dir);
        bilerp(table, SKY_W, SKY_H, u * (SKY_W - 1) as f64, v * (SKY_H - 1) as f64)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Aerial-perspective volume
// ---------------------------------------------------------------------------------------------------------------------------

/// The distance along the ray of aerial-perspective slice `k` for the base range (a camera below about 5 km altitude); a frame's own
/// range is [`FrameGeometry::ap_range`], see [`FrameGeometry::ap_slice_distance`].
pub fn ap_slice_distance(k: usize) -> f64 {
    ap_slice_distance_for(AP_MAX_DISTANCE_M, k)
}

/// The distance along the ray of slice `k` of a volume whose last slice is at `range`.
pub fn ap_slice_distance_for(range: f64, k: usize) -> f64 {
    let w = (k + 1) as f64 / AP_D as f64;
    range * w * w
}

impl FrameGeometry {
    /// The distance along the ray of aerial-perspective slice `k` in this frame's volume (the last slice is at [`FrameGeometry::ap_range`]).
    pub fn ap_slice_distance(&self, k: usize) -> f64 {
        ap_slice_distance_for(self.ap_range, k)
    }

    /// The (unnormalised) ray through the centre of froxel column `(ix, iy)`: `forward + right a + up b` with `(a, b)` in `[-1, 1]`.
    pub fn froxel_ray(&self, ix: usize, iy: usize) -> [f64; 3] {
        let a = 2.0 * (ix as f64 + 0.5) / AP_W as f64 - 1.0;
        let b = 1.0 - 2.0 * (iy as f64 + 0.5) / AP_H as f64;
        [
            self.fwd[0] + self.right[0] * a + self.upv[0] * b,
            self.fwd[1] + self.right[1] * a + self.upv[1] * b,
            self.fwd[2] + self.right[2] * a + self.upv[2] * b,
        ]
    }

    /// In-scattered radiance (RGB) and the mean transmittance (alpha) from the camera to slice `iz` along froxel `(ix, iy)`'s ray
    /// (cut where the ray leaves the atmosphere or reaches the ground), by `min(2 (iz + 1), AP_MAX_STEPS)` uniform steps.
    pub fn ap_texel(&self, atm: &Atmosphere, luts: &StaticLuts, ix: usize, iy: usize, iz: usize) -> [f64; 4] {
        let dir = normalize3(self.froxel_ray(ix, iy));
        let (mu, cos_vs) = self.view_cosines(dir);
        // The depth of the geometry ends the ray, not the reference sphere: terrain below it (a basin, a camera below the water level)
        // gets its haze too. Sample altitudes below the surface use the surface's density.
        let limit = distance_to_top(self.r * mu, self.c_top);
        let s_end = self.ap_slice_distance(iz).min(limit);
        let o = self.origin(mu);
        let steps = (2 * (iz + 1)).clamp(1, AP_MAX_STEPS);
        let m = atm.march(luts, &o, cos_vs, s_end, StepLayout::Uniform(steps), Scatter::Physical { cos_gamma: cos_vs });
        [m.radiance[0], m.radiance[1], m.radiance[2], (m.transmittance[0] + m.transmittance[1] + m.transmittance[2]) / 3.0]
    }

    /// The aerial perspective at screen position `uv` (`[0, 1]^2`, y down) and distance `t` along the ray from the volume `table`
    /// (`AP_W x AP_H x AP_D` texels, `z` slowest, RGBA): manual trilinear interpolation, the identity `(0, 0, 0, 1)` before slice 0
    /// and the last slice beyond the range.
    pub fn ap_lookup(&self, table: &[[f64; 4]], uv: [f64; 2], t: f64) -> [f64; 4] {
        let px = (uv[0] * AP_W as f64 - 0.5).clamp(0.0, (AP_W - 1) as f64);
        let py = (uv[1] * AP_H as f64 - 0.5).clamp(0.0, (AP_H - 1) as f64);
        let w = libm::sqrt((t / self.ap_range).max(0.0));
        let pz = (w * AP_D as f64 - 1.0).clamp(-1.0, (AP_D - 1) as f64);
        let x0 = (libm::floor(px) as usize).min(AP_W - 2);
        let y0 = (libm::floor(py) as usize).min(AP_H - 2);
        let (fx, fy) = (px - x0 as f64, py - y0 as f64);
        let slice = |z: isize| -> [f64; 4] {
            if z < 0 {
                return [0.0, 0.0, 0.0, 1.0];
            }
            let at = |x: usize, y: usize| table[(z as usize * AP_H + y) * AP_W + x];
            let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
            let mut out = [0.0; 4];
            for k in 0..4 {
                let top = a[k] + (b[k] - a[k]) * fx;
                let bot = c[k] + (d[k] - c[k]) * fx;
                out[k] = top + (bot - top) * fy;
            }
            out
        };
        let z0 = libm::floor(pz) as isize;
        let z1 = (z0 + 1).min(AP_D as isize - 1);
        let fz = pz - z0 as f64;
        let (a, b) = (slice(z0), slice(z1));
        [a[0] + (b[0] - a[0]) * fz, a[1] + (b[1] - a[1]) * fz, a[2] + (b[2] - a[2]) * fz, a[3] + (b[3] - a[3]) * fz]
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The orbit ray march (the camera outside the atmosphere)
// ---------------------------------------------------------------------------------------------------------------------------

/// What the march of one ray from outside the atmosphere returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitRay {
    /// In-scattered radiance along the ray (up to the geometry, the ground or the exit).
    pub radiance: [f64; 3],
    /// Transmittance along the same segment (1 when the ray misses the atmosphere).
    pub transmittance: [f64; 3],
    /// Whether the ray (with no geometry in front) reaches the ground sphere.
    pub reaches_ground: bool,
    /// The sun's cosine to the local up where the ray lands on the ground (meaningful when it reaches it).
    pub ground_mu_sun: f64,
    /// Whether the ray crosses the atmosphere at all.
    pub enters: bool,
}

impl FrameGeometry {
    /// March a ray from the camera outside the atmosphere, in direction `dir` (unit, planet-fixed), ending at geometry at distance
    /// `t_geo` along the ray when given, else at the ground or the exit. The ground's own radiance is **not** added (a sky pixel
    /// adds it with [`Atmosphere::ground_radiance`], a terrain pixel is drawn by the terrain).
    pub fn orbit_ray(&self, atm: &Atmosphere, luts: &StaticLuts, dir: [f64; 3], t_geo: Option<f64>) -> OrbitRay {
        let (mu, cos_vs) = self.view_cosines(dir);
        self.orbit_ray_cosines(atm, luts, mu, cos_vs, t_geo)
    }

    /// [`orbit_ray`](FrameGeometry::orbit_ray) from the ray's cosines to the up and to the sun (the arithmetic the GPU bound measures:
    /// the cosines are dot products the GPU rounds).
    pub fn orbit_ray_cosines(&self, atm: &Atmosphere, luts: &StaticLuts, mu: f64, cos_vs: f64, t_geo: Option<f64>) -> OrbitRay {
        let rmu = self.r * mu;
        let miss = OrbitRay { radiance: [0.0; 3], transmittance: [1.0; 3], reaches_ground: false, ground_mu_sun: 1.0, enters: false };
        if mu >= 0.0 {
            return miss;
        }
        // The discriminants in their factored, limb-stable forms (the GPU's arithmetic: `r mu + q` is exact near the tangent).
        let d_top = (rmu - self.q_top) * (rmu + self.q_top);
        if d_top < 0.0 {
            return miss;
        }
        let sq = libm::sqrt(d_top);
        let t0 = self.c_top / (-rmu + sq);
        let (big_r, top) = (atm.radius_m, atm.top_m());
        // The march is measured from the entry point, whose altitude, c and r mu are exact.
        let o = RayOrigin { h: top, r: big_r + top, c: atm.rho_top * atm.rho_top, rmu: -sq, r_musun: self.r * self.mu_sun + t0 * cos_vs };
        let mut s_end = 2.0 * sq;
        let mut reaches_ground = false;
        let d_ground = (rmu - self.rho) * (rmu + self.rho);
        if d_ground >= 0.0 {
            s_end = self.c / (-rmu + libm::sqrt(d_ground)) - t0;
            reaches_ground = true;
        }
        if let Some(t) = t_geo {
            if t - t0 < s_end {
                s_end = t - t0;
                reaches_ground = false;
            }
        }
        if s_end <= 0.0 {
            return OrbitRay { reaches_ground: false, enters: true, ..miss };
        }
        let m = atm.march(luts, &o, cos_vs, s_end, StepLayout::Uniform(ORBIT_STEPS), Scatter::Physical { cos_gamma: cos_vs });
        let ground_mu_sun = ((o.r_musun + s_end * cos_vs) / big_r).clamp(-1.0, 1.0);
        OrbitRay { radiance: m.radiance, transmittance: m.transmittance, reaches_ground, ground_mu_sun, enters: true }
    }
}

/// What the march of one ray from a camera inside the atmosphere returns (radiance and transmittance up to the geometry or the exit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRay {
    pub radiance: [f64; 3],
    pub transmittance: [f64; 3],
}

impl FrameGeometry {
    /// Whether a pixel with geometry is hazed by [`FrameGeometry::camera_ray_cosines`] and not by the volume (ATMO-021): the camera is
    /// inside the atmosphere from [`CAMERA_MARCH_BLEND_FROM_M`].
    pub fn uses_camera_march(&self) -> bool {
        self.camera_march_weight() > 0.0
    }

    /// The weight of the camera march in the haze of a pixel with geometry: 0 up to [`CAMERA_MARCH_BLEND_FROM_M`], 1 from
    /// [`CAMERA_MARCH_BLEND_TO_M`], the smoothstep of the altitude between. The haze is `(1 - w) volume + w march`.
    pub fn camera_march_weight(&self) -> f64 {
        let t = ((self.h - CAMERA_MARCH_BLEND_FROM_M) / (CAMERA_MARCH_BLEND_TO_M - CAMERA_MARCH_BLEND_FROM_M)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// The steps of [`FrameGeometry::camera_ray_cosines`] in this frame (by the sun's height).
    pub fn camera_march_steps(&self) -> usize {
        if self.mu_sun >= CAMERA_MARCH_TWILIGHT_MU {
            CAMERA_MARCH_STEPS_DAY
        } else {
            CAMERA_MARCH_STEPS_TWILIGHT
        }
    }

    /// The haze of geometry at distance `t_geo` along a ray from the camera inside the atmosphere with the given cosines to the up and to
    /// the sun: [`FrameGeometry::camera_march_steps`] steps from the camera to the geometry, dense towards its end, cut where the ray
    /// leaves the atmosphere.
    pub fn camera_ray_cosines(&self, atm: &Atmosphere, luts: &StaticLuts, mu: f64, cos_vs: f64, t_geo: f64) -> CameraRay {
        let limit = distance_to_top(self.r * mu, self.c_top);
        let s_end = t_geo.min(limit);
        if s_end <= 0.0 {
            return CameraRay { radiance: [0.0; 3], transmittance: [1.0; 3] };
        }
        let o = self.origin(mu);
        let m =
            atm.march(luts, &o, cos_vs, s_end, StepLayout::EndDense(self.camera_march_steps()), Scatter::Physical { cos_gamma: cos_vs });
        CameraRay { radiance: m.radiance, transmittance: m.transmittance }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Sun disc
// ---------------------------------------------------------------------------------------------------------------------------

/// Radiance of the sun's disc in scene units (illuminance 1 over its solid angle), before atmospheric transmittance.
pub fn sun_disc_radiance(sun_angular_radius_rad: f64) -> f64 {
    1.0 / (PI * sun_angular_radius_rad * sun_angular_radius_rad)
}

/// Coverage of a pixel by the sun's disc for a view direction `dir` and the sun direction (unit vectors): 1 inside, 0 outside, a
/// linear ramp of 6 % of the radius across the edge. The angle comes from the chord `|dir - sun|` (`2 asin(chord / 2)`), which is
/// well conditioned for the 0.005 rad that matters; `1 - cos` is not in f32.
pub fn sun_disc_coverage(dir: [f64; 3], sun: [f64; 3], sun_angular_radius_rad: f64) -> f64 {
    let d = [dir[0] - sun[0], dir[1] - sun[1], dir[2] - sun[2]];
    let angle = 2.0 * libm::asin((libm::sqrt(dot3(d, d)) * 0.5).min(1.0));
    ((sun_angular_radius_rad - angle) / (0.06 * sun_angular_radius_rad) + 0.5).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Debug views and the plan's part
// ---------------------------------------------------------------------------------------------------------------------------

/// What the composite pass shows (CON-18): the lit frame, or one of the atmosphere's tables or products as a debug image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AtmosphereView {
    #[default]
    Composite,
    /// The transmittance LUT in discrete bands of optical depth (the static table: identical on every adapter).
    TransmittanceLut,
    /// The multiple-scattering LUT in discrete bands.
    MultiScatterLut,
    /// The sky-view LUT (this frame's).
    SkyViewLut,
    /// The aerial-perspective volume's slice at the camera's depth (the scene's own depth at each pixel).
    AerialPerspective,
    /// The transmittance along each view ray to the geometry or the top of the atmosphere.
    ViewTransmittance,
}

impl AtmosphereView {
    pub const ALL: [AtmosphereView; 6] = [
        AtmosphereView::Composite,
        AtmosphereView::TransmittanceLut,
        AtmosphereView::MultiScatterLut,
        AtmosphereView::SkyViewLut,
        AtmosphereView::AerialPerspective,
        AtmosphereView::ViewTransmittance,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AtmosphereView::Composite => "composite",
            AtmosphereView::TransmittanceLut => "transmittance-lut",
            AtmosphereView::MultiScatterLut => "multiscatter-lut",
            AtmosphereView::SkyViewLut => "sky-view-lut",
            AtmosphereView::AerialPerspective => "aerial-perspective",
            AtmosphereView::ViewTransmittance => "view-transmittance",
        }
    }

    pub fn parse(s: &str) -> Option<AtmosphereView> {
        AtmosphereView::ALL.into_iter().find(|v| v.name() == s)
    }

    pub fn index(self) -> u32 {
        AtmosphereView::ALL.iter().position(|v| *v == self).expect("known view") as u32
    }
}

/// The atmosphere of a lit frame: the body's atmosphere (look parameters and the world's radius) and what to show. A plan without one
/// draws no atmosphere, and its text and hash are what they were before the atmosphere existed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmospherePlan {
    pub atmosphere: Atmosphere,
    pub view: AtmosphereView,
}

impl AtmospherePlan {
    pub fn new(params: AtmosphereParams, radius_m: f64) -> Result<AtmospherePlan, String> {
        Ok(AtmospherePlan { atmosphere: Atmosphere::new(params, radius_m)?, view: AtmosphereView::Composite })
    }

    /// Tokens after `atmosphere` in the plan text: the view, the radius and the 22 parameters, as `f64` bit patterns.
    pub fn tokens(&self) -> Vec<String> {
        let mut out = vec![self.view.name().to_string(), format!("{:016x}", self.atmosphere.radius_m.to_bits())];
        out.extend(self.atmosphere.params.bits().iter().map(|b| format!("{b:016x}")));
        out
    }

    /// Parse the 24 tokens after `atmosphere`.
    pub fn from_tokens(t: &[&str]) -> Result<AtmospherePlan, String> {
        if t.len() != 24 {
            return Err(format!("the atmosphere needs a view, the radius and 22 parameters (24 tokens), got {}", t.len()));
        }
        let view = AtmosphereView::parse(t[0]).ok_or_else(|| format!("unknown atmosphere view '{}'", t[0]))?;
        let bits = |s: &str| -> Result<u64, String> {
            if s.len() != 16 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err(format!("bad f64 bits '{s}': 16 lower-case hex digits expected"));
            }
            u64::from_str_radix(s, 16).map_err(|e| format!("bad f64 bits '{s}': {e}"))
        };
        let radius_m = f64::from_bits(bits(t[1])?);
        let mut b = [0u64; 22];
        for (slot, s) in b.iter_mut().zip(&t[2..]) {
            *slot = bits(s)?;
        }
        let atmosphere = Atmosphere::new(AtmosphereParams::from_bits(&b), radius_m)?;
        Ok(AtmospherePlan { atmosphere, view })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: ATMO-006
    #[test]
    fn the_step_layouts_tile_the_ray() {
        for layout in [StepLayout::Uniform(7), StepLayout::Quadratic(7)] {
            let mut prev = 0.0;
            for i in 0..layout.count() {
                let (a, b) = layout.bounds(1000.0, i);
                assert_eq!(a, prev);
                assert!(b > a);
                prev = b;
            }
            assert_eq!(prev, 1000.0);
        }
    }
}
