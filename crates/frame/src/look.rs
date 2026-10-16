//! `frame::look`: the GPU-free maths of the lit path's presentation data (ADR 0038, ADR 0042 point 7): exposure units, the
//! luminance histogram and its percentile band, the adaptation step, the ACES filmic fit and the sRGB encode, the pre-exposure
//! factor, and the lit terrain's base radiance. Everything the shaders compute with these formulas has its f64 reference here
//! (CON-02, CON-03): the renderer executes, the tests compare. Class: presentation data (ADR 0035); the bounds are stated per
//! function and in `openspec/specs/renderer` (REND-019 to REND-022).
//!
//! Units. Scene radiance is in units of the top-of-atmosphere solar illuminance (= 1, ADR 0038 point 1): a white Lambertian
//! surface facing the sun has `1 / pi`. One unit is [`SOLAR_ILLUMINANCE_LUX`] lux, so `luminance_cd = unit * LUMINANCE_PER_UNIT`
//! for a diffuse surface when the unit radiance is read as luminance per steradian. Exposure follows the saturation-based
//! convention: `exposure = 1 / (1.2 * 2^EV100)` applied to luminance in cd/m2, and the EV100 that suits a luminance `L` is
//! `log2(L * 100 / 12.5) = log2(8 L)`, the same function the histogram bins by.

use planet_core::hash::hash_u64s;

/// Illuminance of the sun above the atmosphere in lux (1361 W/m2 at 93 lm/W): the value of "1" in scene units.
pub const SOLAR_ILLUMINANCE_LUX: f64 = 126_600.0;
/// Luminance in cd/m2 of one scene unit of (diffuse) radiance.
pub const LUMINANCE_PER_UNIT: f64 = SOLAR_ILLUMINANCE_LUX;
/// Every write into the scene target is clamped to this, so the sun disk cannot make `inf` or `NaN` in f16 (ADR 0038 point 1).
pub const HDR_CLAMP: f32 = 60_000.0;
/// Bins of the luminance histogram.
pub const HIST_BINS: usize = 64;
/// The histogram covers EV -10 to +20 of pixel luminance (`log2(8 L)`), 30 stops in 64 bins.
pub const HIST_EV_MIN: f64 = -10.0;
pub const HIST_EV_MAX: f64 = 20.0;
/// The mean is taken over the pixels between these fractions of the counted pixels (ADR 0038 point 2).
pub const BAND_LOW: f64 = 0.4;
pub const BAND_HIGH: f64 = 0.9;
/// Luminance weights of linear Rec. 709 / sRGB primaries.
pub const LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];

/// Exposure range and adaptation speeds (look data, ADR 0042 point 6; the defaults of ADR 0038 point 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureParams {
    pub min_ev100: f32,
    pub max_ev100: f32,
    /// Stops per second when the scene gets brighter (EV100 rises; light adaptation is fast).
    pub speed_up: f32,
    /// Stops per second when the scene gets darker (EV100 falls; dark adaptation is slow).
    pub speed_down: f32,
}

impl Default for ExposureParams {
    fn default() -> Self {
        Self { min_ev100: -2.0, max_ev100: 16.0, speed_up: 3.0, speed_down: 1.0 }
    }
}

/// How the exposure is set for a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExposureMode {
    /// A fixed EV100: goldens, perf scenes and repro bundles (CON-19). No histogram, no history.
    Fixed { ev100: f32 },
    /// Automatic exposure from the luminance histogram. `pre_ev100` is the CPU-decided pre-exposure: the EV100 the scene
    /// target is written at (so f16 holds the range), chosen by the view, not read back from the GPU.
    Adaptive { params: ExposureParams, pre_ev100: f32 },
}

impl ExposureMode {
    /// The EV100 the scene target is written at.
    pub fn pre_ev100(&self) -> f32 {
        match *self {
            ExposureMode::Fixed { ev100 } => ev100,
            ExposureMode::Adaptive { pre_ev100, .. } => pre_ev100,
        }
    }

