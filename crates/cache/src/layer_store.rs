//! The erosion layer's pass 1 in a store (STRM-065, GEN-100; m9-bake PR 4): one record per cube face holding the face's change (carved minus base), checkpointed per face so that
//! a killed bake resumes from its last complete face, and refused when it was made for another bake or other parameters.
//!
//! Record kind 12 ([`KIND_LAYER_FACE`]) filed under the face's root tile. The key is the bake's stage key (the source), the bake's resolution, and under tag 15 the layer's
//! version, the face grid's cells, the radius, the sea level, the seed, every parameter of the carve, and `inputs_id` (an identity of what the base field and the carve factor read: the
//! class weights and relief channels, the erosion field's key). The payload is `codec (u8) | cells (u32 LE) | halo (u32 LE) | key (16 bytes) |` the face's `f32` plane of `cells + 2 halo` cells a side (row `j` outer, then `i`, little-endian).
//! A face found under another key is an error naming both keys, not a silent recompute (ADR 0006: a changed input is another layer, never a mix).

use std::path::Path;

use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::{pass1_face_rock, FaceChange, FaceSpec, RockFields};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_LAYER, PARAM_RESOLUTION};
use crate::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};
use crate::relief_store::face_tile;

/// The kind byte of a face's pass-1 change.
pub const KIND_LAYER_FACE: u8 = Kind::LayerFace as u8;
/// Version of the layer's record layout and of the meaning of its key. History: 1 = PR 4 first push, 2 = the carve's area cap and incise-only flag joined the key (a store of version 1 is refused as another key and is deleted by hand).
pub const LAYER_VERSION: u32 = 2;
const LAYER_IMPLEMENTATION_ID: &str = "erosion-layer-face";
const CODEC_VERSION: u8 = 1;
const HEADER_LEN: usize = 1 + 4 + 4 + 16;

/// What a layer is made of besides the bake it is beside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerSpec {
    /// Cells per face edge of pass 1's grid.
    pub cells: usize,
    pub radius_m: f64,
    pub sea_level: f64,
    pub seed: u64,
    pub carve: CarveParams,
    /// An identity of the inputs of the base field and the carve factor that the bake's key does not cover (the relief channels' hash, the class lattice's, the erosion field's).
    pub inputs_id: u64,
    /// Whether the carve factor of GEN-097 is applied (it is part of the key; `run_pass1` refuses a factor that disagrees).
    pub uses_factor: bool,
    /// Which rock fields the carves read (`RockFields::mask`: bit 0 the erodibility, bit 1 the uplift, bit 2 the runoff weights, bit 3 the vegetation factor), part of the key: a store written with other fields is another key (GEN-127).
    pub rock: u8,
    /// Cells carved beyond each edge of a face ([`FaceSpec::halo`]); part of the key.
    pub halo: usize,
}

impl LayerSpec {
    pub(crate) fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(120);
        b.extend(LAYER_VERSION.to_le_bytes());
        b.extend(planet_generators::bake::BAKE_KERNEL_VERSION.to_le_bytes());
        b.push(u8::from(self.uses_factor));
        b.push(self.rock);
        b.extend((self.halo as u64).to_le_bytes());
        b.extend((self.cells as u64).to_le_bytes());
        for v in [self.radius_m, self.sea_level] {
            b.extend(v.to_bits().to_le_bytes());
        }
        b.extend(self.seed.to_le_bytes());
        let c = &self.carve;
        for v in [c.u_over_k, c.k_dt, c.epsilon, c.breach_slope, c.breach_max_m, c.breach_height_scale_m, c.uplift_taper_m] {
            b.extend(v.to_bits().to_le_bytes());
        }
        b.extend(c.max_area.to_bits().to_le_bytes());
        b.push(u8::from(c.incise_only));
        b.push(u8::from(c.keep_pits));
        // The physical cell metric (GEN-138) enters the key only when on: a layer of the uniform metric keeps its key.
        if c.cell_metric {
            b.push(0xC3);
        }
        // The round kernels (GEN-141) likewise: a byte for the MFD clip and one for the flood rule, only when one is on.
        if !c.round_rules.is_default() {
            b.extend([0xC4, c.round_rules.clip_mfd as u8, u8::from(c.round_rules.canonical_flood)]);
        }
        b.extend((c.steps as u64).to_le_bytes());
        b.extend((c.reroute_every as u64).to_le_bytes());
        b.extend(self.inputs_id.to_le_bytes());
        b
    }

    fn face(&self, face: Face) -> FaceSpec {
        FaceSpec { face, cells: self.cells, radius_m: self.radius_m, sea_level: self.sea_level, seed: self.seed, halo: self.halo }
    }
}

