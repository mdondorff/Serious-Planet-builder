//! The water surface (ADR 0041 points 4 to 6, ADR 0051, ADR 0053): Gerstner waves with phases reduced on the CPU, the shoreline from
//! the vertical depth `W - h_morph`, and the surface's radiance (absorption, Fresnel, sun glint, emission). The f64 reference of every
//! function the water shader evaluates lives here (CON-02, CON-03): the renderer executes, the tests compare.
//!
//! Class (ADR 0035): **presentation data**. Wave displacement, shading and foam are never stored, keyed or read back; the field W and
//! the liquid table they read are world data (item `ocean-bake-and-water-level`). Declared bounds (REND-030, REND-031):
//! * wave displacement and slope: the GPU's agree with this module's evaluation of the same f32-rounded inputs and CPU-reduced phases
//!   within `1 mm + 1e-3 x (sum of the amplitudes)` per component (WGSL guarantees `sin` and `cos` only to about 5e-4 absolute);
//! * the surface's radiance: within `max(1 % |v|, 1e-4)` per channel before the f16 store, on the same inputs;
//! * temporal stability (CON-11 v1.4): the phases the shader adds are small (`|k . local| <= WAVE_ARGUMENT_BOUND` rad wherever a
//!   wave has weight) and the planet-sized part of every phase is reduced in f64, so a camera move of 1 mm changes no value by more
//!   than the functions' own slope times 1 mm.
//!
//! **The wave set.** Every liquid has up to [`WAVES`] plane waves in the planet-fixed 3D frame, `k_i = 2 pi / lambda_i x D_i`, with the
//! directions `D_i` the vertices of a regular tetrahedron (so that at every place at least two waves lie well along the surface).
//! The phase of a wave at the planet-fixed point `P` is `k_i . P + phi_i - omega_i t`; the CPU reduces `(k_i . O + phi_i - omega_i t)
//! mod 2 pi` per tile (O the tile origin, in f64) and the shader adds `k_i . local`, `local` the tile-local position of the point,
//! so the argument of every sine is small and two tiles agree on the phase of a shared point (no projection onto a per-tile tangent
//! plane, which would open a seam at every tile border). A wave's weight is the product of three continuous factors: its tangent
//! share (a wave that points up out of the surface has none), its resolution (a wave the vertex grid cannot hold, fewer than four vertices
//! per wavelength at the vertex's own spacing, fades out) and the shore (the amplitude falls to zero with the depth, so the surface never dips below
//! the terrain where the water is thin).

use super::shading::smoothstep;

/// Waves per liquid.
pub const WAVES: usize = 4;
/// Liquids a frame may draw (kinds of the water tiles of one plan).
pub const MAX_LIQUIDS: usize = 4;
/// Bytes of the water globals uniform: two `vec4` of look values, one `vec4<u32>` and [`MAX_LIQUIDS`] liquids of seven `vec4`.
pub const GLOBALS_BYTES: u64 = 3 * 16 + MAX_LIQUIDS as u64 * LIQUID_VEC4S as u64 * 16;
/// `vec4` words of one liquid: colour, absorption, emission and the four waves (`xyz` the wave vector, `w` the amplitude).
pub const LIQUID_VEC4S: usize = 7;
/// Bytes of the per-node record: the level, the liquid and two reserved words, then the four reduced phases.
pub const NODE_RECORD_BYTES: u64 = 32;

/// Wavelength of wave `i` as a fraction of the liquid's dominant wavelength.
pub const WAVE_LENGTH_RATIOS: [f64; WAVES] = [1.0, 0.62, 0.38, 0.23];
/// Amplitude of wave `i` as a fraction of the liquid's wave amplitude.
pub const WAVE_AMPLITUDE_RATIOS: [f64; WAVES] = [1.0, 0.55, 0.30, 0.15];
/// Phase offsets of the waves (rad): fixed, so that the waves do not crest together at the origin.
pub const WAVE_PHASE_OFFSETS: [f64; WAVES] = [0.0, 1.3, 2.9, 4.4];
/// The wave directions in the planet-fixed frame: the vertices of a regular tetrahedron (unit vectors).
pub const WAVE_DIRECTIONS: [[f64; 3]; WAVES] = [
    [0.577_350_269_189_625_8, 0.577_350_269_189_625_8, 0.577_350_269_189_625_8],
    [0.577_350_269_189_625_8, -0.577_350_269_189_625_8, -0.577_350_269_189_625_8],
    [-0.577_350_269_189_625_8, 0.577_350_269_189_625_8, -0.577_350_269_189_625_8],
    [-0.577_350_269_189_625_8, -0.577_350_269_189_625_8, 0.577_350_269_189_625_8],
];
/// The tangent share of a wave (the length of its direction's part along the surface) below which it has no weight, and above which
/// it has full weight (a smoothstep between).
pub const TANGENT_SHARE: (f64, f64) = (0.25, 0.6);
/// Pixels per wavelength over which a wave fades out of the shading (ADR 0056): none below the first, full from the second. The
/// footprint is the pixel's size on the surface; the vertex stage has no footprint and applies no fade.
pub const PIXEL_FADE: (f64, f64) = (2.0, 4.0);
/// The largest `|k . local|` (rad) any wave with weight may have in a shader (about 1e3 would cost the argument its f32 digits);
/// tested on real plans.
pub const WAVE_ARGUMENT_BOUND: f64 = 1.0e3;
const TWO_PI: f64 = core::f64::consts::TAU;
const INV_PI: f64 = core::f64::consts::FRAC_1_PI;

