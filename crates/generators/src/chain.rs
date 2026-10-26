//! The height-field chain of a world, GPU-free (M8 R1): the base field of the definition's kind over a bake, the stamp stages of its
//! layers over it (GEN-026, ADR 0007) and the optional water bound. One home for what the viewer, the headless modes and the tests
//! assemble; `app` only wires it. Also the bake sources that need nothing but this crate.

use std::sync::Arc;

use planet_core::cube::FaceMapping;
use planet_core::hash::mix64;
use planet_core::{Face, TileId, Vec3};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

use crate::climate::ClimateTexture;
use crate::coupled::{lens_climate, lens_version_word, CoupledLens, CoupledMaps};
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
    field_chain_with(kind, def, src, bake_key, bound_water, None)
}

/// [`field_chain`] with the chunk stage of the chain (GEN-039): the base, the chunk stage over it, then the stamp stages.
pub fn field_chain_with<'a>(
    kind: HeightFieldKind,
    def: &Arc<WorldDefinition>,
    src: &'a dyn LatticeSource,
    bake_key: [u8; 16],
    bound_water: bool,
    chunks: Option<&crate::chunkfield::ChunkLayer>,
) -> Result<Box<dyn HeightField + 'a>, String> {
    // The layers whose chunk stage is `Under` (the default, GEN-039) put their stamps below the chunk stage, the rest above it.
    let under = |layer: &str| def.chunk_stage(BodyId::PLANET, layer) == planet_world_def::ChunkStagePosition::Under;
    let ctx = GenContext::new(def);
    let built = build_field(kind, &ctx, BodyId::PLANET, src, bake_key)
        .map_err(|e| format!("the definition cannot be generated: {e}"))
        .and_then(|base| with_stamps_where(base, def, &under))
        .map(|base| match chunks {
            Some(c) => c.wrap(base),
            None => base,
        })
        .and_then(|base| with_stamps_where(base, def, &|layer: &str| !under(layer)));
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
    with_stamps_where(base, def, &|_: &str| true)
}

/// [`with_stamps`] for the layers `keep` names only (the layers below or above the chunk stage, GEN-039); every layer keeps its own stage index.
pub fn with_stamps_where<'a>(
    base: Box<dyn HeightField + 'a>,
    def: &Arc<WorldDefinition>,
    keep: &dyn Fn(&str) -> bool,
) -> Result<Box<dyn HeightField + 'a>, String> {
    use crate::stamp::{ItemReader, StampStage, STAMP_STAGE_VERSION};
    let all = def.stamp_layers(BodyId::PLANET);
    let layers: Vec<String> = all.iter().filter(|l| keep(l)).cloned().collect();
    let Some((top, below)) = layers.split_last() else { return Ok(base) };
    let stage = |field: Arc<dyn HeightField + 'a>, layer: &str, index: usize| {
        let items: Arc<dyn ItemReader> = def.clone();
        StampStage::new(field, items, BodyId::PLANET, layer, index as u32, STAMP_STAGE_VERSION)
            .ok_or_else(|| "the definition has stamps but no usable radius".to_string())
    };
    // The index of a stage is its layer's place in the stack of ADR 0007 (a layer outside it follows the known ones, in name order), not
    // its place among the layers that hold stamps: a layer added below shifts no index above it.
    let unknown: Vec<&String> = all.iter().filter(|l| !planet_world_def::LAYER_STACK.contains(&l.as_str())).collect();
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

/// `base` seen through the coupled run (M8-1 part c, GEN-043): every height, key and reading is the base's, but the climate texture of a tile
/// shows the run's heights or drainage area (the temperature and precipitation views), and the mesh key carries the run and the lens, so
/// switching the lens re-keys the tiles and the streamer refreshes them (STRM-026). The debug view of the global network.
pub struct CoupledLensField<'a> {
    inner: Box<dyn HeightField + 'a>,
    maps: Arc<CoupledMaps>,
    lens: CoupledLens,
}

impl<'a> CoupledLensField<'a> {
    pub fn new(inner: Box<dyn HeightField + 'a>, maps: Arc<CoupledMaps>, lens: CoupledLens) -> CoupledLensField<'a> {
        CoupledLensField { inner, maps, lens }
    }

    fn f(&self) -> &dyn HeightField {
        self.inner.as_ref()
    }
}

impl HeightField for CoupledLensField<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.f().height(dir)
    }
    fn height_at_level(&self, dir: Vec3, level: u8) -> f64 {
        self.f().height_at_level(dir, level)
    }
    fn level_aware(&self) -> bool {
        self.f().level_aware()
    }
    fn level_drivers(&self, dirs: &[Vec3], level: u8) -> Option<Vec<Option<[u8; 3]>>> {
        self.f().level_drivers(dirs, level)
    }
    fn level_change(&self, dirs: &[Vec3], level: u8) -> Option<Vec<f64>> {
        self.f().level_change(dirs, level)
    }
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        self.f().heights_until(dirs, stop)
    }
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        self.f().heights_threaded_until(dirs, stop, threads)
    }
    fn seed(&self) -> u64 {
        self.f().seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.f().implementation_id()
    }
    fn version(&self) -> u32 {
        self.f().version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.f().max_abs_height_m()
    }
    fn inputs_hash(&self) -> u64 {
        self.f().inputs_hash()
    }
    fn read_records(&self) -> Vec<planet_world_def::ReadRecord> {
        self.f().read_records()
    }
    fn read_records_for(&self, tile: TileId, map: &dyn FaceMapping) -> Vec<planet_world_def::ReadRecord> {
        self.f().read_records_for(tile, map)
    }
    fn reads_cells(&self) -> bool {
        self.f().reads_cells()
    }
    fn inputs_hash_for(&self, tile: TileId, map: &dyn FaceMapping) -> u64 {
        self.f().inputs_hash_for(tile, map)
    }
    fn body(&self) -> BodyId {
        self.f().body()
    }
    fn source_key(&self) -> Option<[u8; 16]> {
        self.f().source_key()
    }
    fn upstream(&self) -> Option<&dyn HeightField> {
        self.f().upstream()
    }
    fn stage_params(&self) -> Option<(&str, u32)> {
        self.f().stage_params()
    }
    fn item_revision(&self) -> u64 {
        self.f().item_revision()
    }
    // The lens changes the climate of every tile, so it is never the identity on its source: the keying walk must stop here (GEN-043).
    fn passes_through(&self, _tile: TileId, _map: &dyn FaceMapping) -> bool {
        false
    }
    fn surface_id(&self) -> &'static str {
        self.f().surface_id()
    }
    fn climate(&self, map: &dyn FaceMapping, id: TileId) -> ClimateTexture {
        lens_climate(&self.maps, self.lens, map, id)
    }
    fn heights_and_climate(&self, map: &dyn FaceMapping, id: TileId, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> (Vec<f64>, ClimateTexture) {
        // The inner field's own batch for this tile, so a chunk field below reads the height at the tile's level; only the climate is the lens's.
        let (heights, _) = self.f().heights_and_climate(map, id, dirs, stop);
        (heights, lens_climate(&self.maps, self.lens, map, id))
    }
    fn climate_version_word(&self) -> u64 {
        mix64(self.f().climate_version_word() ^ lens_version_word(&self.maps, self.lens))
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        self.f().height_bounds(centre, angular_radius)
    }
    fn bounds_monotone(&self) -> bool {
        self.f().bounds_monotone()
    }
    fn water_cull_level(&self) -> Option<f64> {
        self.f().water_cull_level()
    }
}
