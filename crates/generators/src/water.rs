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

/// Bump whenever any answer of this module changes (ADR 0006).
pub const WATER_VERSION: u32 = 1;
/// Names this implementation in cache keys (CON-07).
pub const WATER_IMPLEMENTATION_ID: &str = "cpu-water-level-v1";

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
}

/// The sea (ADR 0041 point 1): W is the definition's sea level where the point is ocean-connected sea.
pub struct SeaProducer<'a> {
    ocean: &'a dyn OceanLattice,
    sea: SeaParams,
}

impl<'a> SeaProducer<'a> {
    pub fn new(ocean: &'a dyn OceanLattice, sea: SeaParams) -> SeaProducer<'a> {
        SeaProducer { ocean, sea }
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
        let (vertices, n) = q.pos.weighted_vertices(q.pos.res);
        for &(i, j) in &vertices[..n] {
            if self.ocean.is_ocean(q.pos.face, i, j)? {
                return Ok(Some(WaterSample { level_m: self.sea.level_m, body: WaterBodyId::SEA, kind: self.sea.liquid }));
            }
        }
        Ok(None)
    }

    fn reference_level_m(&self) -> Option<f64> {
        Some(self.sea.level_m)
    }
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
