//! The world names its height field by implementation id (GEN-018, owner's decision of 2026-10-13).
//!
//! A definition may carry `world.height_field` (world-def stores the string; it cannot know the implementations, CON-02).
//! [`resolve`] turns it into a [`HeightFieldKind`]: the eroded field (GEN-017) when none is named, the named one when the
//! build knows the id, otherwise an error that lists the known ids. [`build_field`] builds the field of a body over a bake.

use crate::erosion::{
    ErosionGenerator, EROSION_C1_IMPLEMENTATION_ID, EROSION_C1_LAYER_IMPLEMENTATION_ID, EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID,
    EROSION_IMPLEMENTATION_ID, EROSION_LAYER_IMPLEMENTATION_ID, EROSION_LAYER_ONLY_IMPLEMENTATION_ID,
};
use crate::erosion_layer::ErosionLayer;
use crate::global::LatticeSource;
use crate::height::{HeightField, HeightGenerator, ReferenceGenerator, IMPLEMENTATION_ID as REFERENCE_IMPLEMENTATION_ID};
use crate::relief_lattice::ReliefLattice;
use crate::terrain::{TerrainGenerator, TERRAIN_IMPLEMENTATION_ID};
use planet_world_def::{BodyId, GenContext, MissingNode, WorldDefinition};
use std::sync::Arc;

/// The height fields a build knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeightFieldKind {
    /// `cpu-erosion-v1` (GEN-017): the bake plus the erosion filter. Reads a bake.
    Eroded,
    /// `cpu-erosion-v2` (GEN-107): the eroded field with the bake read by a C1 interpolant (no slope breaks at the lattice edges). Reads a bake.
    ErodedC1,
    /// `cpu-terrain-v1` (GEN-016): the bake plus fBm detail. Reads a bake.
    Terrain,
    /// `cpu-ref-height-v2`: the seed-only value-noise reference every golden was made with. Reads no bake.
    Reference,
}

/// The field a world gets when it names none.
pub const DEFAULT_HEIGHT_FIELD: HeightFieldKind = HeightFieldKind::ErodedC1;

impl HeightFieldKind {
    pub const ALL: [HeightFieldKind; 4] =
        [HeightFieldKind::Eroded, HeightFieldKind::Terrain, HeightFieldKind::Reference, HeightFieldKind::ErodedC1];

    /// The implementation id the world names this field by.
    pub fn id(self) -> &'static str {
        match self {
            HeightFieldKind::Eroded => EROSION_IMPLEMENTATION_ID,
            HeightFieldKind::ErodedC1 => EROSION_C1_IMPLEMENTATION_ID,
            HeightFieldKind::Terrain => TERRAIN_IMPLEMENTATION_ID,
            HeightFieldKind::Reference => REFERENCE_IMPLEMENTATION_ID,
        }
    }

    /// Whether the field reads the global bake (the reference does not).
    pub fn needs_bake(self) -> bool {
        !matches!(self, HeightFieldKind::Reference)
    }

    pub fn from_id(id: &str) -> Option<HeightFieldKind> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }
}

/// Every known id, the default first.
pub fn known_ids() -> [&'static str; 4] {
    [DEFAULT_HEIGHT_FIELD.id(), HeightFieldKind::Eroded.id(), HeightFieldKind::Terrain.id(), HeightFieldKind::Reference.id()]
}

/// The erosion layer of a run that turns it on (GEN-112), with the relief channels when the run has them (GEN-099; none: `k` is 1 and `m` is not read): a run-time choice, not part
/// of the world definition (paths are machine-local), applied to the eroded fields by [`build_field_erosion`].
#[derive(Clone)]
pub struct ErosionSwitch {
    pub relief: Option<Arc<ReliefLattice>>,
    pub layer: Arc<ErosionLayer>,
    /// The sea level of the definition: the crease band and the coast fade follow it.
    pub sea_level_m: f64,
    /// The baked channels the tiles' driver block is filled from (GEN-119); none: the neutral block.
    pub channels: Option<Arc<dyn crate::channels::ChannelSource>>,
}