/// The presentation parameters of the water surface (look data, schema version 4, `[body.<n>.water]`): how a liquid is shown, not
/// what it is (the liquid table of the world definition holds colour, emission, viscosity and the wave height and length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterLook {
    /// Scales the optical depth: the extinction per metre of the colour channel `c` is `absorption_per_m x -ln(max(colour_c, 1e-3)) / -ln(1e-3)`.
    pub absorption_per_m: f64,
    /// Reflectance at normal incidence (water 0.02).
    pub fresnel_f0: f64,
    /// Roughness of the sun glint's distribution (GGX `alpha = roughness^2`), beyond the waves' own slope.
    pub glint_roughness: f64,
    /// Depth over which the surface layer (reflection, specular) fades in from the shore.
    pub shore_fade_m: f64,
    /// Depth of the foam band at the shore, and the share of the band that is foam at its densest.
    pub foam_width_m: f64,
    pub foam_strength: f64,
    /// Horizontal displacement of a crest as a multiple of its height (0 sinusoidal). A liquid of short, high waves folds the surface
    /// over itself when `choppy x |k| x amplitude` of its waves sums above 1: keep it small there.
    pub choppy: f64,
    /// Gravity of the body in m/s2: the dispersion `omega = sqrt(g k)` of deep-water waves.
    pub gravity_m_s2: f64,
    /// Multiplies the wave height of every liquid (a look override of the table's amplitude; 0 switches the waves off).
    pub wave_scale: f64,
    /// Strength of the `fwidth` filter of the shore bands (ADR 0041 point 6): the cover, the foam and their edges are widened by this
    /// multiple of the screen-space change of the depth per pixel, so a band narrower than a pixel fades instead of coming and going
    /// with sub-pixel motion. 0 switches the filter off (the aliased bands; the default since the owner's M5 gate decision of 2026-10-18,
    /// the cheaper shore).
    pub shore_filter: f64,
}

impl WaterLook {
    pub const EARTH: WaterLook = WaterLook {
        absorption_per_m: 0.5,
        fresnel_f0: 0.02,
        glint_roughness: 0.08,
        shore_fade_m: 0.05,
        foam_width_m: 0.4,
        foam_strength: 0.8,
        choppy: 0.8,
        gravity_m_s2: 9.81,
        wave_scale: 1.0,
        shore_filter: 0.0,
    };

    /// The keys of the table in the order of [`WaterLook::values`].
    pub const KEYS: [&'static str; 10] = [
        "absorption_per_m",
        "fresnel_f0",
        "glint_roughness",
        "shore_fade_m",
        "foam_width_m",
        "foam_strength",
        "choppy",
        "gravity_m_s2",
        "wave_scale",
        "shore_filter",
    ];

    /// The valid range of each key (the shader divides by the fade and the width, and the colours are linear).
    pub const RANGES: [(f64, f64); 10] = [
        (1.0e-3, 100.0),
        (0.0, 0.2),
        (0.01, 1.0),
        (1.0e-3, 10.0),
        (1.0e-2, 100.0),
        (0.0, 1.0),
        (0.0, 2.0),
        (0.1, 100.0),
        (0.0, 10.0),
        (0.0, 4.0),
    ];

    pub fn values(&self) -> [f64; 10] {
        [
            self.absorption_per_m,
            self.fresnel_f0,
            self.glint_roughness,
            self.shore_fade_m,
            self.foam_width_m,
            self.foam_strength,
            self.choppy,
            self.gravity_m_s2,
            self.wave_scale,
            self.shore_filter,
        ]
    }

    pub fn values_mut(&mut self) -> [&mut f64; 10] {
        [
            &mut self.absorption_per_m,
            &mut self.fresnel_f0,
            &mut self.glint_roughness,
            &mut self.shore_fade_m,
            &mut self.foam_width_m,
            &mut self.foam_strength,
            &mut self.choppy,
            &mut self.gravity_m_s2,
            &mut self.wave_scale,
            &mut self.shore_filter,
        ]
    }

    /// The first value outside its range: the key (relative to the table), the value and why.
    pub fn validate(&self) -> Result<(), (String, String, String)> {
        for ((name, v), (lo, hi)) in Self::KEYS.iter().zip(self.values()).zip(Self::RANGES) {
            if !v.is_finite() || v < lo || v > hi {
                return Err((format!("water.{name}"), format!("{v:?}"), format!("must be a finite number between {lo:?} and {hi:?}")));
            }
        }
        Ok(())
    }

    /// Every value rounded once to `f32` and read back: what the shader sees.
    pub fn as_f32(&self) -> WaterLook {
        let mut out = *self;
        for v in out.values_mut() {
            *v = f64::from(*v as f32);
        }
        out
    }
}

impl Default for WaterLook {
    fn default() -> Self {
        Self::EARTH
    }
}

