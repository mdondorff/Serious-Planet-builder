//! The fields a bake reads (m9-bake): the base elevation of the default class source with its class-blended `k`, and the carve factor of GEN-097, at a direction. One place for the
//! command and the tests, so that every bake of a seed reads the same function and keys it the same way.

use super::carve::Layer;
use crate::classes::{
    blended_carve_factor, blended_k, ClassLattice, ClassSource, CLASS_CARVE, CLASS_K, CLASS_LATTICE_N, CLASS_LIST_VERSION,
    CLASS_SOURCE_VERSION,
};
use crate::lithology::{Lithology, LITHOLOGY_VERSION};
use crate::uplift::{Uplift, UPLIFT_VERSION};
use planet_core::Vec3;

/// The default class source of a seed and its class lattice.
pub struct BakeInputs {
    source: ClassSource,
    lattice: ClassLattice,
    lithology: Lithology,
    uplift: Uplift,
    /// The factor that makes the land mean of K at the surface 1 ([`k_norm_of`]).
    k_norm: f64,
}

impl BakeInputs {
    /// Builds the class lattice of `seed` on `threads` threads (same result for any count).
    pub fn new(seed: u64, threads: usize) -> BakeInputs {
        let source = ClassSource::new(seed);
        let lattice = ClassLattice::build_with(&source, CLASS_LATTICE_N, threads);
        let lithology = Lithology::new(&source, threads);
        let uplift = Uplift::new(source.seed());
        let mut inputs = BakeInputs { source, lattice, lithology, uplift, k_norm: 1.0 };
        inputs.k_norm = k_norm_of(&inputs);
        inputs
    }

    /// The seed of the class source (a bake that samples it must be the world of this seed).
    pub fn seed(&self) -> u64 {
        self.source.seed()
    }

    /// The class weights at a direction (the lattice read; the class of a cell is the largest).
    pub fn weights_at(&self, d: Vec3) -> [f64; crate::classes::CLASS_COUNT] {
        self.lattice.weights_at(d)
    }

    /// The base elevation at a direction: the class source's, with the class-blended relief driver.
    pub fn base(&self, d: Vec3) -> f64 {
        let p = self.source.eval(d);
        self.source.elevation_m(d, &p, blended_k(&self.lattice.weights_at(d)))
    }

    /// The carve factor of GEN-097 at a direction.
    pub fn carve_factor(&self, d: Vec3) -> f64 {
        blended_carve_factor(&self.lattice.weights_at(d))
    }

    /// The erodibility K of the lithology at a direction and a height with the lower edge of its band (GEN-116, GEN-127).
    pub fn rock_k(&self, d: Vec3, z: f64) -> Layer {
        let s = self.lithology.strata(d, z);
        Layer { k: s.k * self.k_norm, floor: s.lower_m }
    }

    /// The uplift `U / K` at K 1 at a direction (GEN-123).
    pub fn uplift_at(&self, d: Vec3) -> f64 {
        self.uplift.at(self.lithology.provinces(), d)
    }

    /// The lithology field the bake reads.
    pub fn lithology(&self) -> &Lithology {
        &self.lithology
    }

    /// An identity of what the base, the carve factor, the lithology and the uplift read besides the seed: the class source's and class list's versions, the lattice size and the class tables of `k` and of the
    /// carve factor. Part of every layer key until the relief channels are a stored input of the run.
    pub fn id() -> u64 {
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        let mut put = |v: u64| h = (h ^ v).wrapping_mul(0x0000_0100_0000_01B3);
        put(u64::from(CLASS_SOURCE_VERSION));
        put(u64::from(CLASS_LIST_VERSION));
        put(CLASS_LATTICE_N as u64);
        put(u64::from(LITHOLOGY_VERSION));
        put(u64::from(UPLIFT_VERSION));
        put(K_NORM_SAMPLES as u64);
        CLASS_K.iter().chain(CLASS_CARVE.iter()).for_each(|v| put(v.to_bits()));
        h
    }
}

/// The equal-area sample of directions (a Fibonacci spiral) whose land cells give the mean of K that is normalised to 1.
pub const K_NORM_SAMPLES: usize = 200_000;

/// `1 / mean` of the lithology's erodibility over the land of the sample, read at the base elevation: the lithology varies the erosion in space and the carve keeps today's overall rate (GEN-127).
/// Serial, fixed order, `libm`: the same bits for any thread count. 1 when the sample holds no land.
fn k_norm_of(inputs: &BakeInputs) -> f64 {
    let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
    let (mut sum, mut land) = (0.0, 0usize);
    for i in 0..K_NORM_SAMPLES {
        let y = 1.0 - 2.0 * (i as f64 + 0.5) / K_NORM_SAMPLES as f64;
        let r = libm::sqrt(1.0 - y * y);
        let (s, c) = libm::sincos(golden * i as f64);
        let d = Vec3::new(r * c, y, r * s);
        let z = inputs.base(d);
        if z > 0.0 {
            sum += inputs.lithology.k_at(d, z);
            land += 1;
        }
    }
    if land == 0 || sum <= 0.0 {
        1.0
    } else {
        land as f64 / sum
    }
}

impl BakeInputs {
    /// The factor that normalises the lithology's erodibility to a land mean of 1 at the surface (for the preview text and the tests).
    pub fn k_norm(&self) -> f64 {
        self.k_norm
    }
}
