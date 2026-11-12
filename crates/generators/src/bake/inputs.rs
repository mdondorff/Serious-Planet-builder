//! The fields a bake reads (m9-bake): the base elevation of the default class source with its class-blended `k`, and the carve factor of GEN-097, at a direction. One place for the
//! command and the tests, so that every bake of a seed reads the same function and keys it the same way.

use super::carve::Layer;
use crate::arid::{AridInputs, AridRules, ARID_RULES_VERSION};
use crate::classes::{
    blended_carve_factor, blended_k, ClassLattice, ClassSource, CLASS_CARVE, CLASS_K, CLASS_LATTICE_N, CLASS_LIST_VERSION,
    CLASS_SOURCE_VERSION,
};
use crate::lithology::{Lithology, ProvinceLattice, LITHOLOGY_VERSION, ROCK_K_SCALE};
use crate::orography::{Orography, OROGRAPHY_VERSION, SEASONS};
use crate::runoff::{discharge_weight, runoff_mm, vegetation_factor, RUNOFF_VERSION};
use crate::uplift::{Uplift, UPLIFT_VERSION};
use planet_core::Vec3;

/// The default class source of a seed and its class lattice.
pub struct BakeInputs {
    source: ClassSource,
    lattice: ClassLattice,
    uplift: Uplift,
    threads: usize,
    orography_cells: usize,
    orography: std::sync::OnceLock<Orography>,
    lithology: std::sync::OnceLock<Lithology>,
    arid: AridRules,
}

impl BakeInputs {
    /// Builds the class lattice of `seed` on `threads` threads (same result for any count).
    pub fn new(seed: u64, threads: usize) -> BakeInputs {
        let source = ClassSource::new(seed).with_coast_threads(threads);
        let lattice = ClassLattice::build_with(&source, CLASS_LATTICE_N, threads);
        let uplift = Uplift::new(source.seed());
        BakeInputs {
            source,
            lattice,
            threads,
            orography_cells: crate::orography::default_cells(),
            orography: std::sync::OnceLock::new(),
            lithology: std::sync::OnceLock::new(),
            uplift,
            arid: AridRules::earth(),
        }
    }

    /// The orographic climate of the seed (GEN-117), built on first use from the class source's base elevation `e0` (never from the class weights) on the threads of [`BakeInputs::new`]: the
    /// precipitation of the global field and, later, the aridity of the class rules and the discharge of the carve read it.
    pub fn orography(&self) -> &Orography {
        self.orography.get_or_init(|| Orography::build(self.source.seed(), &self.source, self.orography_cells, &SEASONS, self.threads))
    }

    /// The seed of the class source (a bake that samples it must be the world of this seed).
    pub fn seed(&self) -> u64 {
        self.source.seed()
    }

    /// The lithology of the seed (GEN-116), built on first use: its hardness and province terms feed the arid class rules. The province lattice has the cells of the climate's, a power of two up
    /// to the class lattice's.
    pub fn lithology(&self) -> &Lithology {
        self.lithology.get_or_init(|| {
            let cells = self.orography_cells.next_power_of_two().min(CLASS_LATTICE_N);
            Lithology::with_provinces(self.source.seed(), ProvinceLattice::build_with(&self.source, cells, self.threads))
        })
    }

    /// The class lattice (the weights without the arid rules, on its nodes).
    pub fn class_lattice(&self) -> &ClassLattice {
        &self.lattice
    }

    /// The class weights of the class lattice at a direction, without the arid rules (the class of a cell is the largest).
    pub fn base_weights_at(&self, d: Vec3) -> [f64; crate::classes::CLASS_COUNT] {
        self.lattice.weights_at(d)
    }

    /// The class weights at a direction: the lattice's with the arid rules (GEN-128) blended over them.
    ///
    /// The base elevation and the aridity are read from the climate's lattice (bilinear, about 20 km a cell in production), not evaluated at the point: the rules are regional, and a lattice read
    /// keeps `weights_at` cheap; the land ramp fades the weights in over about 100 m of lattice height, so a coastal point reads them smoothly.
    pub fn weights_at(&self, d: Vec3) -> [f64; crate::classes::CLASS_COUNT] {
        let (climate, lithology) = (self.orography(), self.lithology());
        let inputs = AridInputs {
            aridity: climate.aridity(d),
            hardness: lithology.hardness(d),
            e0: climate.base_elevation_m(d),
            provinces: lithology.provinces().at(d),
        };
        self.arid.blend(&self.lattice.weights_at(d), &inputs)
    }

    /// An identity of what the climate of a class-source world reads besides the seed and the class source: the orographic climate's version and the cells a face edge of its lattice
    /// ([`crate::orography::default_cells`]: the production lattice, or the coarse one of a test build). Part of the global store's class-source identity, so that a change of the climate's
    /// constants or of its lattice re-keys the stored climate; [`BakeInputs::id`] folds it too, because the arid rules make the base elevation read the climate.
    pub fn climate_id() -> u64 {
        climate_identity(OROGRAPHY_VERSION, crate::orography::default_cells())
    }

    /// The base elevation at a direction: the class source's, with the class-blended relief driver.
    pub fn base(&self, d: Vec3) -> f64 {
        let p = self.source.eval(d);
        self.source.elevation_m(d, &p, blended_k(&self.weights_at(d)))
    }

    /// The carve factor of GEN-097 at a direction.
    pub fn carve_factor(&self, d: Vec3) -> f64 {
        blended_carve_factor(&self.weights_at(d))
    }

    /// The erodibility K of the lithology at a direction and a height with the lower edge of its band (GEN-116, GEN-127).
    pub fn rock_k(&self, d: Vec3, z: f64) -> Layer {
        let s = self.lithology().strata(d, z);
        Layer { k: s.k * ROCK_K_SCALE, floor: s.lower_m }
    }