/// Why a layer could not be used. Every message names the record and what was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerStoreError(pub String);

impl std::fmt::Display for LayerStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn face_key(world: &GlobalWorld, res: u32, spec: &LayerSpec, face: Face) -> CacheKey {
    let mut inputs = world.base_key_inputs(Kind::LayerFace, face_tile(face), res, LAYER_IMPLEMENTATION_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, (spec.cells as u32).to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_LAYER, spec.bytes());
    cache_key(&inputs)
}

/// The record of a face's change under `spec`.
pub fn face_record(world: &GlobalWorld, res: u32, spec: &LayerSpec, change: &FaceChange) -> Record {
    assert!(
        (change.cells, change.halo) == (spec.cells, spec.halo),
        "the change of face {} has {} cells and halo {}, the spec {} and {}",
        change.face.0,
        change.cells,
        change.halo,
        spec.cells,
        spec.halo
    );
    let key = face_key(world, res, spec, change.face);
    let mut payload = Vec::with_capacity(HEADER_LEN + 4 * change.change.len());
    payload.push(CODEC_VERSION);
    payload.extend((change.cells as u32).to_le_bytes());
    payload.extend((change.halo as u32).to_le_bytes());
    payload.extend(key.0);
    for v in &change.change {
        payload.extend(v.to_le_bytes());
    }
    Record {
        key,
        kind: KIND_LAYER_FACE,
        tile: face_tile(change.face),
        generator_version: planet_generators::global::GLOBAL_VERSION,
        hash_version: planet_core::hash::HASH_VERSION,
        node_set_id: world.node_set_id(),
        payload,
    }
}

/// The stored change of `face` under `spec`: `Ok(None)` when the store holds no pass-1 record of the face; an error naming both keys when it holds one made under another key;
/// an error naming the face when the record is damaged.
pub fn load_face_change(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &LayerSpec,
    face: Face,
) -> Result<Option<FaceChange>, LayerStoreError> {
    let key = face_key(world, res, spec, face);
    let what = |why: String| LayerStoreError(format!("layer face {} (record kind {KIND_LAYER_FACE}) key {key}: {why}", face.0));
    match store.get(KIND_LAYER_FACE, face_tile(face), key).map_err(|e| what(e.to_string()))? {
        Some(r) => {
            let p = &r.payload;
            let plane = (spec.cells + 2 * spec.halo).pow(2);
            if p.len() != HEADER_LEN + 4 * plane
                || p[0] != CODEC_VERSION
                || u32::from_le_bytes(p[1..5].try_into().expect("four bytes")) as usize != spec.cells
                || u32::from_le_bytes(p[5..9].try_into().expect("four bytes")) as usize != spec.halo
            {
                return Err(what(format!(
                    "a payload of {} bytes that is not {} cells and halo {} in codec {CODEC_VERSION} ({} bytes)",
                    p.len(),
                    spec.cells,
                    spec.halo,
                    HEADER_LEN + 4 * plane
                )));
            }
            if p[9..25] != key.0 {
                return Err(what("its payload names another key than its record".into()));
            }
            let change = p[HEADER_LEN..].as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
            Ok(Some(FaceChange { face, cells: spec.cells, halo: spec.halo, change }))
        }
        None => match store.keys_for(KIND_LAYER_FACE, face_tile(face)).first() {
            Some(other) => Err(what(format!(
                "the store holds this face under key {other}, made for another bake or other parameters: refused, not mixed"
            ))),
            None => Ok(None),
        },
    }
}

/// What a run did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass1Report {
    pub computed: Vec<Face>,
    pub skipped: Vec<Face>,
}