/// One plane wave of a liquid in the planet-fixed frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    /// Wave vector (rad/m), `2 pi / lambda` along the wave's direction.
    pub k: [f64; 3],
    /// Height (m, half the crest-to-trough distance).
    pub amplitude: f64,
    /// Phase offset (rad).
    pub phi: f64,
    /// Angular frequency (rad/s).
    pub omega: f64,
}

/// How a liquid is drawn: its colour, extinction, emission and waves, decided in f64 from the liquid table of the world definition and
/// the look. Rounded once to f32 where the shader takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiquidAppearance {
    /// The kind id of the world definition's liquid table (the water tiles name it).
    pub kind: u32,
    /// Colour in deep water, linear RGB in [0, 1]; the albedo of the liquid body.
    pub colour: [f64; 3],
    /// Extinction per metre per channel.
    pub sigma: [f64; 3],
    /// Emitted radiance in scene units (the sun's illuminance is 1), per channel.
    pub emission: [f64; 3],
    /// The waves; amplitude 0 means none (a lava lake).
    pub waves: [Wave; WAVES],
}

/// The extinction of a liquid of the given colour: darker channels are absorbed faster.
pub fn extinction(colour: [f64; 3], absorption_per_m: f64) -> [f64; 3] {
    let norm = -libm::log(1.0e-3);
    colour.map(|c| absorption_per_m * (-libm::log(c.max(1.0e-3))) / norm)
}

impl LiquidAppearance {
    /// From a row of the liquid table (`planet_world_def::LiquidKind`) and the look. `viscosity` slows the waves: `omega` is the
    /// deep-water dispersion divided by `1 + viscosity` (water at 1 mPa s is not slowed visibly, lava barely moves).
    pub fn new(kind: u32, row: &planet_world_def::LiquidKind, look: &WaterLook) -> LiquidAppearance {
        let amplitude = row.wave_amplitude_m * look.wave_scale;
        let slow = 1.0 / (1.0 + row.viscosity_pa_s);
        let mut waves = [Wave { k: [0.0; 3], amplitude: 0.0, phi: 0.0, omega: 0.0 }; WAVES];
        if amplitude > 0.0 && row.wave_length_m > 0.0 {
            for (i, w) in waves.iter_mut().enumerate() {
                let klen = TWO_PI / (row.wave_length_m * WAVE_LENGTH_RATIOS[i]);
                w.k = WAVE_DIRECTIONS[i].map(|c| c * klen);
                w.amplitude = amplitude * WAVE_AMPLITUDE_RATIOS[i];
                w.phi = WAVE_PHASE_OFFSETS[i];
                w.omega = libm::sqrt(look.gravity_m_s2 * klen) * slow;
            }
        }
        LiquidAppearance { kind, colour: row.colour, sigma: extinction(row.colour, look.absorption_per_m), emission: row.emission, waves }
    }

    pub fn has_waves(&self) -> bool {
        self.waves[0].amplitude > 0.0
    }

    /// The sum of the wave amplitudes (the largest the surface moves).
    pub fn amplitude_sum(&self) -> f64 {
        self.waves.iter().map(|w| w.amplitude).sum()
    }

    /// The liquid as the uniform block holds it ([`LIQUID_VEC4S`] `vec4<f32>`): colour, extinction, emission, then the four waves'
    /// `(k, amplitude)`. Each value rounded once to f32.
    pub fn to_words(&self) -> [f32; LIQUID_VEC4S * 4] {
        let mut w = [0f32; LIQUID_VEC4S * 4];
        for (slot, v) in w[0..3].iter_mut().zip(self.colour) {
            *slot = v as f32;
        }
        for (slot, v) in w[4..7].iter_mut().zip(self.sigma) {
            *slot = v as f32;
        }
        for (slot, v) in w[8..11].iter_mut().zip(self.emission) {
            *slot = v as f32;
        }
        for (i, wave) in self.waves.iter().enumerate() {
            let o = 12 + 4 * i;
            for c in 0..3 {
                w[o + c] = wave.k[c] as f32;
            }
            w[o + 3] = wave.amplitude as f32;
        }
        w
    }

    /// The same values rounded once to `f32` and read back as `f64`: what the shader sees.
    pub fn as_f32(&self) -> LiquidAppearance {
        let r = |v: f64| f64::from(v as f32);
        let mut out = *self;
        out.colour = self.colour.map(r);
        out.sigma = self.sigma.map(r);
        out.emission = self.emission.map(r);
        for (o, w) in out.waves.iter_mut().zip(&self.waves) {
            o.k = w.k.map(r);
            o.amplitude = r(w.amplitude);
        }
        out
    }
}

/// The reduced phase of wave `w` at the tile origin `origin` (planet-fixed metres) at time `t_s`: `(k . O + phi - omega t) mod 2 pi`
/// in f64, in `[0, 2 pi)`. The only place a planet-sized number meets a wave vector; the shader receives the result as an f32.
pub fn tile_phase(w: &Wave, origin: [f64; 3], t_s: f64) -> f64 {
    let a = w.k[0] * origin[0] + w.k[1] * origin[1] + w.k[2] * origin[2] + w.phi - w.omega * t_s;
    let r = a.rem_euclid(TWO_PI);
    // rem_euclid of a tiny negative number can round up to 2 pi itself.
    if r >= TWO_PI {
        0.0
    } else {
        r
    }
}

