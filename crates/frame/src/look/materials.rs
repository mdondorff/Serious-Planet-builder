//! The material weights of the ground (M7 G2): one function from a fixed vector of inputs to the weights of the ground materials, with the response
//! curves per material as look data. The detail of the ground at the lowest levels (rock, sand, grass, soil) is shaded by these weights; the
//! albedo of the base shading (ADR 0039) is a different function and is not changed by them.
//!
//! **The seam for the driver maps (M8).** The inputs are a vector of [`INPUTS`] numbers in a fixed order ([`Input`]), the same on the CPU and in
//! the shader. Today the slope (the cosine of the angle between the normal and up), the height above the water, the temperature, the precipitation
//! and the region's surface type are filled; soil depth, moisture and insolation are reserved and read 0 until a driver map supplies them. A new
//! driver is a new value in a reserved slot and new curves in the look: the shader's interface (the input vector and the curve table) does not
//! change, and no material is a branch of code.
//!
//! A material's weight is the product, over the inputs, of a smoothstep response `f(x; lo, hi)`: 0 at `lo`, 1 at `hi`, rising when `lo < hi`
//! and falling when `lo > hi`; a pair with `lo == hi` leaves the input out (factor 1). A material whose pairs are all equal is off (weight 0). The
//! weights are normalised to sum to 1 when any is positive; with none positive they are all 0 (no material, no detail).
//!
//! Class (ADR 0035): **presentation data**, nothing here is stored, keyed or read back. The shader evaluates the same function on the same f32
//! words ([`MaterialParams::to_words`]); a GPU test shows that the table drives the picture (REND-034).

use super::shading::smoothstep;

/// The materials the ground detail knows (slots of the curve table).
pub const MATERIALS: usize = 4;
/// The names of the material slots, in order.
pub const MATERIAL_NAMES: [&str; MATERIALS] = ["rock", "sand", "grass", "soil"];
/// The inputs of the weight function.
pub const INPUTS: usize = 8;
/// Number of `vec4<f32>` words the curve table occupies: per material, four words of two `(lo, hi)` pairs each.
pub const MATERIAL_VEC4S: usize = MATERIALS * INPUTS / 2;

/// The slots of the input vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Cosine of the angle between the surface normal and up, in `[0, 1]`.
    SlopeCos = 0,
    /// Height above the water level, metres.
    Height = 1,
    /// Temperature, degrees C.
    Temperature = 2,
    /// Precipitation, millimetres a year.
    Precipitation = 3,
    /// The region's surface type (the index of the type; 0 outside regions).
    SurfaceType = 4,
    /// Reserved for a driver map: soil depth.
    SoilDepth = 5,
    /// Reserved for a driver map: moisture.
    Moisture = 6,
    /// Reserved for a driver map: insolation.
    Insolation = 7,
}

/// The inputs of one ground point, in the order of [`Input`].
pub type MaterialInputs = [f64; INPUTS];

/// The response curves: per material, per input, `(lo, hi)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialParams {
    pub curves: [[[f64; 2]; INPUTS]; MATERIALS],
}

const OFF: [f64; 2] = [0.0, 0.0];

impl MaterialParams {
    /// The Earth default: rock on steep ground, sand where it is dry, grass where it is wet, both on flat ground; the thresholds are those of the
    /// base albedo (`AlbedoParams::EARTH`), so the detail and the colour agree on what is rock, desert and vegetation. Soil is off.
    pub const EARTH: MaterialParams = {
        let mut c = [[OFF; INPUTS]; MATERIALS];
        // Rock: falls from 0.86 (flat enough for soil) to 0.62 (bare rock) in the slope cosine.
        c[0][Input::SlopeCos as usize] = [0.86, 0.62];
        // Sand: flat ground (the cosine rising 0.62 to 0.86) and dry (precipitation falling 900 to 250 mm).
        c[1][Input::SlopeCos as usize] = [0.62, 0.86];
        c[1][Input::Precipitation as usize] = [900.0, 250.0];
        // Grass: flat ground and wet (precipitation rising 250 to 900 mm).
        c[2][Input::SlopeCos as usize] = [0.62, 0.86];
        c[2][Input::Precipitation as usize] = [250.0, 900.0];
        MaterialParams { curves: c }
    };

