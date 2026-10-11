//! Test-only stress terrain (M2 gate, TEST-015), compiled only with the `stress` feature (dev-dependencies): a steep
//! ridged field (`|h| <= 8 km`, about -5.5 to +6 km in practice, 20 m slopes up to about 72 degrees) used to check that
//! cracks, morphing, borders and jitter hold on terrain the smooth reference generator never produces. Its own
//! implementation id and version enter its height and mesh keys, so its artefacts never alias reference ones.

use crate::height::{layer_noise, layer_noise_bounds, HeightField};
use planet_core::Vec3;

/// Peak `|height|` of [`Steep`] in metres.
pub const STEEP_AMPLITUDE_M: f64 = 8000.0;
const OCTAVES: u32 = 16;
const BASE_FREQUENCY: f64 = 8.0;
/// Octave gain above 0.5: the amplitude falls slower than the wavelength, so fine octaves dominate the slope.
const GAIN: f64 = 0.7;
/// Lattice layer of the stress field: its lattice values never coincide with the reference height layer's.
const LAYER_STEEP: u64 = 0x5354_4545_5021;
/// Bump whenever [`Steep`]'s output changes (ADR 0006); part of its height and mesh keys.
pub const STEEP_VERSION: u32 = 1;
/// Names the stress field in keys.
pub const STEEP_IMPLEMENTATION_ID: &str = "test-steep-ridged-v1";

/// Ridged multifractal: `2·(1-|n|)² - 1` per octave, 16 octaves from 8 to 262,144 cycles per radian
/// (finest lattice cell about 24 m on an Earth-sized sphere).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Steep(pub u64);

impl HeightField for Steep {
    fn height(&self, dir: Vec3) -> f64 {
        let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, BASE_FREQUENCY);
        for octave in 0..OCTAVES {
            let r = 1.0 - layer_noise(self.0, LAYER_STEEP, octave, dir * freq).abs();
            sum += amp * (2.0 * r * r - 1.0);
            norm += amp;
            amp *= GAIN;
            freq *= 2.0;
        }
        STEEP_AMPLITUDE_M * sum / norm
    }
    fn seed(&self) -> u64 {
        self.0
    }
    fn implementation_id(&self) -> &'static str {
        STEEP_IMPLEMENTATION_ID
    }
    fn version(&self) -> u32 {
        STEEP_VERSION
    }
    fn max_abs_height_m(&self) -> f64 {
        STEEP_AMPLITUDE_M
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (mut lo, mut hi, mut amp, mut norm, mut freq) = (0.0, 0.0, 1.0, 0.0, BASE_FREQUENCY);
        for octave in 0..OCTAVES {
            let (a, b) = layer_noise_bounds(self.0, LAYER_STEEP, octave, centre, angular_radius, freq);
            // |n| in [near, far]; the octave term 2 (1 - |n|)^2 - 1 falls as |n| grows.
            let far = a.abs().max(b.abs());
            let near = if a <= 0.0 && b >= 0.0 { 0.0 } else { a.abs().min(b.abs()) };
            lo += amp * (2.0 * (1.0 - far) * (1.0 - far) - 1.0);
            hi += amp * (2.0 * (1.0 - near) * (1.0 - near) - 1.0);
            norm += amp;
            amp *= GAIN;
            freq *= 2.0;
        }
        (STEEP_AMPLITUDE_M * lo / norm - 1e-6, STEEP_AMPLITUDE_M * hi / norm + 1e-6)
    }
}
