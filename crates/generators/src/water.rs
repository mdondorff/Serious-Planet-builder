//! The water-level field W(dir) (ADR 0041 point 1, GEN-023): where there is water, the height of its surface above the reference
//! surface, the id of the water body and the kind of liquid.
//!
//! W is a **field** (CON-07, ADR 0035): a CPU function of the direction and the data below it, bit-exact for every thread count
//! and operating system (CON-14); it has no GPU twin, and the water pass of M5 (ADR 0041 point 4) reads it through the plan.
//!
//! * **Producers.** W is made by [`WaterProducer`]s listed in a [`WaterField`], most specific first; the first that reports water
//!   at a point wins, so a lake never has to know about the sea and the sea never about lakes. The only producer of M5 is
//!   [`SeaProducer`]; hydrology adds lakes and rivers as further producers (ADR 0041 consequences), and the retrofit test
//!   (GEN-023) puts a synthetic lake through the same path.
//! * **The sea** is the ocean-connected part of the low terrain: a point is sea when the interpolated continentalness of the bake
//!   is at or below the sea level (ADR 0027 point 5) **and** at least one of the lattice vertices that carry weight in the
//!   interpolation (the stored vertex itself at weight 0) is ocean-connected (`crate::ocean`). The channel is categorical and never
//!   interpolated. An inland basin below sea level has no ocean-connected vertex and stays dry. The set of vertices is the same
//!   from either face of a shared face edge (the copies of an edge vertex are one node with one ocean bit), so the rule is
//!   edge-symmetric.
//! * **Where W is absent** there is no water: [`WaterField::water_at`] answers `None`, and a consumer never reads a level from
//!   the absence.
//!
//! Attributes and their class (ADR 0035): the level (m, `f64`), the body id and the liquid kind id are **fields** with an exact
//! bound (CPU only; the level of the sea is the definition's `body.<n>.sea.level_m` bit for bit).

use planet_core::cube::FaceMapping;
use planet_core::Vec3;
use planet_world_def::{LiquidKindId, SeaParams};

use crate::global::{locate_direction, read_direction, GlobalSample, LatticePos, LatticeSource, ReadError, SEA_LEVEL_C};
use crate::ocean::OceanLattice;
use crate::water_summary::WaterSummary;
use crate::wet_floor::{bound_code, FloorLattice};

/// Bump whenever any answer of this module changes (ADR 0006).
pub const WATER_VERSION: u32 = 2;
/// Names this implementation in cache keys (CON-07).
pub const WATER_IMPLEMENTATION_ID: &str = "cpu-water-level-v2";

/// Identifies a water body. The sea of a body is [`WaterBodyId::SEA`]; producers hand out further ids (a lake of hydrology will
/// take `LABEL_BIT | label`, the label being the component's smallest node id, ADR 0041 point 2: ids with that bit set are reserved
/// for it and no producer of M5 uses them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WaterBodyId(pub u64);

impl WaterBodyId {
    /// The sea: every ocean component of a body shares its level and its liquid, so they share the id.
    pub const SEA: WaterBodyId = WaterBodyId(1);
    /// Reserved: ids derived from the labels of the bake's components.
    pub const LABEL_BIT: u64 = 1 << 63;
}

/// W at one point: the surface height above the reference surface (metres), the body and the liquid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterSample {
    pub level_m: f64,
    pub body: WaterBodyId,
    pub kind: LiquidKindId,
}

/// What a producer is asked: the direction, where it falls in the bake's lattice and the bake interpolated there (one reading
/// for all producers).
#[derive(Clone, Copy, Debug)]
pub struct WaterQuery {
    pub dir: Vec3,
    pub pos: LatticePos,
    pub ground: GlobalSample,
}

/// A source of water. A pure function of the query and its own data (CON-06); it reads the bake only through the query.
pub trait WaterProducer: Sync {
    /// The water at the query, if this producer makes any there.
    fn water_at(&self, q: &WaterQuery) -> Result<Option<WaterSample>, ReadError>;

    /// The level the plan shades against when no water is at the camera: the level of this producer's standing water, if it has
    /// one level for the whole body (the sea).
    fn reference_level_m(&self) -> Option<f64> {
        None
    }

    /// The water this producer makes at the query by the coastal band (ADR 0056), if any: where the bake says land but the ground is
    /// low enough for erosion to cut it below the level. The default has no band.
    fn band_at(&self, _q: &WaterQuery) -> Result<Option<WaterSample>, ReadError> {
        Ok(None)
    }