    pub fn is_adaptive(&self) -> bool {
        matches!(self, ExposureMode::Adaptive { .. })
    }
}

/// What the plan says about the lit path of one frame (ADR 0042 point 3). The renderer decides nothing: the sun, the exposure
/// mode, the time step and the reset all arrive here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LitPlan {
    pub exposure: ExposureMode,
    /// Seconds since the previous frame, from the injected clock (CON-04): the only time the exposure adaptation sees.
    pub dt_s: f32,
    /// Unit vector towards the sun in the planet-fixed axes the vertex normals use. Until the sun is a field of the plan
    /// (ADR 0039) the views set it.
    pub sun_dir: [f32; 3],
    /// Snap the adapted exposure to its target now (a cut, a first frame).
    pub reset_exposure: bool,
    /// Tone-map output: normal, or the false-colour luminance debug view (CON-18).
    pub tonemap: ToneMapMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToneMapMode {
    /// ACES fit and sRGB encode.
    Aces,
    /// False-colour EV bands of the exposed luminance (debug view, ADR 0038 point 5).
    EvBands,
}

impl ToneMapMode {
    pub fn name(self) -> &'static str {
        match self {
            ToneMapMode::Aces => "aces",
            ToneMapMode::EvBands => "ev-bands",
        }
    }
}

/// The fixed sun of the first lit views (the relief view's sun): towards +x, +y and +z, normalised.
pub const DEFAULT_SUN: [f32; 3] = [0.501_405_9, 0.601_687_1, 0.621_743_3];

impl LitPlan {
    /// A fixed-exposure lit plan, the golden and perf form (CON-19).
    pub fn fixed(ev100: f32) -> Self {
        Self { exposure: ExposureMode::Fixed { ev100 }, dt_s: 0.0, sun_dir: DEFAULT_SUN, reset_exposure: false, tonemap: ToneMapMode::Aces }
    }

    /// An adaptive-exposure lit plan with the default range and speeds.
    pub fn adaptive(pre_ev100: f32, dt_s: f32) -> Self {
        Self {
            exposure: ExposureMode::Adaptive { params: ExposureParams::default(), pre_ev100 },
            dt_s,
            sun_dir: DEFAULT_SUN,
            reset_exposure: false,
            tonemap: ToneMapMode::Aces,
        }
    }

    /// Stable text for plan snapshots and repro bundles (floats as bit patterns).
    pub fn to_text(&self) -> String {
        let h = |v: f32| format!("{:08x}", v.to_bits());
        let mode = match self.exposure {
            ExposureMode::Fixed { ev100 } => format!("fixed {}", h(ev100)),
            ExposureMode::Adaptive { params: p, pre_ev100 } => {
                format!("adaptive {} {} {} {} {}", h(pre_ev100), h(p.min_ev100), h(p.max_ev100), h(p.speed_up), h(p.speed_down))
            }
        };
        format!(
            "lit {mode} dt {} sun {} {} {} reset {} tonemap {}",
            h(self.dt_s),
            h(self.sun_dir[0]),
            h(self.sun_dir[1]),
            h(self.sun_dir[2]),
            u8::from(self.reset_exposure),
            self.tonemap.name()
        )
    }

    /// Parse the tokens after the leading `lit` of [`to_text`](LitPlan::to_text).
    pub fn from_tokens(t: &[&str]) -> Result<LitPlan, String> {
        let f = |s: &str| u32::from_str_radix(s, 16).map(f32::from_bits).map_err(|e| format!("bad f32 bits '{s}': {e}"));
        let (exposure, rest) = match t {
            ["fixed", ev, rest @ ..] => (ExposureMode::Fixed { ev100: f(ev)? }, rest),
            ["adaptive", pre, lo, hi, up, down, rest @ ..] => (
                ExposureMode::Adaptive {
                    params: ExposureParams { min_ev100: f(lo)?, max_ev100: f(hi)?, speed_up: f(up)?, speed_down: f(down)? },
                    pre_ev100: f(pre)?,
                },
                rest,
            ),
            _ => return Err("lit line needs 'fixed' or 'adaptive'".into()),
        };
        match rest {
            ["dt", dt, "sun", sx, sy, sz, "reset", reset, "tonemap", tm] => Ok(LitPlan {
                exposure,
                dt_s: f(dt)?,
                sun_dir: [f(sx)?, f(sy)?, f(sz)?],
                reset_exposure: *reset == "1",
                tonemap: match *tm {
                    "aces" => ToneMapMode::Aces,
                    "ev-bands" => ToneMapMode::EvBands,
                    other => return Err(format!("unknown tone map '{other}'")),
                },
            }),
            _ => Err("lit line is not 'dt .. sun .. reset .. tonemap ..'".into()),
        }
    }