/// The reduced phases of all waves of a liquid at a tile origin, rounded once to f32 (what the node record holds).
pub fn tile_phases(liquid: &LiquidAppearance, origin: [f64; 3], t_s: f64) -> [f32; WAVES] {
    let mut out = [0f32; WAVES];
    for (o, w) in out.iter_mut().zip(&liquid.waves) {
        *o = tile_phase(w, origin, t_s) as f32;
    }
    out
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len(a: [f64; 3]) -> f64 {
    libm::sqrt(dot(a, a))
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    a.map(|c| c * s)
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(a: [f64; 3]) -> [f64; 3] {
    scale(a, 1.0 / len(a))
}

/// The surface's displacement (planet-fixed axes, metres) and slope (the surface gradient along the tangent plane, dimensionless).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveSample {
    pub disp: [f64; 3],
    pub slope: [f64; 3],
}

/// What a wave evaluation is asked at a point of the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WavePoint {
    /// Tile-local position of the undisplaced surface point (m), planet-fixed axes.
    pub local: [f64; 3],
    /// Unit up at the point.
    pub up: [f64; 3],
    /// The tile's reduced phases ([`tile_phases`], f32 values).
    pub phases: [f64; WAVES],
    /// The effective vertex spacing of the tile at the point (m): the tile's sample spacing times `1 + m`, `m` the vertex's morph
    /// factor (a vertex morphed onto the parent grid sits on a grid twice as coarse). A wave needs four vertices per wavelength.
    pub spacing: f64,
    /// Vertical depth `W - h_morph` (m); the waves fall to zero as it does.
    pub depth: f64,
    /// The pixel's size on the surface (m), the larger of its two screen-axis derivatives of the position; 0 where there is no pixel
    /// (the vertex stage): no pixel fade.
    pub footprint: f64,
}

/// The weights of the waves at a point: tangent share, resolution and shore (each continuous in the point).
pub fn wave_weights(liquid: &LiquidAppearance, p: &WavePoint) -> [f64; WAVES] {
    let total = liquid.amplitude_sum();
    let mut out = [0.0; WAVES];
    if total <= 0.0 {
        return out;
    }
    let shore = smoothstep(0.0, 4.0 * total, p.depth);
    for (o, w) in out.iter_mut().zip(&liquid.waves) {
        if w.amplitude <= 0.0 {
            continue;
        }
        let klen = len(w.k);
        let khat = scale(w.k, 1.0 / klen);
        let kn = dot(khat, p.up);
        let s = libm::sqrt((1.0 - kn * kn).max(0.0));
        let w_tan = smoothstep(TANGENT_SHARE.0, TANGENT_SHARE.1, s);
        let lambda = TWO_PI / klen;
        // Resolution: full weight from eight vertices per wavelength, none below four (continuous in the spacing, which is continuous
        // in the morph factor, so no LOD border shows).
        let w_res = smoothstep(4.0, 8.0, lambda / p.spacing);
        // Pixel footprint (ADR 0056): a wave under about two pixels per wavelength fades out of the shading instead of aliasing.
        let w_px = if p.footprint > 0.0 { smoothstep(PIXEL_FADE.0, PIXEL_FADE.1, lambda / p.footprint) } else { 1.0 };
        *o = w_tan * w_res * w_px * shore;
    }
    out
}

/// The reduced argument of wave `i`: `k . local + phase`, brought to `[-pi, pi]`.
pub fn wave_argument(w: &Wave, local: [f64; 3], phase: f64) -> f64 {
    let a = dot(w.k, local) + phase;
    a - TWO_PI * libm::floor(a / TWO_PI + 0.5)
}

/// Gerstner displacement and slope of a liquid at a point (the f64 reference of the shader's `water_waves`).
pub fn wave_sample(liquid: &LiquidAppearance, p: &WavePoint, choppy: f64) -> WaveSample {
    let weights = wave_weights(liquid, p);
    let mut disp = [0.0; 3];
    let mut slope = [0.0; 3];
    for (i, w) in liquid.waves.iter().enumerate() {
        if weights[i] <= 0.0 || w.amplitude <= 0.0 {
            continue;
        }
        let klen = len(w.k);
        let khat = scale(w.k, 1.0 / klen);
        let kn = dot(khat, p.up);
        let tang = sub(khat, scale(p.up, kn));
        let s = len(tang);
        let dir = scale(tang, 1.0 / s.max(1.0e-6));
        let arg = wave_argument(w, p.local, p.phases[i]);
        let (sn, cs) = (libm::sin(arg), libm::cos(arg));
        let aw = w.amplitude * weights[i];
        disp = add(disp, sub(scale(p.up, aw * cs), scale(dir, choppy * aw * sn)));
        slope = add(slope, scale(dir, -(aw * klen * s) * sn));
    }
    WaveSample { disp, slope }
}

/// The shading normal of a surface with the given up and wave slope.
pub fn wave_normal(up: [f64; 3], slope: [f64; 3]) -> [f64; 3] {
    normalize(sub(up, slope))
}

