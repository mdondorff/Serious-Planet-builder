//! The uplift field: an analytic tectonic uplift for the erosion carve (GEN-123; planner inbox #70, owner 2026-11-05).
//!
//! `U(d)` is the uplift of one carve step in units of `k_dt`, that is the ratio `U / K` at `K = 1` (today's uniform `u_over_k` is 20). It is high in the active belts and on the arc coasts, medium
//! in the back-arc, about 0 on the cratons. CPU-canonical: `libm`, integer hashing, the same bits for any thread count (CON-14); **never stored**: a pure function of the seed, the direction, the
//! province lattice of the lithology field ([`ProvinceLattice`]) and [`UPLIFT_VERSION`], which a consumer folds into its keys. All numbers are start values; the owner's preview round decides them.
//!
//! The carve reads it per face cell as `k_dt * U * taper` (the sea taper multiplies it), not scaled by the carve factor or by K, so rock of different hardness settles at different slopes.

use crate::global::fbm;
use crate::lithology::{ProvinceLattice, Provinces};
use planet_core::Vec3;

/// Bump whenever the field's output changes for a seed (the formula, the constants or the province terms it reads); a consumer folds it into its keys.
pub const UPLIFT_VERSION: u32 = 1;

/// The ratio `U / K` of a craton, of a flat (neither craton nor belt), of a rift shoulder, of a back-arc and of an active belt or arc coast; today's uniform value is 20.
pub const U_CRATON: f64 = 2.0;
pub const U_FLAT: f64 = 24.0;
pub const U_RIFT: f64 = 28.0;
pub const U_BACKARC: f64 = 32.0;
pub const U_BELT: f64 = 70.0;
/// The amplitude of the multiplying noise (`1 + NOISE_AMPLITUDE * noise`, noise in `[-1, 1]`) and its frequency.
pub const NOISE_AMPLITUDE: f64 = 0.2;
const NOISE_FREQUENCY: f64 = 3.0;
const LAYER_UPLIFT: u64 = 0x5550_0001;

/// The names of the four terms of [`Uplift::candidates`].
pub const TERM_NAMES: [&str; 4] = ["flat or craton", "belt or arc coast", "back-arc", "rift"];

/// The uplift field of a seed.
#[derive(Clone, Copy, Debug)]
pub struct Uplift {
    seed: u64,
}

impl Uplift {
    pub fn new(seed: u64) -> Uplift {
        Uplift { seed }
    }

    /// The values of the four terms for the province terms `p` (the order of [`TERM_NAMES`]): the flat/craton blend, the belt, the back-arc, the rift.
    pub fn candidates(p: &Provinces) -> [f64; 4] {
        [U_FLAT * (1.0 - p.craton) + U_CRATON * p.craton, U_BELT * p.core, U_BACKARC * p.back_arc, U_RIFT * p.rift]
    }

    /// The uplift for the province terms `p` and the noise `noise` in `[-1, 1]`: the largest of the [`Uplift::candidates`], times `1 + NOISE_AMPLITUDE * noise`. Finite and not negative.
    pub fn of_terms(p: &Provinces, noise: f64) -> f64 {
        let u = Self::candidates(p).into_iter().fold(0.0, f64::max);
        u * (1.0 + NOISE_AMPLITUDE * noise.clamp(-1.0, 1.0))
    }

    /// The uplift at the unit direction `d`, the terms read from `provinces` (the same lattice as the lithology's).
    pub fn at(&self, provinces: &ProvinceLattice, d: Vec3) -> f64 {
        Self::of_terms(&provinces.at(d), fbm(self.seed, LAYER_UPLIFT, d, NOISE_FREQUENCY, 2))
    }
}