    /// Whether material `m` takes part: some pair of its curves has an interval.
    pub fn is_on(&self, m: usize) -> bool {
        self.curves[m].iter().any(|p| p[0] != p[1])
    }

    /// The response of one input: 0 at `lo`, 1 at `hi` (smoothstep between), 1 for an input that is left out.
    pub fn response(pair: [f64; 2], x: f64) -> f64 {
        if pair[0] == pair[1] {
            1.0
        } else {
            smoothstep(pair[0], pair[1], x)
        }
    }

    /// The weights of the materials at a ground point (see the module documentation).
    pub fn weights(&self, inputs: &MaterialInputs) -> [f64; MATERIALS] {
        let mut w = [0.0; MATERIALS];
        for (m, slot) in w.iter_mut().enumerate() {
            if self.is_on(m) {
                *slot = (0..INPUTS).map(|k| Self::response(self.curves[m][k], inputs[k])).product();
            }
        }
        let sum: f64 = w.iter().sum();
        if sum > 0.0 {
            for v in &mut w {
                *v /= sum;
            }
        }
        w
    }

    /// The table as the shader holds it: [`MATERIAL_VEC4S`] words of `vec4<f32>`, material by material, each input pair in turn (`lo`, `hi`), each
    /// value rounded once.
    pub fn to_words(&self) -> [f32; MATERIAL_VEC4S * 4] {
        let mut w = [0f32; MATERIAL_VEC4S * 4];
        for m in 0..MATERIALS {
            for k in 0..INPUTS {
                w[(m * INPUTS + k) * 2] = self.curves[m][k][0] as f32;
                w[(m * INPUTS + k) * 2 + 1] = self.curves[m][k][1] as f32;
            }
        }
        w
    }

    /// The same values rounded once to `f32` and read back as `f64`: what the shader sees.
    pub fn as_f32(&self) -> MaterialParams {
        let w = self.to_words();
        let mut c = [[OFF; INPUTS]; MATERIALS];
        for m in 0..MATERIALS {
            for k in 0..INPUTS {
                c[m][k] = [f64::from(w[(m * INPUTS + k) * 2]), f64::from(w[(m * INPUTS + k) * 2 + 1])];
            }
        }
        MaterialParams { curves: c }
    }

    /// Bit patterns of every value, material by material (the plan text form).
    pub fn bits(&self) -> [u64; MATERIALS * INPUTS * 2] {
        let mut out = [0u64; MATERIALS * INPUTS * 2];
        for m in 0..MATERIALS {
            for k in 0..INPUTS {
                out[(m * INPUTS + k) * 2] = self.curves[m][k][0].to_bits();
                out[(m * INPUTS + k) * 2 + 1] = self.curves[m][k][1].to_bits();
            }
        }
        out
    }

    /// Inverse of [`bits`](MaterialParams::bits).
    pub fn from_bits(b: &[u64; MATERIALS * INPUTS * 2]) -> MaterialParams {
        let mut c = [[OFF; INPUTS]; MATERIALS];
        for m in 0..MATERIALS {
            for k in 0..INPUTS {
                c[m][k] = [f64::from_bits(b[(m * INPUTS + k) * 2]), f64::from_bits(b[(m * INPUTS + k) * 2 + 1])];
            }
        }
        MaterialParams { curves: c }
    }