    /// Refuse values a renderer cannot use: non-finite or negative time, non-finite EVs, a reversed or non-finite range, negative
    /// speeds, a sun that is not a unit vector. The message names the value (repro bundles can carry any text).
    pub fn validate(&self) -> Result<(), String> {
        let finite = |name: &str, v: f32| if v.is_finite() { Ok(()) } else { Err(format!("lit plan: {name} is {v}, not finite")) };
        finite("dt", self.dt_s)?;
        if self.dt_s < 0.0 {
            return Err(format!("lit plan: dt is {} s, negative", self.dt_s));
        }
        finite("pre-exposure EV100", self.exposure.pre_ev100())?;
        if let ExposureMode::Adaptive { params: p, .. } = self.exposure {
            for (n, v) in [("min EV100", p.min_ev100), ("max EV100", p.max_ev100), ("speed up", p.speed_up), ("speed down", p.speed_down)] {
                finite(n, v)?;
            }
            if p.min_ev100 > p.max_ev100 {
                return Err(format!("lit plan: EV100 range [{}, {}] is reversed", p.min_ev100, p.max_ev100));
            }
            if p.speed_up < 0.0 || p.speed_down < 0.0 {
                return Err(format!("lit plan: adaptation speeds {} and {} must not be negative", p.speed_up, p.speed_down));
            }
        }
        let n = self.sun_dir.map(f64::from);
        let len = libm::sqrt(n[0] * n[0] + n[1] * n[1] + n[2] * n[2]);
        if !len.is_finite() || (len - 1.0).abs() > 1.0e-3 {
            return Err(format!("lit plan: sun direction {:?} has length {len}, not 1", self.sun_dir));
        }
        Ok(())
    }

    /// Hash of the text form (for the plan hash).
    pub fn hash(&self) -> u64 {
        let bytes: Vec<u64> = self.to_text().bytes().map(u64::from).collect();
        hash_u64s(0x006c_6974, &bytes)
    }
}

/// `1 / (1.2 * 2^ev100)`: the factor from luminance in cd/m2 to display-referred linear (1.0 = the saturation luminance).
pub fn exposure_scale(ev100: f64) -> f64 {
    1.0 / (1.2 * libm::exp2(ev100))
}

/// The factor the scene target is written with: scene units to the pre-exposed value, `LUMINANCE_PER_UNIT * exposure_scale`.
/// Computed in f64 here and rounded once to f32 for the shader (CON-03: the CPU decides it).
pub fn pre_exposure_factor(pre_ev100: f64) -> f64 {
    LUMINANCE_PER_UNIT * exposure_scale(pre_ev100)
}

/// The EV100 that suits a luminance in cd/m2 (and the axis of the histogram): `log2(8 L)`.
pub fn luminance_ev(luminance_cd: f64) -> f64 {
    libm::log2(8.0 * luminance_cd)
}

/// Luminance of a linear RGB triple.
pub fn luminance(rgb: [f64; 3]) -> f64 {
    LUMA[0] * rgb[0] + LUMA[1] * rgb[1] + LUMA[2] * rgb[2]
}

/// Width of one histogram bin in stops.
pub fn bin_width() -> f64 {
    (HIST_EV_MAX - HIST_EV_MIN) / HIST_BINS as f64
}