/// The class of a water pixel by its vertical depth (the debug view of the shoreline, CON-18, and the observable of the flicker
/// metric): discrete, a function of the depth alone, so it is the same on every adapter up to the pixels on a band edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WaterClass {
    /// Not water (depth at or below zero): the pass draws nothing.
    None = 0,
    /// The wet film: shallower than the shore fade.
    Film = 1,
    /// The foam band.
    Foam = 2,
    /// Shallow water, the seabed visible: up to 2 m.
    Shallow = 3,
    /// Water of 2 to 10 m.
    Mid = 4,
    /// Deep water.
    Deep = 5,
}

impl WaterClass {
    /// The colours of the debug view (linear display values before the pre-exposure).
    pub const COLOURS: [[f64; 3]; 6] =
        [[0.0, 0.0, 0.0], [0.9, 0.9, 0.2], [1.0, 1.0, 1.0], [0.1, 0.8, 0.8], [0.1, 0.35, 0.9], [0.05, 0.08, 0.35]];
}

/// Depth bands of the class view: the edge between shallow and mid water, and between mid and deep.
pub const CLASS_SHALLOW_M: f64 = 2.0;
pub const CLASS_MID_M: f64 = 10.0;

pub fn water_class(depth: f64, look: &WaterLook) -> WaterClass {
    if depth <= 0.0 {
        WaterClass::None
    } else if depth < look.shore_fade_m {
        WaterClass::Film
    } else if depth < look.foam_width_m {
        WaterClass::Foam
    } else if depth < CLASS_SHALLOW_M {
        WaterClass::Shallow
    } else if depth < CLASS_MID_M {
        WaterClass::Mid
    } else {
        WaterClass::Deep
    }
}

/// How much of the surface layer (reflection, specular) shows at `depth`: it fades in from the shore.
///
/// `width` is the filter width in metres of depth (the shader's `shore_filter x fwidth(depth)`): the fade is stretched by two widths, so
/// a fade narrower than a pixel spreads over about a pixel and the waterline itself stays at zero.
pub fn shore_cover(depth: f64, look: &WaterLook, width: f64) -> f64 {
    smoothstep(0.0, look.shore_fade_m + 2.0 * width, depth)
}

/// The share of foam at `depth`: zero at the shore line, densest a little inside, gone at the band's width; modulated by `modulation`
/// in [0, 1] (the swell's phase), never fully off.
///
/// `width` as in [`shore_cover`]: the rise is stretched by two widths and the fall's edges by one width each way, so a band narrower
/// than a pixel loses height instead of flickering.
pub fn foam_amount(depth: f64, look: &WaterLook, modulation: f64, width: f64) -> f64 {
    let w = look.foam_width_m;
    let f_in = smoothstep(0.0, 0.15 * w + 2.0 * width, depth);
    let f_out = 1.0 - smoothstep(0.15 * w - width, w + width, depth);
    look.foam_strength * f_in * f_out * (0.6 + 0.4 * modulation)
}

/// Everything the water shading takes from the lighting and the scene at one pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceInputs {
    /// Unit shading normal (waves included), unit view vector (surface to camera), unit up and unit sun direction.
    pub n: [f64; 3],
    pub v: [f64; 3],
    pub up: [f64; 3],
    pub sun: [f64; 3],
    /// Vertical depth (m), the wave modulation of the foam in [0, 1] and the filter width of the shore bands in metres of depth
    /// (`shore_filter x fwidth(depth)` of the pixel).
    pub depth: f64,
    pub foam_mod: f64,
    pub depth_width: f64,
    /// The scene under the water (the terrain pass's radiance at this pixel).
    pub seabed: [f64; 3],
    /// Radiance per unit albedo of the horizontal surface (sun and ambient), the sun's transmitted share (RGB) and the sky's
    /// radiance in the mirror direction.
    pub diffuse: [f64; 3],
    pub sun_t: [f64; 3],
    pub sky: [f64; 3],
}

/// The radiance leaving the water surface towards the viewer, before the pre-exposure (the f64 reference of the shader's
/// `water_surface`): the seabed seen through the water's extinction and the liquid body's own colour lit by the same light, the
/// Fresnel-weighted sky reflection and sun glint (GGX), the liquid's emission, faded in from the shore, then the foam.
pub fn surface_radiance(liquid: &LiquidAppearance, look: &WaterLook, i: &SurfaceInputs) -> [f64; 3] {
    let mu_v = dot(i.n, i.v).clamp(0.0, 1.0);
    let path_mu = dot(i.up, i.v).max(0.1);
    let p = i.depth / path_mu;
    let t: [f64; 3] = liquid.sigma.map(|s| libm::exp(-s * p));
    let t_mean = (t[0] + t[1] + t[2]) / 3.0;
    let one_minus = 1.0 - mu_v;
    let o2 = one_minus * one_minus;
    let fresnel = look.fresnel_f0 + (1.0 - look.fresnel_f0) * (o2 * o2 * one_minus);
    // Sun glint: GGX distribution at the half vector, the cosine to the light cancelling against the BRDF's denominator.
    let l = i.sun;
    let h = normalize(add(l, i.v));
    let nh = dot(i.n, h).max(0.0);
    let nl = dot(i.n, l).max(0.0);
    let a = look.glint_roughness * look.glint_roughness;
    let nxh = cross(i.n, h);
    let sin2 = dot(nxh, nxh);
    let denom = sin2 + a * a * nh * nh;
    let d = a * a / (core::f64::consts::PI * denom * denom);
    let spec_gate = (nl * 20.0).min(1.0);
    let spec_scale = fresnel * d / (4.0 * mu_v.max(0.05)) * spec_gate;
    let mut out = [0.0; 3];
    let cover = shore_cover(i.depth, look, i.depth_width);
    let foam = foam_amount(i.depth, look, i.foam_mod, i.depth_width);
    for c in 0..3 {
        let body = liquid.colour[c] * i.diffuse[c];
        let refracted = i.seabed[c] * t[c] + body * (1.0 - t[c]);
        let water = refracted * (1.0 - fresnel) + fresnel * i.sky[c] + spec_scale * i.sun_t[c] + liquid.emission[c] * (1.0 - t_mean);
        let surf = i.seabed[c] * (1.0 - cover) + water * cover;
        let foam_colour = 0.9 * i.diffuse[c];
        out[c] = surf * (1.0 - foam) + foam_colour * foam;
    }
    out
}

