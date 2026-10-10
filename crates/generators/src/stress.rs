//! Test-only stress terrain (M2 gate, TEST-015): a steep ridged field with about 8 km of relief and fine-scale slopes,
//! used to check that cracks, morphing, borders and jitter hold on terrain the smooth reference generator never
//! produces. Never used by production paths; its mesh keys are salted so they cannot alias reference meshes.

use crate::height::{value_noise, HeightField};
use planet_core::Vec3;

/// Peak `|height|` of [`Steep`] in metres.
pub const STEEP_AMPLITUDE_M: f64 = 8000.0;
const OCTAVES: u32 = 16;
const BASE_FREQUENCY: f64 = 8.0;
/// Octave gain above 0.5: the amplitude falls slower than the wavelength, so fine octaves dominate the slope.
const GAIN: f64 = 0.7;
/// Domain tag so the stress lattice never correlates with the reference lattice for the same seed.
const STEEP_TAG: u64 = 0x5354_4545_5021;

/// Ridged multifractal: `2·(1-|n|)² - 1` per octave, 16 octaves from 8 to 262,144 cycles per radian
/// (finest wavelength about 150 m on an Earth-sized sphere).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Steep(pub u64);

impl HeightField for Steep {
    fn height(&self, dir: Vec3) -> f64 {
        let seed = self.0 ^ STEEP_TAG;
        let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, BASE_FREQUENCY);
        for octave in 0..OCTAVES {
            let r = 1.0 - value_noise(seed, octave, dir * freq).abs();
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
    fn key_salt(&self) -> Option<u64> {
        Some(STEEP_TAG)
    }
    fn max_abs_height_m(&self) -> f64 {
        STEEP_AMPLITUDE_M
    }
}