/// Pass 1 over `faces`, one at a time, into the layer store in `dir`: a face that is stored under the current key is skipped (a resume), the others are computed and appended
/// with the index written after each (the checkpoint). `base` and `factor` are the base field and the carve factor (GEN-097) at a direction. A face stored under another key is an error.
#[allow(clippy::too_many_arguments)]
pub fn run_pass1(
    dir: &Path,
    world: &GlobalWorld,
    res: u32,
    spec: &LayerSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    faces: &[Face],
    threads: usize,
) -> Result<Pass1Report, LayerStoreError> {
    run_pass1_rock(dir, world, res, spec, base, factor, &RockFields::none(), faces, threads)
}

/// [`run_pass1`] with the rock fields of GEN-127 (the erodibility and the uplift), which the key of the run must name (`inputs_id`).
#[allow(clippy::too_many_arguments)]
pub fn run_pass1_rock(
    dir: &Path,
    world: &GlobalWorld,
    res: u32,
    spec: &LayerSpec,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    factor: Option<&(dyn Fn(Vec3) -> f64 + Sync)>,
    rock: &RockFields,
    faces: &[Face],
    threads: usize,
) -> Result<Pass1Report, LayerStoreError> {
    let err = |e: crate::pack::PackError| LayerStoreError(format!("the layer store {}: {e}", dir.display()));
    let mut writer = PackWriter::open(dir, world.params(), DEFAULT_MAX_PACK_BYTES).map_err(err)?;
    let store = TileStore::open(dir, world.params()).map_err(err)?;
    if spec.uses_factor != factor.is_some() {
        return Err(LayerStoreError(format!(
            "the layer spec says uses_factor = {} but the carve factor given is {}",
            spec.uses_factor,
            if factor.is_some() { "present" } else { "absent" }
        )));
    }
    if spec.rock != rock.mask() {
        return Err(LayerStoreError(format!(
            "the layer spec says rock = {} but the rock fields given are {} (bit 0 the erodibility, bit 1 the uplift, bit 2 the runoff weights, bit 3 the vegetation factor)",
            spec.rock,
            rock.mask()
        )));
    }
    let mut report = Pass1Report { computed: Vec::new(), skipped: Vec::new() };
    for &face in faces {
        if report.computed.contains(&face) || report.skipped.contains(&face) {
            continue;
        }
        let have_change = load_face_change(&store, world, res, spec, face)?.is_some();
        let have_area = crate::layer_channels::has_area(&store, world, res, spec, face)?;
        if have_change && have_area {
            report.skipped.push(face);
            continue;
        }
        // A face stored before the channels existed is carved again for its area only; what is stored is not written twice.
        let (change, area) = pass1_face_rock(&spec.face(face), base, factor, rock, &spec.carve, threads);
        if !have_change {
            writer.append(&face_record(world, res, spec, &change)).map_err(err)?;
        }
        if !have_area {
            writer.append(&crate::layer_channels::area_record(world, res, spec, face, &area)).map_err(err)?;
        }
        writer.write_index().map_err(err)?;
        report.computed.push(face);
    }
    writer.finish().map_err(err)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: GEN-100
    #[test]
    fn every_breach_parameter_is_part_of_the_layer_key() {
        let base = LayerSpec {
            cells: 64,
            radius_m: 6.0e6,
            sea_level: 0.0,
            seed: 1,
            carve: CarveParams::default(),
            inputs_id: 7,
            uses_factor: true,
            rock: 0,
            halo: 4,
        };
        let key = base.bytes();
        for carve in [
            CarveParams { breach_max_m: 50.0, ..base.carve },
            CarveParams { breach_height_scale_m: 2000.0, ..base.carve },
            CarveParams { breach_slope: 2e-3, ..base.carve },
            CarveParams { keep_pits: false, ..base.carve },
            CarveParams { cell_metric: true, ..base.carve },
            CarveParams { round_rules: planet_generators::bake::route::RoundRules::ROUND, ..base.carve },
            CarveParams {
                round_rules: planet_generators::bake::route::RoundRules { canonical_flood: true, ..Default::default() },
                ..base.carve
            },
        ] {
            assert_ne!(LayerSpec { carve, ..base }.bytes(), key, "a breach parameter is missing from the key: {carve:?}");
        }
    }
}
