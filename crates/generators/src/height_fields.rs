//! The world names its height field by implementation id (GEN-018, owner's decision of 2026-10-13).
//!
//! A definition may carry `world.height_field` (world-def stores the string; it cannot know the implementations, CON-02).
//! [`resolve`] turns it into a [`HeightFieldKind`]: the eroded field (GEN-017) when none is named, the named one when the
//! build knows the id, otherwise an error that lists the known ids. [`build_field`] builds the field of a body over a bake.

use crate::erosion::{ErosionGenerator, EROSION_IMPLEMENTATION_ID};
use crate::global::LatticeSource;
use crate::height::{HeightField, HeightGenerator, ReferenceGenerator, IMPLEMENTATION_ID as REFERENCE_IMPLEMENTATION_ID};
use crate::terrain::{TerrainGenerator, TERRAIN_IMPLEMENTATION_ID};
use planet_world_def::{BodyId, GenContext, MissingNode, WorldDefinition};

/// The height fields a build knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeightFieldKind {
    /// `cpu-erosion-v1` (GEN-017): the bake plus the erosion filter. Reads a bake.
    Eroded,
    /// `cpu-terrain-v1` (GEN-016): the bake plus fBm detail. Reads a bake.
    Terrain,
    /// `cpu-ref-height-v2`: the seed-only value-noise reference every golden was made with. Reads no bake.
    Reference,
}

/// The field a world gets when it names none.
pub const DEFAULT_HEIGHT_FIELD: HeightFieldKind = HeightFieldKind::Eroded;

impl HeightFieldKind {
    pub const ALL: [HeightFieldKind; 3] = [HeightFieldKind::Eroded, HeightFieldKind::Terrain, HeightFieldKind::Reference];

    /// The implementation id the world names this field by.
    pub fn id(self) -> &'static str {
        match self {
            HeightFieldKind::Eroded => EROSION_IMPLEMENTATION_ID,
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
pub fn known_ids() -> [&'static str; 3] {
    [DEFAULT_HEIGHT_FIELD.id(), HeightFieldKind::Terrain.id(), HeightFieldKind::Reference.id()]
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
        HeightFieldKind::Terrain => Box::new(TerrainGenerator.field(ctx, body, bake, bake_key)?),
        HeightFieldKind::Reference => ReferenceGenerator.field(ctx, body)?,
    })
}