/// The `INV_PI` the lit terrain's lighting uses, exported so the plain (no atmosphere) lighting of the water matches the terrain's.
pub fn plain_diffuse(n: [f64; 3], sun: [f64; 3]) -> [f64; 3] {
    let v = dot(n, sun).max(0.0) * INV_PI;
    [v; 3]
}

// ---------------------------------------------------------------------------------------------------------------------------
// The coastal flicker metric (ADR 0041 point 6, ADR 0053)
// ---------------------------------------------------------------------------------------------------------------------------

use crate::camera::Camera;
use planet_core::Vec3;

/// The class of every pixel of a frame (a [`WaterClass`] as a `u8`, row-major, top row first): what a test makes from the water pass's
/// class view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassMap {
    pub size: (u32, u32),
    pub class: Vec<u8>,
}

impl ClassMap {
    pub fn new(size: (u32, u32), class: Vec<u8>) -> ClassMap {
        assert_eq!(class.len(), size.0 as usize * size.1 as usize, "a class per pixel");
        ClassMap { size, class }
    }

    fn at(&self, x: i64, y: i64) -> Option<u8> {
        (x >= 0 && y >= 0 && x < i64::from(self.size.0) && y < i64::from(self.size.1))
            .then(|| self.class[y as usize * self.size.0 as usize + x as usize])
    }
}

/// What [`class_flicker`] counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlickerReport {
    /// Water pixels of the first frame whose point on the water surface falls inside the second frame.
    pub covered: u64,
    /// Of those, the pixels the second frame shows no water at anywhere in the 3 x 3 pixels around the reprojected position (the
    /// water's coverage flickers: the metric of the budget).
    pub changed: u64,
    /// Of those, the pixels whose exact class the second frame does not show in those 3 x 3 pixels (informational: a band thinner
    /// than a pixel, the film and the foam at a grazing angle, comes and goes with sub-pixel motion by sampling, not by shading).
    pub changed_fine: u64,
    /// Water pixels whose point left the second frame (not counted).
    pub left_frame: u64,
    /// Pixels the second frame does not show because terrain nearer the camera hides the point (parallax, not flicker): judged by the
    /// `ground` test of [`class_flicker`].
    pub occluded: u64,
    /// Water pixels within [`HORIZON_PIXELS`] pixels' angle of the horizon, where a pixel is kilometres of water and the reprojection
    /// is unconditioned (not counted).
    pub unresolved: u64,
    /// The changed pixels by the class they had in the first frame (index = `WaterClass as usize`).
    pub changed_by_class: [u64; 6],
    /// The covered pixels by class.
    pub covered_by_class: [u64; 6],
}

impl FlickerReport {
    /// The share of covered pixels that changed class.
    pub fn share(&self) -> f64 {
        if self.covered == 0 {
            0.0
        } else {
            self.changed as f64 / self.covered as f64
        }
    }

    pub fn add(&mut self, o: FlickerReport) {
        self.covered += o.covered;
        self.changed += o.changed;
        self.changed_fine += o.changed_fine;
        self.left_frame += o.left_frame;
        self.unresolved += o.unresolved;
        self.occluded += o.occluded;
        for k in 0..6 {
            self.changed_by_class[k] += o.changed_by_class[k];
            self.covered_by_class[k] += o.covered_by_class[k];
        }
    }
}

/// Pixels' angle from the horizon within which a water pixel is not judged (its ground footprint is kilometres and the reprojection
/// of the point is unconditioned).
pub const HORIZON_PIXELS: f64 = 4.0;

/// The budget of the flicker metric: the share of covered pixels that may change water class between consecutive frames of a coastal
/// flight (ADR 0041 point 6: 0.1 %, "provisional, confirmed by the first run"; the first run is in the change that built the pass).
pub const FLICKER_BUDGET: f64 = 1.0e-3;

/// The point where the ray from `from` along `dir` first meets the sphere of radius `radius` about the planet's centre, if it does in
/// front of `from` (f64, planet-fixed).
fn ray_sphere(from: Vec3, dir: Vec3, radius: f64) -> Option<Vec3> {
    let b = from.dot(dir);
    // |from|^2 - radius^2 as (|from| - radius)(|from| + radius): the altitude is small against the radius.
    let l = from.length();
    let c = (l - radius) * (l + radius);
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let s = libm::sqrt(disc);
    let t = if c >= 0.0 { -b - s } else { -b + s };
    (t > 0.0).then(|| from + dir * t)
}

