//! The fields a bake reads (m9-bake): the base elevation of the default class source with its class-blended `k`, and the carve factor of GEN-097, at a direction. One place for the
//! command and the tests, so that every bake of a seed reads the same function and keys it the same way.

use crate::classes::{
    blended_carve_factor, blended_k, ClassLattice, ClassSource, CLASS_CARVE, CLASS_K, CLASS_LATTICE_N, CLASS_LIST_VERSION,
    CLASS_SOURCE_VERSION,
};
use planet_core::Vec3;

/// The default class source of a seed and its class lattice.
pub struct BakeInputs {
    source: ClassSource,
    lattice: ClassLattice,
}

impl BakeInputs {
    /// Builds the class lattice of `seed` on `threads` threads (same result for any count).
    pub fn new(seed: u64, threads: usize) -> BakeInputs {
        let source = ClassSource::new(seed);
        let lattice = ClassLattice::build_with(&source, CLASS_LATTICE_N, threads);
        BakeInputs { source, lattice }
    }

    /// The base elevation at a direction: the class source's, with the class-blended relief driver.
    pub fn base(&self, d: Vec3) -> f64 {
        let p = self.source.eval(d);
        let k = if crate::bake::experiment::k_one() { 1.0 } else { blended_k(&self.lattice.weights_at(d)) };
        self.source.elevation_m(d, &p, k)
    }

    /// The class weights of the lattice at a direction (SPIKE-99: the uplift map and the per-class statistics).
    pub fn weights(&self, d: Vec3) -> [f64; 8] {
        self.lattice.weights_at(d)
    }

    /// The blended k at a direction (SPIKE-99: logged beside the statistics).
    pub fn k(&self, d: Vec3) -> f64 {
        blended_k(&self.lattice.weights_at(d))
    }

    /// The carve factor of GEN-097 at a direction.
    pub fn carve_factor(&self, d: Vec3) -> f64 {
        blended_carve_factor(&self.lattice.weights_at(d))
    }

    /// An identity of what the base and the carve factor read besides the seed: the class source's and class list's versions, the lattice size and the class tables of `k` and of the
    /// carve factor. Part of every layer key until the relief channels are a stored input of the run.
    pub fn id() -> u64 {
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        let mut put = |v: u64| h = (h ^ v).wrapping_mul(0x0000_0100_0000_01B3);
        put(u64::from(CLASS_SOURCE_VERSION));
        put(u64::from(CLASS_LIST_VERSION));
        put(CLASS_LATTICE_N as u64);
        CLASS_K.iter().chain(CLASS_CARVE.iter()).for_each(|v| put(v.to_bits()));
        h
    }
}
