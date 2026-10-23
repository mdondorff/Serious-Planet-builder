//! The height-field chain of a world, GPU-free (M8 R1): the base field of the definition's kind over a bake, the stamp stages of its
//! layers over it (GEN-026, ADR 0007) and the optional water bound. One home for what the viewer, the headless modes and the tests
//! assemble; `app` only wires it. Also the bake sources that need nothing but this crate.

use std::sync::Arc;

use planet_core::Face;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

use crate::global::{GlobalGrid, GlobalSample, GridSource, LatticeSource, ReadError};
use crate::height::HeightField;
use crate::height_fields::{build_field, HeightFieldKind};
use crate::ocean::{OceanGrid, OceanLattice};
use crate::water::WaterBounded;

/// The reference field reads no bake; this stands in for one.
pub struct NoBake;

impl LatticeSource for NoBake {
    fn res(&self) -> u32 {
        1
    }

    fn at(&self, _: Face, _: u32, _: u32) -> Result<GlobalSample, ReadError> {
        Err(ReadError::Source("this height field reads no bake".into()))
    }
}

/// An in-memory bake the run owns.
pub struct OwnedBake(pub GlobalGrid);

impl LatticeSource for OwnedBake {
    fn res(&self) -> u32 {
        GridSource(&self.0).res()
    }

    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        GridSource(&self.0).at(face, i, j)
    }
}

/// The in-memory ocean channel.
pub struct OwnedOcean(pub OceanGrid);

impl OceanLattice for OwnedOcean {
    fn res(&self) -> u32 {
        self.0.res
    }

    fn is_ocean(&self, face: Face, i: u32, j: u32) -> Result<bool, ReadError> {
        self.0.is_ocean(face, i, j)
    }
}

/// The field of the planet of `def`: the base of kind `kind` over `src` (whose stage key is `bake_key`), the stamp stages of the
/// layers over it, and with `bound_water` the water-level bound over all (a fresh context per field: its key covers what it read).
pub fn field_chain<'a>(
    kind: HeightFieldKind,
    def: &Arc<WorldDefinition>,
    src: &'a dyn LatticeSource,
    bake_key: [u8; 16],
    bound_water: bool,
) -> Result<Box<dyn HeightField + 'a>, String> {
    let ctx = GenContext::new(def);
    let built = build_field(kind, &ctx, BodyId::PLANET, src, bake_key)
        .map_err(|e| format!("the definition cannot be generated: {e}"))
        .and_then(|base| with_stamps(base, def));
    match (built, bound_water) {
        (Ok(f), true) => {
            let level = def.sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
            Ok(Box::new(WaterBounded::new(f, level)))
        }
        (built, _) => built,
    }
}

/// `base` with the stamp stages of the definition's layers over it, lowest layer first (GEN-026, ADR 0007); the base itself when the
/// definition holds no stamp. The stages read the snapshot `def`, so the field keeps what it was built from.
pub fn with_stamps<'a>(base: Box<dyn HeightField + 'a>, def: &Arc<WorldDefinition>) -> Result<Box<dyn HeightField + 'a>, String> {
    use crate::stamp::{ItemReader, StampStage, STAMP_STAGE_VERSION};
    let layers = def.stamp_layers(BodyId::PLANET);
    let Some((top, below)) = layers.split_last() else { return Ok(base) };
    let stage = |field: Arc<dyn HeightField + 'a>, layer: &str, index: usize| {
        let items: Arc<dyn ItemReader> = def.clone();
        StampStage::new(field, items, BodyId::PLANET, layer, index as u32, STAMP_STAGE_VERSION)
            .ok_or_else(|| "the definition has stamps but no usable radius".to_string())
    };
    // The index of a stage is its layer's place in the stack of ADR 0007 (a layer outside it follows the known ones, in name order), not
    // its place among the layers that hold stamps: a layer added below shifts no index above it.
    let unknown: Vec<&String> = layers.iter().filter(|l| !planet_world_def::LAYER_STACK.contains(&l.as_str())).collect();
    let index_of = |layer: &str| {
        planet_world_def::LAYER_STACK.iter().position(|s| *s == layer).unwrap_or_else(|| {
            planet_world_def::LAYER_STACK.len() + unknown.iter().position(|u| u.as_str() == layer).expect("a layer of the list")
        })
    };
    let mut field: Arc<dyn HeightField + 'a> = Arc::from(base);
    for layer in below {
        field = Arc::new(stage(field, layer, index_of(layer))?);
    }
    Ok(Box::new(stage(field, top, index_of(top))?))
}