/// The share of water pixels that change water class between two frames after reprojection with the f64 camera poses (ADR 0041
/// point 6): each water pixel of `a` is a ray that meets the water surface (the sphere of `water_radius_m`, the sea level) at a point;
/// the point is projected into `b`; its class must be the class `b` shows there, within one pixel (the quantisation of the reprojected
/// position and the pixel-wide edge of a band). A point that leaves the frame is not counted. Waves move the surface by metres at most,
/// so the sphere is the surface for the class, which depends on the depth `W - h` and not on the waves.
pub fn class_flicker(
    a: &ClassMap,
    cam_a: &Camera,
    b: &ClassMap,
    cam_b: &Camera,
    water_radius_m: f64,
    ground: Option<&dyn Fn(Vec3) -> bool>,
) -> FlickerReport {
    let (w, h) = (f64::from(a.size.0), f64::from(a.size.1));
    let (right, up, fwd) = cam_a.ray_frame();
    let pixel_angle = 2.0 * libm::tan(0.5 * cam_a.fov_y_rad) / h;
    let mut report = FlickerReport::default();
    for y in 0..a.size.1 {
        for x in 0..a.size.0 {
            let class = a.class[y as usize * a.size.0 as usize + x as usize];
            let dry = class == WaterClass::None as u8;
            let (u, v) = (2.0 * (f64::from(x) + 0.5) / w - 1.0, 1.0 - 2.0 * (f64::from(y) + 0.5) / h);
            let dir = (fwd + right * u + up * v).normalized();
            let Some(p) = ray_sphere(cam_a.position.0, dir, water_radius_m) else { continue };
            // The grazing angle of the ray at the water: its sine is the ray's part along the local up.
            let sin_graze = (-dir.dot(p.normalized())).abs();
            if sin_graze < HORIZON_PIXELS * pixel_angle {
                report.unresolved += u64::from(!dry);
                continue;
            }
            let c = cam_b.project(p - cam_b.position.0);
            if c[3] <= 0.0 {
                report.left_frame += u64::from(!dry);
                continue;
            }
            let (nx, ny) = (c[0] / c[3], c[1] / c[3]);
            let (px, py) = ((nx * 0.5 + 0.5) * f64::from(b.size.0), (0.5 - ny * 0.5) * f64::from(b.size.1));
            if px < 0.0 || py < 0.0 || px >= f64::from(b.size.0) || py >= f64::from(b.size.1) {
                report.left_frame += u64::from(!dry);
                continue;
            }
            let (cx, cy) = (libm::floor(px) as i64, libm::floor(py) as i64);
            if dry {
                // A hole: this pixel shows no water, and the second frame shows water all around the same point. Terrain nearer this
                // camera that hides the point (a disocclusion) is not a hole.
                let all_water = (-1..=1).all(|dy| (-1..=1).all(|dx| b.at(cx + dx, cy + dy).is_some_and(|c| c != WaterClass::None as u8)));
                if all_water {
                    let hidden = ground.is_some_and(|inside| {
                        let from = cam_a.position.0;
                        (1..128).any(|k| inside(from + (p - from) * (f64::from(k) / 128.0)))
                    });
                    if hidden {
                        report.occluded += 1;
                    } else {
                        report.covered += 1;
                        report.changed += 1;
                        report.changed_fine += 1;
                    }
                }
                continue;
            }
            report.covered += 1;
            report.covered_by_class[usize::from(class).min(5)] += 1;
            let seen_class = (-1..=1).any(|dy| (-1..=1).any(|dx| b.at(cx + dx, cy + dy) == Some(class)));
            let seen = (-1..=1).any(|dy| (-1..=1).any(|dx| b.at(cx + dx, cy + dy).is_some_and(|c| c != WaterClass::None as u8)));
            if !seen_class {
                report.changed_fine += 1;
                report.changed_by_class[usize::from(class).min(5)] += 1;
            }
            if !seen {
                // A point that terrain nearer the second camera hides is parallax, not flicker: `ground(x)` says whether the planet-fixed
                // point `x` is inside the terrain, and the segment from the camera to the point is walked.
                if let Some(inside) = ground {
                    let from = cam_b.position.0;
                    if (1..128).any(|k| inside(from + (p - from) * (f64::from(k) / 128.0))) {
                        report.occluded += 1;
                        continue;
                    }
                }
                report.changed += 1;
            }
        }
    }
    report
}

/// [`class_flicker`] in both directions, summed: a pixel that appears in the second frame without a counterpart in the first (an
/// isolated flicker pixel) is found from the second frame's side, one that disappears from the first's.
pub fn class_flicker_both(
    a: &ClassMap,
    cam_a: &Camera,
    b: &ClassMap,
    cam_b: &Camera,
    water_radius_m: f64,
    ground: Option<&dyn Fn(Vec3) -> bool>,
) -> FlickerReport {
    let mut r = class_flicker(a, cam_a, b, cam_b, water_radius_m, ground);
    r.add(class_flicker(b, cam_b, a, cam_a, water_radius_m, ground));
    r
}