    /// Whether a vertex that weighs in the interpolation at the query is wet (ADR 0071): an ocean vertex, or one the sea reaches through
    /// ground at most the conduction bound high.
    fn wet_at(&self, _q: &WaterQuery) -> Result<bool, ReadError> {
        Ok(false)
    }

    /// The water of a tile that spans more than one block of the bake, from this producer's summary (ADR 0058): `None` when the
    /// producer has no summary for the tile (the field asks it point by point), else the answer for the whole tile.
    fn coarse_tile_water(
        &self,
        _map: &dyn FaceMapping,
        _tile: planet_core::TileId,
        _radius_m: f64,
    ) -> Result<Option<Option<WaterSample>>, ReadError> {
        Ok(None)
    }
}

/// The sea (ADR 0041 point 1): W is the definition's sea level where the point is ocean-connected sea.
pub struct SeaProducer<'a> {
    ocean: &'a dyn OceanLattice,
    sea: SeaParams,
    summary: Option<&'a WaterSummary>,
    floor: Option<&'a dyn FloorLattice>,
    /// The code of the conduction bound `C(W)` (ADR 0071 point 2): a vertex is wet when its floor is at most this.
    bound_code: u16,
}

impl<'a> SeaProducer<'a> {
    pub fn new(ocean: &'a dyn OceanLattice, sea: SeaParams) -> SeaProducer<'a> {
        SeaProducer { ocean, sea, summary: None, floor: None, bound_code: bound_code(coast_band_elevation_m(sea.level_m)) }
    }

    /// The same with the wet floor (ADR 0071): a vertex is wet when it is ocean or its floor is at most the code of the conduction bound.
    /// Without a floor only the ocean vertices are wet.
    pub fn with_floor(mut self, floor: &'a dyn FloorLattice) -> SeaProducer<'a> {
        self.floor = Some(floor);
        self
    }

    /// Whether lattice vertex `(i, j)` of `face` is wet (ADR 0071 point 2). The one comparison of the point query; the summary makes the
    /// same one on the lowest floor of a cell.
    fn vertex_wet(&self, face: planet_core::Face, i: u32, j: u32) -> Result<bool, ReadError> {
        if self.ocean.is_ocean(face, i, j)? {
            return Ok(true);
        }
        match self.floor {
            Some(f) => Ok(f.floor(face, i, j)? <= self.bound_code),
            None => Ok(false),
        }
    }

    fn check_floor_shape(&self, res: u32) -> Result<(), ReadError> {
        match self.floor {
            Some(f) if f.res() != res => {
                Err(ReadError::Shape(format!("the wet floor is of a {}-cell bake and the water field reads a {res}-cell bake", f.res())))
            }
            _ => Ok(()),
        }
    }

    /// The same with the coarse level of W (ADR 0058): tiles at or above the bake's block level are answered from `summary`.
    pub fn with_summary(mut self, summary: &'a WaterSummary) -> SeaProducer<'a> {
        self.summary = Some(summary);
        self
    }

    pub fn sea(&self) -> SeaParams {
        self.sea
    }
}

impl WaterProducer for SeaProducer<'_> {
    fn water_at(&self, q: &WaterQuery) -> Result<Option<WaterSample>, ReadError> {
        if self.ocean.res() != q.pos.res {
            return Err(ReadError::Shape(format!(
                "the ocean channel is of a {}-cell bake and the water field reads a {}-cell bake",
                self.ocean.res(),
                q.pos.res
            )));
        }
        // Land is the stored semantic of ADR 0026 point 3, on the interpolated continentalness (ADR 0027 point 5).
        if f64::from(q.ground.continent) > SEA_LEVEL_C {
            return Ok(None);
        }
        self.check_floor_shape(q.pos.res)?;
        let (vertices, n) = q.pos.weighted_vertices(q.pos.res);
        for &(i, j) in &vertices[..n] {
            if self.vertex_wet(q.pos.face, i, j)? {
                return Ok(Some(WaterSample { level_m: self.sea.level_m, body: WaterBodyId::SEA, kind: self.sea.liquid }));
            }
        }
        Ok(None)
    }

    fn reference_level_m(&self) -> Option<f64> {
        Some(self.sea.level_m)
    }

    fn band_at(&self, q: &WaterQuery) -> Result<Option<WaterSample>, ReadError> {
        let land = f64::from(q.ground.continent) > SEA_LEVEL_C;
        let low = f64::from(q.ground.elevation_m) <= coast_band_elevation_m(self.sea.level_m);
        Ok((land && low).then_some(WaterSample { level_m: self.sea.level_m, body: WaterBodyId::SEA, kind: self.sea.liquid }))
    }

    fn coarse_tile_water(
        &self,
        map: &dyn FaceMapping,
        tile: planet_core::TileId,
        radius_m: f64,
    ) -> Result<Option<Option<WaterSample>>, ReadError> {
        let Some(summary) = self.summary else { return Ok(None) };
        if summary.res() != self.ocean.res() {
            return Err(ReadError::Shape(format!(
                "the water summary is of a {}-cell bake and the ocean channel of a {}-cell bake",
                summary.res(),
                self.ocean.res()
            )));
        }
        Ok(summary.tile_water(map, tile, radius_m, &self.sea))
    }

    fn wet_at(&self, q: &WaterQuery) -> Result<bool, ReadError> {
        if self.ocean.res() != q.pos.res {
            return Err(ReadError::Shape(format!(
                "the ocean channel is of a {}-cell bake and the water field reads a {}-cell bake",
                self.ocean.res(),
                q.pos.res
            )));
        }
        self.check_floor_shape(q.pos.res)?;
        let (vertices, n) = q.pos.weighted_vertices(q.pos.res);
        for &(i, j) in &vertices[..n] {
            if self.vertex_wet(q.pos.face, i, j)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// The highest bake elevation (m) at which a point of the coastal band can still lie below the sea level `level_m` (ADR 0056): the
/// fixed point of `E = level + the deepest flat-ground cut at E` ([`crate::erosion::flat_ground_band_elevation_m`], about 64 m at level 0).
/// Monotone in the level. Steep ground (a larger slope factor) is not covered.
pub fn coast_band_elevation_m(level_m: f64) -> f64 {
    crate::erosion::flat_ground_band_elevation_m(level_m)
}

/// W over a body: the producers, the bake they read and its face mapping.
pub struct WaterField<'a> {
    map: &'a (dyn FaceMapping + Sync),
    bake: &'a dyn LatticeSource,
    producers: Vec<&'a dyn WaterProducer>,
}

/// The water a plan is built against at one place (GPU-free plan data): the level the shading uses and, when the place has water,
/// its body and liquid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanWater {
    pub level_m: f64,
    pub water: Option<(WaterBodyId, LiquidKindId)>,
}

impl<'a> WaterField<'a> {
    /// `producers` are consulted in order: list the most specific first and the sea last.
    pub fn new(map: &'a (dyn FaceMapping + Sync), bake: &'a dyn LatticeSource, producers: Vec<&'a dyn WaterProducer>) -> WaterField<'a> {
        WaterField { map, bake, producers }
    }

    /// W at the unit direction `dir`.
    pub fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        let pos = locate_direction(self.bake.res(), self.map, dir)?;
        let ground = read_direction(self.bake, self.map, dir)?;
        let q = WaterQuery { dir, pos, ground };
        for p in &self.producers {
            if let Some(w) = p.water_at(&q)? {
                return Ok(Some(w));
            }
        }
        Ok(None)
    }

    /// The sea of the coastal band at `dir` (ADR 0056): the bake says land, the ground is low enough for the terrain to dip below the
    /// level, and no producer has water there. `None` elsewhere.
    pub fn band_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        let pos = locate_direction(self.bake.res(), self.map, dir)?;
        let ground = read_direction(self.bake, self.map, dir)?;
        let q = WaterQuery { dir, pos, ground };
        for p in &self.producers {
            if let Some(w) = p.band_at(&q)? {
                return Ok(Some(w));
            }
        }
        Ok(None)
    }

    /// The water of `tile` from the producers' summaries (ADR 0058), when every producer has one for it: the first producer with water
    /// wins, as in [`water_at`](WaterField::water_at); `None` when some producer must be asked point by point.
    pub fn coarse_tile_water(
        &self,
        map: &dyn FaceMapping,
        tile: planet_core::TileId,
        radius_m: f64,
    ) -> Result<Option<Option<WaterSample>>, ReadError> {
        let mut found = None;
        for p in &self.producers {
            match p.coarse_tile_water(map, tile, radius_m)? {
                None => return Ok(None),
                Some(Some(w)) => found = found.or(Some(w)),
                Some(None) => {}
            }
        }
        Ok(Some(found))
    }

    /// Whether a vertex that weighs in the interpolation at `dir` is wet (ADR 0071).
    pub fn wet_at(&self, dir: Vec3) -> Result<bool, ReadError> {
        let pos = locate_direction(self.bake.res(), self.map, dir)?;
        // Only the lattice position decides: the bake is not read.
        let ground = GlobalSample { continent: 0.0, elevation_m: 0.0, temperature_c: 0.0, precipitation_mm: 0.0 };
        let q = WaterQuery { dir, pos, ground };
        for p in &self.producers {
            if p.wet_at(&q)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The level of the water at `dir`, if there is water.
    pub fn level_at(&self, dir: Vec3) -> Result<Option<f64>, ReadError> {
        Ok(self.water_at(dir)?.map(|w| w.level_m))
    }

    /// The level the plan shades against for a view above `dir`: the water there, else the first producer's reference level (the
    /// sea's), else 0 (the reference surface). With the sea as the only producer it is the sea level everywhere.
    pub fn plan_water(&self, dir: Vec3) -> Result<PlanWater, ReadError> {
        if let Some(w) = self.water_at(dir)? {
            return Ok(PlanWater { level_m: w.level_m, water: Some((w.body, w.kind)) });
        }
        let level_m = self.producers.iter().find_map(|p| p.reference_level_m()).unwrap_or(0.0);
        Ok(PlanWater { level_m, water: None })
    }
}

/// A height field that names a water level for the frustum cut of the node selection: the volume a tile is culled by then reaches up to
/// the level, so the water of a tile whose terrain the view does not see (a camera low over deep water, a steep view down) is not
/// culled with the seabed (ADR 0053). Every height, bound, key and reading is the inner field's, so meshes, mesh keys and caches are
/// unchanged; only [`HeightField::water_cull_level`] differs.
pub struct WaterBounded<'a> {
    inner: Inner<'a>,
    level_m: f64,
}

enum Inner<'a> {
    Owned(Box<dyn crate::height::HeightField + 'a>),
    Borrowed(&'a dyn crate::height::HeightField),
}

impl<'a> WaterBounded<'a> {
    pub fn new(inner: Box<dyn crate::height::HeightField + 'a>, level_m: f64) -> WaterBounded<'a> {
        WaterBounded { inner: Inner::Owned(inner), level_m }
    }

    pub fn borrowed(inner: &'a dyn crate::height::HeightField, level_m: f64) -> WaterBounded<'a> {
        WaterBounded { inner: Inner::Borrowed(inner), level_m }
    }

    fn f(&self) -> &dyn crate::height::HeightField {
        match &self.inner {
            Inner::Owned(b) => b.as_ref(),
            Inner::Borrowed(r) => *r,
        }
    }
}

impl crate::height::HeightField for WaterBounded<'_> {
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
    fn read_records_for(&self, tile: planet_core::TileId, map: &dyn FaceMapping) -> Vec<planet_world_def::ReadRecord> {
        self.f().read_records_for(tile, map)
    }
    fn reads_cells(&self) -> bool {
        self.f().reads_cells()
    }
    fn inputs_hash_for(&self, tile: planet_core::TileId, map: &dyn FaceMapping) -> u64 {
        self.f().inputs_hash_for(tile, map)
    }
    fn body(&self) -> planet_world_def::BodyId {
        self.f().body()
    }
    fn source_key(&self) -> Option<[u8; 16]> {
        self.f().source_key()
    }
    fn upstream(&self) -> Option<&dyn crate::height::HeightField> {
        self.f().upstream()
    }
    fn stage_params(&self) -> Option<(&str, u32)> {
        self.f().stage_params()
    }
    fn item_revision(&self) -> u64 {
        self.f().item_revision()
    }
    fn passes_through(&self, tile: planet_core::TileId, map: &dyn FaceMapping) -> bool {
        self.f().passes_through(tile, map)
    }
    fn surface_id(&self) -> &'static str {
        self.f().surface_id()
    }
    fn climate(&self, map: &dyn FaceMapping, id: planet_core::TileId) -> crate::climate::ClimateTexture {
        self.f().climate(map, id)
    }
    fn heights_and_climate(
        &self,
        map: &dyn FaceMapping,
        id: planet_core::TileId,
        dirs: &[Vec3],
        stop: &dyn Fn() -> bool,
    ) -> (Vec<f64>, crate::climate::ClimateTexture) {
        self.f().heights_and_climate(map, id, dirs, stop)
    }
    fn climate_version_word(&self) -> u64 {
        self.f().climate_version_word()
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        self.f().height_bounds(centre, angular_radius)
    }
    fn bounds_monotone(&self) -> bool {
        self.f().bounds_monotone()
    }
    fn water_cull_level(&self) -> Option<f64> {
        Some(self.level_m)
    }
}