    /// The erodibility of the lithology times the vegetation factor of the local precipitation (GEN-129): what a bake with the climate reads as K.
    pub fn climate_k(&self, d: Vec3, z: f64) -> Layer {
        let l = self.rock_k(d, z);
        Layer { k: l.k * vegetation_factor(self.orography().precipitation_mm(d)), floor: l.floor }
    }

    /// The runoff weight of the carve at a direction (GEN-129): the runoff of the local precipitation and temperature over the reference runoff, within the bounds.
    pub fn runoff_weight(&self, d: Vec3) -> f64 {
        let o = self.orography();
        discharge_weight(runoff_mm(o.precipitation_mm(d), o.temperature_c(d)))
    }

    /// The uplift `U / K` at K 1 at a direction (GEN-123).
    pub fn uplift_at(&self, d: Vec3) -> f64 {
        self.uplift.at(self.lithology().provinces(), d)
    }

    /// An identity of what the base, the carve factor, the lithology and the uplift read besides the seed: the class source's and class list's versions, the lattice size and the class tables of `k` and of the
    /// carve factor. Part of every layer key until the relief channels are a stored input of the run.
    pub fn id() -> u64 {
        Self::id_with(LITHOLOGY_VERSION, UPLIFT_VERSION, ROCK_K_SCALE, RUNOFF_VERSION)
    }

    /// [`BakeInputs::id`] for the given lithology and uplift versions and K scale (a test of what a change of each re-keys).
    pub fn id_with(lithology_version: u32, uplift_version: u32, k_scale: f64, runoff_version: u32) -> u64 {
        Self::id_with_rules(lithology_version, uplift_version, k_scale, runoff_version, &crate::classes::band_rules())
    }

    /// [`BakeInputs::id_with`] under the given band rules (GEN-179): the identity moves with each rule and its value, and is unchanged while both are off.
    pub fn id_with_rules(
        lithology_version: u32,
        uplift_version: u32,
        k_scale: f64,
        runoff_version: u32,
        rules: &crate::classes::BandRules,
    ) -> u64 {
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        let mut put = |v: u64| h = (h ^ v).wrapping_mul(0x0000_0100_0000_01B3);
        put(u64::from(CLASS_SOURCE_VERSION));
        put(u64::from(CLASS_LIST_VERSION));
        put(CLASS_LATTICE_N as u64);
        // The base elevation reads the class weights, which read the arid rules and, through them, the climate: their identities are the layer's.
        put(arid_identity(ARID_RULES_VERSION, LITHOLOGY_VERSION, Self::climate_id()));
        put(u64::from(lithology_version));
        put(u64::from(uplift_version));
        put(k_scale.to_bits());
        put(u64::from(runoff_version));
        put(crate::runoff::constants_identity());
        CLASS_K.iter().chain(CLASS_CARVE.iter()).for_each(|v| put(v.to_bits()));
        // The turned lattices of the bands (GEN-140): the word is unchanged while the switch is off.
        let h = rules.fold(h);
        crate::height::turned_noise_word(h)
    }
}

/// The identity word of the arid rules of `version` over a climate of identity `climate` (a layer's base elevation reads both).
fn arid_identity(version: u32, lithology_version: u32, climate: u64) -> u64 {
    planet_core::hash::hash_u64s(0x6172_6964, &[u64::from(version), u64::from(lithology_version), climate])
}

/// The identity word of a climate of `version` on a lattice of `cells` cells a face edge.
fn climate_identity(version: u32, cells: usize) -> u64 {
    planet_core::hash::hash_u64s(0x636c_696d, &[u64::from(version), cells as u64])
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: GEN-117
    #[test]
    fn the_climate_identity_moves_with_the_orographic_version_and_the_lattice() {
        let base = climate_identity(OROGRAPHY_VERSION, OROGRAPHY_CELLS_FOR_TEST);
        assert_ne!(base, climate_identity(OROGRAPHY_VERSION + 1, OROGRAPHY_CELLS_FOR_TEST), "a new version re-keys the climate");
        assert_ne!(base, climate_identity(OROGRAPHY_VERSION, OROGRAPHY_CELLS_FOR_TEST * 2), "a finer lattice re-keys the climate");
        assert_eq!(BakeInputs::climate_id(), climate_identity(OROGRAPHY_VERSION, crate::orography::default_cells()));
    }

    const OROGRAPHY_CELLS_FOR_TEST: usize = 512;

    // spec: GEN-128
    #[test]
    fn the_layer_identity_moves_with_the_rules_version_and_the_climate_identity() {
        let climate = climate_identity(OROGRAPHY_VERSION, OROGRAPHY_CELLS_FOR_TEST);
        let base = arid_identity(ARID_RULES_VERSION, LITHOLOGY_VERSION, climate);
        let moved = |version: u32, lithology: u32, climate: u64| arid_identity(version, lithology, climate) != base;
        assert!(moved(ARID_RULES_VERSION + 1, LITHOLOGY_VERSION, climate), "a new rules version re-keys the layers");
        assert!(moved(ARID_RULES_VERSION, LITHOLOGY_VERSION + 1, climate), "a new lithology re-keys the layers");
        assert!(
            moved(ARID_RULES_VERSION, LITHOLOGY_VERSION, climate_identity(OROGRAPHY_VERSION + 1, OROGRAPHY_CELLS_FOR_TEST)),
            "a new climate re-keys the layers"
        );
        assert!(
            moved(ARID_RULES_VERSION, LITHOLOGY_VERSION, climate_identity(OROGRAPHY_VERSION, OROGRAPHY_CELLS_FOR_TEST * 2)),
            "a finer climate lattice re-keys the layers"
        );
    }
}