    /// Every value must be finite; the first violation names the key (`materials.<material>.<input>`) and the value.
    pub fn validate(&self) -> Result<(), (String, String, String)> {
        const INPUT_NAMES: [&str; INPUTS] =
            ["slope_cos", "height", "temperature", "precipitation", "surface_type", "soil_depth", "moisture", "insolation"];
        for (material, row) in MATERIAL_NAMES.iter().zip(&self.curves) {
            for (input, pair) in INPUT_NAMES.iter().zip(row) {
                if !(pair[0].is_finite() && pair[1].is_finite() && (pair[0] as f32).is_finite() && (pair[1] as f32).is_finite()) {
                    return Err((
                        format!("materials.{material}.{input}"),
                        format!("{pair:?}"),
                        "both values of a response curve must be finite numbers that fit f32".into(),
                    ));
                }
                // The shader decides "left out" on the f32 words: a pair that differs only below f32 would be an interval here and none there.
                if pair[0] != pair[1] && (pair[0] as f32) == (pair[1] as f32) {
                    return Err((
                        format!("materials.{material}.{input}"),
                        format!("{pair:?}"),
                        "the two values of a response curve must differ as f32 numbers, or be equal (the input is left out)".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// The strength of the ground detail the window starts with and F6 calls "natural" (the owner's tuning pass sets it; 1 is full).
pub const NATURAL_STRENGTH: f64 = 0.6;

/// Number of roughness modifiers (the wet and the icy ground).
pub const GLOSS_MODIFIERS: usize = 2;
/// Number of values of [`GlossParams`]: the roughness of each material, four per modifier, and `f0`.
pub const GLOSS_VALUES: usize = MATERIALS + 4 * GLOSS_MODIFIERS + 1;
/// Number of `vec4<f32>` words the roughness table occupies: the roughness of the materials, then one per modifier.
pub const GLOSS_VEC4S: usize = 1 + GLOSS_MODIFIERS;

/// The specular term of the ground (M7 G2b, LOOK-010): a GGX highlight of the sun with a Schlick Fresnel of the base reflectance `f0`. The
/// roughness of a point is the weights' mean of the materials' roughness (rock and soil rough, sand and grass less so), scaled by the
/// modifiers: each is `[input, lo, hi, factor]`, the roughness is multiplied by `mix(1, factor, s)` for the smoothstep response `s` of the input
/// between `lo` and `hi` (`lo == hi` leaves the modifier out). The wet ground (the reserved moisture input, until a driver map exists it reads 0
/// and the modifier is out) and the icy ground (temperature falling from 0 to -5 C) are glossy this way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlossParams {
    pub roughness: [f64; MATERIALS],
    pub modifiers: [[f64; 4]; GLOSS_MODIFIERS],
    pub f0: f64,
}

impl GlossParams {
    pub const EARTH: GlossParams = GlossParams {
        roughness: [0.85, 0.65, 0.75, 0.80],
        modifiers: [[Input::Moisture as usize as f64, 0.0, 0.0, 0.3], [Input::Temperature as usize as f64, 0.0, -5.0, 0.4]],
        f0: 0.04,
    };

    /// The roughness at a ground point with the material `weights` (summing to 1, or all 0) and the `inputs`; `None` when there is no material.
    pub fn roughness_at(&self, weights: &[f64; MATERIALS], inputs: &MaterialInputs) -> Option<f64> {
        let sum: f64 = weights.iter().sum();
        if sum <= 0.0 {
            return None;
        }
        let mut r: f64 = weights.iter().zip(&self.roughness).map(|(w, r)| w * r).sum::<f64>() / sum;
        for m in &self.modifiers {
            if m[1] != m[2] {
                let s = super::shading::smoothstep(m[1], m[2], inputs[m[0] as usize]);
                r *= 1.0 + (m[3] - 1.0) * s;
            }
        }
        Some(r)
    }

    /// The table as the shader holds it: [`GLOSS_VEC4S`] words of `vec4<f32>` (the roughness of the materials, then each modifier), each value
    /// rounded once. `f0` goes with the strength.
    pub fn to_words(&self) -> [f32; GLOSS_VEC4S * 4] {
        let mut w = [0f32; GLOSS_VEC4S * 4];
        for (i, r) in self.roughness.iter().enumerate() {
            w[i] = *r as f32;
        }
        for (j, m) in self.modifiers.iter().enumerate() {
            for (k, v) in m.iter().enumerate() {
                w[4 * (1 + j) + k] = *v as f32;
            }
        }
        w
    }

    pub fn bits(&self) -> [u64; GLOSS_VALUES] {
        let mut out = [0u64; GLOSS_VALUES];
        for (i, v) in self.roughness.iter().chain(self.modifiers.iter().flatten()).chain([&self.f0]).enumerate() {
            out[i] = v.to_bits();
        }
        out
    }

    pub fn from_bits(b: &[u64; GLOSS_VALUES]) -> GlossParams {
        let f = |i: usize| f64::from_bits(b[i]);
        GlossParams {
            roughness: [0, 1, 2, 3].map(f),
            modifiers: [0, 1].map(|j| [0, 1, 2, 3].map(|k| f(MATERIALS + 4 * j + k))),
            f0: f(GLOSS_VALUES - 1),
        }
    }

    /// Every value must be finite, the roughness in `(0, 1]`, a modifier's input one of the [`INPUTS`], its factor in `(0, 1]` and `f0` in `[0, 1]`;
    /// a modifier pair that differs only below `f32` is refused (the shader decides on the `f32` words). The first violation names the key.
    pub fn validate(&self) -> Result<(), (String, String, String)> {
        let bad = |key: String, v: String, why: &str| Err((key, v, why.to_string()));
        for (name, r) in MATERIAL_NAMES.iter().zip(&self.roughness) {
            if !(*r > 0.0 && *r <= 1.0) {
                return bad(format!("gloss.roughness.{name}"), format!("{r}"), "the roughness must be a number above 0 and up to 1");
            }
        }
        for (j, m) in self.modifiers.iter().enumerate() {
            let key = |what: &str| format!("gloss.modifier{j}.{what}");
            if !m.iter().all(|v| v.is_finite() && (*v as f32).is_finite()) {
                return bad(key("value"), format!("{m:?}"), "every value of a modifier must be a finite number that fits f32");
            }
            if m[0].fract() != 0.0 || !(0.0..INPUTS as f64).contains(&m[0]) {
                return bad(key("input"), format!("{}", m[0]), "the input must be the index of one of the inputs");
            }
            if m[1] != m[2] && (m[1] as f32) == (m[2] as f32) {
                return bad(
                    key("curve"),
                    format!("{:?}", [m[1], m[2]]),
                    "the two values of a curve must differ as f32 numbers, or be equal (the modifier is out)",
                );
            }
            if !(m[3] > 0.0 && m[3] <= 1.0) {
                return bad(key("factor"), format!("{}", m[3]), "the factor must be a number above 0 and up to 1");
            }
        }
        if !(0.0..=1.0).contains(&self.f0) {
            return bad("gloss.f0".into(), format!("{}", self.f0), "the base reflectance must be a number from 0 to 1");
        }
        Ok(())
    }
}

/// The strength of the ground detail the editor window shows (VIEW-046): F6 cycles it. The window starts at `Natural`; `test-render` and the
/// goldens never set it (the plan's strength is 0 there).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroundDetailMode {
    Off,
    #[default]
    Natural,
    Full,
}

impl GroundDetailMode {
    /// The next one in the cycle (natural, full, off, natural).
    pub fn next(self) -> GroundDetailMode {
        match self {
            GroundDetailMode::Natural => GroundDetailMode::Full,
            GroundDetailMode::Full => GroundDetailMode::Off,
            GroundDetailMode::Off => GroundDetailMode::Natural,
        }
    }

    /// The strength of the lit plan (LOOK-009).
    pub fn strength(self) -> f64 {
        match self {
            GroundDetailMode::Off => 0.0,
            GroundDetailMode::Natural => NATURAL_STRENGTH,
            GroundDetailMode::Full => 1.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            GroundDetailMode::Off => "off",
            GroundDetailMode::Natural => "natural",
            GroundDetailMode::Full => "full",
        }
    }
}