/// Centre of bin `i` in EV.
pub fn bin_center_ev(i: usize) -> f64 {
    HIST_EV_MIN + (i as f64 + 0.5) * bin_width()
}

/// The bin of a pixel EV, or `None` for a pixel that is not counted (no light: NaN or minus infinity; plus infinity is the last bin). EVs outside the range land in the
/// first or last bin.
pub fn bin_of_ev(ev: f64) -> Option<usize> {
    if ev.is_nan() || ev == f64::NEG_INFINITY {
        return None;
    }
    let b = libm::floor((ev - HIST_EV_MIN) / bin_width());
    Some(b.clamp(0.0, (HIST_BINS - 1) as f64) as usize)
}

/// The histogram of a scene target image, the CPU reference of the GPU pass. `pixels` are the values as written (pre-exposed,
/// linear RGB per pixel); `pre_ev100` undoes the pre-exposure so the bins are in scene EV. A pixel with no luminance is not
/// counted.
pub fn histogram_of(pixels: &[[f64; 3]], pre_ev100: f64) -> [u32; HIST_BINS] {
    let mut h = [0u32; HIST_BINS];
    for p in pixels {
        let y = luminance(*p);
        if y > 0.0 {
            // L_cd = y / exposure_scale(pre); EV = log2(8 L_cd).
            if let Some(b) = bin_of_ev(luminance_ev(y / exposure_scale(pre_ev100))) {
                h[b] += 1;
            }
        }
    }
    h
}

/// The mean EV of the pixels between the [`BAND_LOW`] and [`BAND_HIGH`] fractions of the counted ones (bins weighted by how much
/// of their count lies in the band, bin centres as values); `None` when nothing was counted.
pub fn band_mean_ev(hist: &[u32; HIST_BINS]) -> Option<f64> {
    let total: u64 = hist.iter().map(|&c| u64::from(c)).sum();
    if total == 0 {
        return None;
    }
    let (lo, hi) = (BAND_LOW * total as f64, BAND_HIGH * total as f64);
    let (mut below, mut weight, mut sum) = (0.0f64, 0.0f64, 0.0f64);
    for (i, &c) in hist.iter().enumerate() {
        let (a, b) = (below, below + f64::from(c));
        let overlap = (b.min(hi) - a.max(lo)).max(0.0);
        weight += overlap;
        sum += overlap * bin_center_ev(i);
        below = b;
    }
    (weight > 0.0).then(|| sum / weight)
}

/// The EV100 the exposure moves towards: the band mean clamped to the range, or `current` when nothing was counted.
pub fn target_ev(hist: &[u32; HIST_BINS], params: &ExposureParams, current: f64) -> f64 {
    match band_mean_ev(hist) {
        Some(m) => m.clamp(f64::from(params.min_ev100), f64::from(params.max_ev100)),
        None => current,
    }
}

/// One adaptation step in stops: `current` moves towards `target` by at most `speed_up * dt` up or `speed_down * dt` down;
/// with `reset` it snaps. Linear in EV, so a step does not overshoot and the result depends on `dt` only through the cap.
pub fn adapt_ev(current: f64, target: f64, dt_s: f64, params: &ExposureParams, reset: bool) -> f64 {
    if reset {
        return target;
    }
    let up = f64::from(params.speed_up) * dt_s.max(0.0);
    let down = f64::from(params.speed_down) * dt_s.max(0.0);
    current + (target - current).clamp(-down, up)
}

/// The ACES filmic fit (Narkowicz 2015) on exposed linear `x`, clamped to [0, 1].
pub fn aces(x: f64) -> f64 {
    let x = x.max(0.0);
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// The sRGB transfer function (linear to encoded), both in [0, 1].
pub fn srgb_encode(l: f64) -> f64 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        12.92 * l
    } else {
        1.055 * libm::pow(l, 1.0 / 2.4) - 0.055
    }
}