impl ErosionSwitch {
    /// The implementation id of `kind`'s field with the switch on; the kind's own id for the kinds the switch does not apply to.
    pub fn implementation_id(&self, kind: HeightFieldKind) -> &'static str {
        match (kind, self.relief.is_some()) {
            (HeightFieldKind::Eroded, true) => EROSION_LAYER_IMPLEMENTATION_ID,
            (HeightFieldKind::ErodedC1, true) => EROSION_C1_LAYER_IMPLEMENTATION_ID,
            (HeightFieldKind::Eroded, false) => EROSION_LAYER_ONLY_IMPLEMENTATION_ID,
            (HeightFieldKind::ErodedC1, false) => EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID,
            (other, _) => other.id(),
        }
    }
}

/// A definition named an id this build does not know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownHeightField {
    pub given: String,
}

impl core::fmt::Display for UnknownHeightField {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "unknown height field '{}' (world.height_field): known ids are {}; the default is {}",
            self.given,
            known_ids().join(", "),
            DEFAULT_HEIGHT_FIELD.id()
        )
    }
}

impl std::error::Error for UnknownHeightField {}

/// The height field a definition selects: the default when it names none.
pub fn resolve(def: &WorldDefinition) -> Result<HeightFieldKind, UnknownHeightField> {
    match def.height_field() {
        None => Ok(DEFAULT_HEIGHT_FIELD),
        Some(id) => HeightFieldKind::from_id(id).ok_or_else(|| UnknownHeightField { given: id.to_string() }),
    }
}

/// [`build_field`] with the erosion layer on, and the relief channels when the switch has them (GEN-099, GEN-112): an error for a kind the switch does not apply to, or when the field refuses the order of the calls.
pub fn build_field_erosion<'a>(
    kind: HeightFieldKind,
    ctx: &GenContext,
    body: BodyId,
    bake: &'a dyn LatticeSource,
    bake_key: [u8; 16],
    erosion: &ErosionSwitch,
) -> Result<Box<dyn HeightField + 'a>, String> {
    let missing = |e: MissingNode| format!("the definition cannot be generated: {e}");
    let field = match kind {
        HeightFieldKind::Eroded => ErosionGenerator.field(ctx, body, bake, bake_key).map_err(missing)?,
        HeightFieldKind::ErodedC1 => ErosionGenerator.field_c1(ctx, body, bake, bake_key).map_err(missing)?,
        other => return Err(format!("--erosion applies to the eroded fields, the world's is {}", other.id())),
    };
    let field = match &erosion.relief {
        Some(r) => field.with_relief(r.clone())?,
        None => field,
    };
    let field = field.with_layer(erosion.layer.clone(), erosion.sea_level_m)?;
    Ok(Box::new(match &erosion.channels {
        Some(c) => field.with_channels(c.clone()),
        None => field,
    }))
}

/// The field of `body` of kind `kind` over `bake` (whose stage key is `bake_key`; the reference field reads neither). `ctx` must
/// be fresh: the field's key covers everything the context recorded.
pub fn build_field<'a>(
    kind: HeightFieldKind,
    ctx: &GenContext,
    body: BodyId,
    bake: &'a dyn LatticeSource,
    bake_key: [u8; 16],
) -> Result<Box<dyn HeightField + 'a>, MissingNode> {
    Ok(match kind {
        HeightFieldKind::Eroded => Box::new(ErosionGenerator.field(ctx, body, bake, bake_key)?),
        HeightFieldKind::ErodedC1 => Box::new(ErosionGenerator.field_c1(ctx, body, bake, bake_key)?),
        HeightFieldKind::Terrain => Box::new(TerrainGenerator.field(ctx, body, bake, bake_key)?),
        HeightFieldKind::Reference => ReferenceGenerator.field(ctx, body)?,
    })
}
