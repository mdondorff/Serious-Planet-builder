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
//! words ([`MaterialParams::to_words`]); the declared bound of the GPU weights is `1e-5` per weight against this module on the same words.

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
                if !(pair[0].is_finite() && pair[1].is_finite()) {
                    return Err((
                        format!("materials.{material}.{input}"),
                        format!("{pair:?}"),
                        "both values of a response curve must be finite numbers".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}