/// The tone-mapped 8-bit colour of a scene-target value: `written` is the pre-exposed linear RGB, `pre_ev100` the EV100 it was
/// written at and `ev100` the exposure now; the exposed value is `written * 2^(pre - ev)`. The CPU reference of the tone-map pass
/// (bound: 1 / 255 per channel, REND-020).
pub fn tonemap_pixel(written: [f64; 3], pre_ev100: f64, ev100: f64) -> [u8; 3] {
    let k = libm::exp2(pre_ev100 - ev100);
    written.map(|v| (srgb_encode(aces(v * k)) * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8)
}

/// Stops per band of the false-colour view and the number of bands.
pub const EV_BAND_STOPS: f64 = 1.0;
pub const EV_BAND_COUNT: usize = 12;

/// Display colours (8-bit, written as they are) of the false-colour luminance view: band 0 (clipped or within one stop of
/// saturation) in red, then warm to cool towards the dark bands; the shader's table is generated from this one.
pub const EV_BAND_COLORS: [[u8; 3]; EV_BAND_COUNT] = [
    [255, 32, 32],
    [255, 128, 32],
    [255, 224, 32],
    [160, 224, 32],
    [32, 192, 64],
    [32, 192, 160],
    [32, 160, 224],
    [32, 96, 224],
    [64, 64, 192],
    [96, 32, 160],
    [64, 16, 96],
    [24, 8, 40],
];

/// The EV offset of the histogram shader: pixel EV = `log2(8 y) + offset` for a value `y` written at `pre_ev100`
/// (`log2(8 y / exposure_scale(pre))`).
pub fn histogram_ev_offset(pre_ev100: f64) -> f64 {
    libm::log2(1.2) + pre_ev100
}

/// The band of the false-colour luminance view: whole stops of the exposed luminance below the saturation value 1.0, band 0 being
/// `[0.5, 1]` and clipped values (above 1) in band 0 as well; `None` for black.
pub fn ev_band(exposed_luminance: f64) -> Option<usize> {
    if exposed_luminance <= 0.0 || !exposed_luminance.is_finite() {
        return None;
    }
    let stops_below = -libm::log2(exposed_luminance) / EV_BAND_STOPS;
    Some((libm::floor(stops_below).max(0.0) as usize).min(EV_BAND_COUNT - 1))
}

/// Base radiance of the lit terrain before the pre-exposure: Lambert of the vertex normal under the sun with a height tint as
/// albedo (a placeholder until the albedo comes from the climate field, ADR 0039), in scene units. `height` and `range` are metres.
/// The tint is the relief view's.
pub fn lit_terrain_radiance(normal: [f64; 3], height: f64, range: [f64; 2], sun: [f64; 3]) -> [f64; 3] {
    let n = normalize(normal);
    let t = ((height - range[0]) / (range[1] - range[0]).max(1e-6)).clamp(0.0, 1.0);
    let albedo = tint(t).map(|c| c * ALBEDO_SCALE);
    let ndl = (n[0] * sun[0] + n[1] * sun[1] + n[2] * sun[2]).max(0.0);
    albedo.map(|a| a * ndl / core::f64::consts::PI)
}

/// Scale from the relief view's display tint to a linear albedo.
pub const ALBEDO_SCALE: f64 = 0.4;

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    v.map(|c| c / l)
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The height tint of the relief view (water blue, green, brown, white by normalised height).
pub fn tint(t: f64) -> [f64; 3] {
    let low = mix3([0.18, 0.36, 0.56], [0.36, 0.56, 0.26], smoothstep(0.0, 0.35, t));
    let high = mix3([0.66, 0.56, 0.38], [0.95, 0.95, 0.95], smoothstep(0.7, 1.0, t));
    mix3(low, high, smoothstep(0.35, 0.7, t))
}

/// What the scene target holds for a radiance: pre-exposed and clamped to [`HDR_CLAMP`] (the CPU reference of every lit write).
pub fn written_value(radiance: [f64; 3], pre_ev100: f64) -> [f64; 3] {
    let f = pre_exposure_factor(pre_ev100);
    radiance.map(|v| (v * f).clamp(0.0, f64::from(HDR_CLAMP)))
}