/// What [`lit_shore_change`] counted.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LitShoreReport {
    /// Shore-band pixels of the first frame whose point is judged (inside the second frame, resolved, not hidden).
    pub covered: u64,
    /// Of those, the pixels whose lit luminance changed by more than the threshold, relative to the larger of the two values.
    pub changed: u64,
}

impl LitShoreReport {
    pub fn share(&self) -> f64 {
        if self.covered == 0 {
            0.0
        } else {
            self.changed as f64 / self.covered as f64
        }
    }

    pub fn add(&mut self, o: LitShoreReport) {
        self.covered += o.covered;
        self.changed += o.changed;
    }
}

/// The lit-path temporal measure of the shore (ADR 0053 point 2): the **flash** of a shore pixel over three consecutive frames. For
/// the pixels of the middle frame whose class is in the shore band (`Film`, `Foam`, `Shallow` of `b_classes`), the point on the water
/// is reprojected with the f64 poses into the frame before and the frame after (bilinear at the reprojected position, so sub-pixel
/// motion is not a change); a pixel flashes when its lit luminance leaves the mean of the other two by more than `threshold` relative
/// to the largest of the three. A smooth change of the shading with the camera (Fresnel, the sun's angle, the seabed) is linear over
/// three frames and does not flash; a band that comes and goes with sub-pixel motion does. Calm sea, the same exclusions as the
/// coverage metric (horizon, left the frame, hidden by terrain via `ground`).
#[allow(clippy::too_many_arguments)]
pub fn lit_shore_flash(
    b_classes: &ClassMap,
    b_lum: &[f64],
    cam_b: &Camera,
    a: (&[f64], (u32, u32), &Camera),
    c: (&[f64], (u32, u32), &Camera),
    water_radius_m: f64,
    threshold: f64,
    ground: Option<&dyn Fn(Vec3) -> bool>,
) -> LitShoreReport {
    let (w, h) = (f64::from(b_classes.size.0), f64::from(b_classes.size.1));
    let (right, up, fwd) = cam_b.ray_frame();
    let pixel_angle = 2.0 * libm::tan(0.5 * cam_b.fov_y_rad) / h;
    let mut report = LitShoreReport::default();
    // The luminance of `frame` at the reprojection of `p`, bilinear; None when the point leaves it or is hidden.
    let sample = |frame: &(&[f64], (u32, u32), &Camera), p: Vec3| -> Option<f64> {
        let (lum, size, cam) = (frame.0, frame.1, frame.2);
        let q = cam.project(p - cam.position.0);
        if q[3] <= 0.0 {
            return None;
        }
        let (px, py) = ((q[0] / q[3] * 0.5 + 0.5) * f64::from(size.0), (0.5 - q[1] / q[3] * 0.5) * f64::from(size.1));
        if px < 0.5 || py < 0.5 || px >= f64::from(size.0) - 0.5 || py >= f64::from(size.1) - 0.5 {
            return None;
        }
        let (fx, fy) = (px - 0.5, py - 0.5);
        let (x0, y0) = (libm::floor(fx) as usize, libm::floor(fy) as usize);
        let (tx, ty) = (fx - libm::floor(fx), fy - libm::floor(fy));
        let at = |x: usize, y: usize| lum[y * size.0 as usize + x];
        Some((at(x0, y0) * (1.0 - tx) + at(x0 + 1, y0) * tx) * (1.0 - ty) + (at(x0, y0 + 1) * (1.0 - tx) + at(x0 + 1, y0 + 1) * tx) * ty)
    };
    for y in 0..b_classes.size.1 {
        for x in 0..b_classes.size.0 {
            let class = b_classes.class[y as usize * b_classes.size.0 as usize + x as usize];
            if !(WaterClass::Film as u8..=WaterClass::Shallow as u8).contains(&class) {
                continue;
            }
            let (u, v) = (2.0 * (f64::from(x) + 0.5) / w - 1.0, 1.0 - 2.0 * (f64::from(y) + 0.5) / h);
            let dir = (fwd + right * u + up * v).normalized();
            let Some(p) = ray_sphere(cam_b.position.0, dir, water_radius_m) else { continue };
            if (-dir.dot(p.normalized())).abs() < HORIZON_PIXELS * pixel_angle {
                continue;
            }
            let (Some(ya), Some(yc)) = (sample(&a, p), sample(&c, p)) else { continue };
            let yb = b_lum[y as usize * b_classes.size.0 as usize + x as usize];
            let rel = (yb - 0.5 * (ya + yc)).abs() / ya.max(yb).max(yc).max(1.0e-3);
            report.covered += 1;
            if rel > threshold {
                if let Some(inside) = ground {
                    let hidden = [cam_b.position.0, a.2.position.0, c.2.position.0]
                        .iter()
                        .any(|from| (1..128).any(|k| inside(*from + (p - *from) * (f64::from(k) / 128.0))));
                    if hidden {
                        report.covered -= 1;
                        continue;
                    }
                }
                report.changed += 1;
            }
        }
    }
    report
}

/// The relative flash of lit luminance in the shore band above which a pixel counts as flashing (a quarter: 0.3 stops, about the
/// smallest step a viewer sees as a flash in a moving picture).
pub const LIT_SHORE_THRESHOLD: f64 = 0.25;
