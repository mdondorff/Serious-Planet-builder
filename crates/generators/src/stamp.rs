//! The stamp stage (GEN-026): the authored items of one layer added to a base height field.
//!
//! A stamp is a delta of the height around its anchor (a hill, a crater, a plateau); the stage adds the stamps whose reach contains a
//! point to the base height there, in application order. It reads items once per batch through an [`ItemReader`] (so a contract test can
//! record what it read), reads the base only as a height field at points, and chains its tile keys to the base's (STRM-027).

use std::sync::{Arc, RwLock};

use planet_core::{FaceMapping, TileId, Vec3};
use planet_world_def::footprint::{angle_between, Cap, FootprintItem};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, GenContext, ReadRecord, WorldDefinition};

use crate::height::HeightField;

/// The halo, as a fraction of the tile edge, with which the stage declares its items for a tile (GEN-026): the sample stencil and the
/// skirt need a small part of it; a larger halo only adds false positives. It is part of the key through the kind 3 and 4 entries, and a
/// change bumps the stage version.
pub const STAGE_HALO: f64 = 0.25;

/// `s(t) = t^2 (3 - 2t)`.
fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// The delta of a hill of height `amount` (signed) at `x = distance / reach`.
pub fn hill_delta(amount: f64, x: f64) -> f64 {
    if x >= 1.0 {
        return 0.0;
    }
    let q = 1.0 - x * x;
    amount * q * q
}

/// The delta of a crater of depth `depth` (at least 0) at `x = distance / reach`: a bowl with a low rim.
pub fn crater_delta(depth: f64, x: f64) -> f64 {
    if x >= 1.0 {
        return 0.0;
    }
    let bowl = 1.0 - smooth((x / 0.8).min(1.0));
    let rim = if (0.7..=1.0).contains(&x) {
        let u = (x - 0.85) / 0.15;
        let v = 1.0 - u * u;
        v * v
    } else {
        0.0
    };
    depth * (-bowl + 0.2 * rim)
}

/// The weight of a plateau at `x = distance / reach`: 1 up to 0.7, falling to 0 at 1.
pub fn plateau_weight(x: f64) -> f64 {
    1.0 - smooth(((x - 0.7) / 0.3).clamp(0.0, 1.0))
}

/// The delta of a plateau toward the absolute height `target` at `x`, for the running height `h` and the bound `m`.
pub fn plateau_delta(target: f64, h: f64, m: f64, x: f64) -> f64 {
    if x >= 1.0 {
        return 0.0;
    }
    plateau_weight(x) * (target - h).clamp(-m, m)
}

/// The delta of `stamp` at `x` for the running height `h` and the item's bound `m`.
pub fn stamp_delta(stamp: &Stamp, m: f64, h: f64, x: f64) -> f64 {
    let p = stamp.params();
    match stamp.kind {
        StampKind::Hill => hill_delta(p[0], x),
        StampKind::Crater => crater_delta(p[0], x),
        StampKind::Plateau => plateau_delta(p[0], h, m, x),
    }
}

/// The range `(lo, hi)` of the delta `item` can add at any point of `cap` (GEN-026), widened outward by a few ulps. The profiles are
/// monotone in `x` (the hill, the bowl, the plateau weight), so the range comes from the nearest and the farthest distance of the cap
/// from the anchor; a crater whose cap reaches its rim takes the rim's maximum, and a plateau is bounded by its weight at the nearest
/// distance times the item's bound. An item that is no stamp adds nothing.
fn delta_range(item: &FootprintItem, cap: Cap, radius_m: f64) -> (f64, f64) {
    let Some(stamp) = item.stamp else { return (0.0, 0.0) };
    let m = f64::from_bits(item.max_delta_bits);
    let reach = f64::from_bits(item.reach_bits);
    let d = angle_between(anchor_of(item), cap.centre);
    let to_x = |angle: f64| angle * radius_m / reach;
    // The angles widen outward by a few ulps of a radian (an angle from `atan2` is wrong by about that much however short the reach),
    // then the distances by a relative 1e-9: the profile argument is rounded.
    let slack = 16.0 * f64::EPSILON;
    let x_min = (to_x((d - cap.angle - slack).max(0.0)) * (1.0 - 1e-9)).max(0.0);
    let x_max = (to_x(d + cap.angle + slack) * (1.0 + 1e-9)).min(1.0);
    let a = stamp.params()[0];
    let (lo, hi) = match stamp.kind {
        StampKind::Hill => {
            let (u, v) = (hill_delta(a, x_min), hill_delta(a, x_max));
            (u.min(v), u.max(v))
        }
        StampKind::Crater => {
            let (u, v) = (crater_delta(a, x_min), crater_delta(a, x_max));
            if x_max < 0.7 {
                (u.min(v), u.max(v))
            } else {
                // The rim adds at most 0.2 of the depth; the bowl is at its deepest at the nearest distance.
                let bowl = -a * (1.0 - smooth((x_min / 0.8).min(1.0)));
                let rim = 0.2 * a;
                (bowl.min(rim), bowl.max(rim))
            }
        }
        StampKind::Plateau => {
            let w = plateau_weight(x_min) * m;
            (-w, w)
        }
    };
    let grow = |x: f64| x.abs() * 8.0 * f64::EPSILON;
    ((lo - grow(lo)).max(-m - grow(m)), (hi + grow(hi)).min(m + grow(m)))
}

/// The unit direction of an item's anchor.
fn anchor_of(item: &FootprintItem) -> Vec3 {
    Vec3::new(f64::from_bits(item.dir[0]), f64::from_bits(item.dir[1]), f64::from_bits(item.dir[2]))
}

/// The running height at `dir` after the stamps of `items` (application order, the caller's) were added to `base`, on a body of `radius_m`.
pub fn apply_stamps(items: &[FootprintItem], radius_m: f64, dir: Vec3, base: f64) -> f64 {
    let mut h = base;
    for item in items {
        let Some(stamp) = item.stamp else { continue };
        let reach = f64::from_bits(item.reach_bits);
        let x = radius_m * angle_between(anchor_of(item), dir) / reach;
        if x < 1.0 {
            h += stamp_delta(&stamp, f64::from_bits(item.max_delta_bits), h, x);
        }
    }
    h
}

/// Where a stage reads its items: the footprint of a cap, the sum of the bounds of a layer, and the reads a tile declares. The world
/// definition implements it; the contract test (ADR 0021 point 7) swaps in a reader that records what the stage asked for.
pub trait ItemReader: Send + Sync {
    /// The radius of the body, if it has a usable one.
    fn radius_m(&self, body: BodyId) -> Option<f64>;
    /// The items of `layer` whose reach touches `cap`, in application order (WDEF-007).
    fn items(&self, body: BodyId, layer: &str, cap: Cap) -> Vec<FootprintItem>;
    /// The sum of `max_delta_m` over every item of the layer (a bound for field-wide uses).
    fn max_delta_sum(&self, body: BodyId, layer: &str) -> f64;
    /// The reads a tile declares for its items, with the stage's halo (WDEF-007, WDEF-008): the footprint or summary entries.
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<ReadRecord>;
    /// The revision of the items (never 0): differs after every change and between definitions (STRM-027).
    fn revision(&self) -> u64;
    /// Whether no item of `layer` touches `tile` with `halo` (WDEF-007): the footprint the tile declares is empty.
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool;
}

impl ItemReader for WorldDefinition {
    fn radius_m(&self, body: BodyId) -> Option<f64> {
        match self.get(&planet_world_def::body_radius_path(body)) {
            Some(planet_world_def::NodeValue::F64(r)) if r.is_finite() && r > 0.0 => Some(r),
            _ => None,
        }
    }
    fn items(&self, body: BodyId, layer: &str, cap: Cap) -> Vec<FootprintItem> {
        self.footprint_items_cap(body, layer, cap).expect("a stage reads the items of a body that has a radius")
    }
    fn max_delta_sum(&self, body: BodyId, layer: &str) -> f64 {
        self.item_ids(body)
            .into_iter()
            .filter(|id| self.item_layer(body, *id).as_deref() == Some(layer))
            .filter_map(|id| self.item_max_delta(body, id))
            .sum()
    }
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<ReadRecord> {
        let ctx = GenContext::new(self);
        ctx.query_footprint(body, layer, tile, halo).expect("a stage declares the items of a body that has a radius");
        ctx.read_records()
    }
    fn revision(&self) -> u64 {
        WorldDefinition::revision(self)
    }
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool {
        self.footprint_items(body, layer, tile, halo).is_ok_and(|v| v.is_empty())
    }
}

/// A definition shared between the editor and the stage: the stage reads whatever it holds at the moment of a batch.
impl ItemReader for RwLock<WorldDefinition> {
    fn radius_m(&self, body: BodyId) -> Option<f64> {
        self.read().expect("definition lock").radius_m(body)
    }
    fn items(&self, body: BodyId, layer: &str, cap: Cap) -> Vec<FootprintItem> {
        self.read().expect("definition lock").items(body, layer, cap)
    }
    fn max_delta_sum(&self, body: BodyId, layer: &str) -> f64 {
        self.read().expect("definition lock").max_delta_sum(body, layer)
    }
    fn tile_reads(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> Vec<ReadRecord> {
        self.read().expect("definition lock").tile_reads(body, layer, tile, halo)
    }
    fn revision(&self) -> u64 {
        self.read().expect("definition lock").revision()
    }
    fn footprint_empty(&self, body: BodyId, layer: &str, tile: TileId, halo: f64) -> bool {
        self.read().expect("definition lock").footprint_empty(body, layer, tile, halo)
    }
}

/// The stamps of one layer added to a base field (GEN-026).
pub struct StampStage<'a> {
    base: Arc<dyn HeightField + 'a>,
    items: Arc<dyn ItemReader>,
    body: BodyId,
    layer: String,
    stack_index: u32,
    stage_version: u32,
    inputs: u64,
}

/// The version of the stamp stage (bumped whenever its output changes, ADR 0006): part of the key of every stamped tile.
pub const STAMP_STAGE_VERSION: u32 = 1;

/// The implementation id of the stamp stage in height keys.
pub const STAMP_STAGE_ID: &str = "stamp-stage";

impl<'a> StampStage<'a> {
    /// A stage over `base` for `layer` of `body`, at `stack_index` in the stack of ADR 0007, version `stage_version`. `None` when the
    /// reader knows no usable radius for the body.
    pub fn new(
        base: Arc<dyn HeightField + 'a>,
        items: Arc<dyn ItemReader>,
        body: BodyId,
        layer: &str,
        stack_index: u32,
        stage_version: u32,
    ) -> Option<StampStage<'a>> {
        items.radius_m(body)?;
        let text: Vec<u64> = layer.bytes().map(u64::from).collect();
        let inputs = planet_core::hash::hash_u64s(
            0x0073_7467,
            &[base.inputs_hash(), planet_core::hash::hash_u64s(3, &text), u64::from(stack_index), u64::from(stage_version)],
        );
        Some(StampStage { base, items, body, layer: layer.to_string(), stack_index, stage_version, inputs })
    }

    /// The items a batch of directions needs: those whose reach touches the smallest cap this stage builds around them.
    fn items_for(&self, dirs: &[Vec3]) -> Vec<FootprintItem> {
        let Some(&first) = dirs.first() else { return Vec::new() };
        let sum = dirs.iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, d| a + *d);
        let centre = if sum.length() > 1e-9 { sum.normalized() } else { first };
        let angle = dirs.iter().fold(0.0f64, |a, d| a.max(angle_between(centre, *d)));
        self.items.items(self.body, &self.layer, Cap { centre, angle })
    }
}

impl HeightField for StampStage<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.heights(&[dir])[0]
    }

    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        let base = self.base.heights_until(dirs, stop);
        self.add_stamps(dirs, base, stop)
    }

    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        let base = self.base.heights_threaded_until(dirs, stop, threads);
        self.add_stamps(dirs, base, stop)
    }

    fn seed(&self) -> u64 {
        self.base.seed()
    }

    fn implementation_id(&self) -> &'static str {
        STAMP_STAGE_ID
    }

    fn version(&self) -> u32 {
        self.stage_version
    }

    fn max_abs_height_m(&self) -> f64 {
        self.base.max_abs_height_m() + self.items.max_delta_sum(self.body, &self.layer)
    }

    fn inputs_hash(&self) -> u64 {
        self.inputs
    }

    fn read_records(&self) -> Vec<ReadRecord> {
        self.base.read_records()
    }

    fn read_records_for(&self, tile: TileId, map: &dyn FaceMapping) -> Vec<ReadRecord> {
        let mut reads = self.base.read_records_for(tile, map);
        reads.extend(self.items.tile_reads(self.body, &self.layer, tile, STAGE_HALO));
        reads
    }

    fn reads_cells(&self) -> bool {
        true
    }

    /// The base's hash for the tile, its version, implementation and bake key, the stage's own inputs and the tile's item reads (CON-07):
    /// the 64-bit in-memory keys chain the stage below as the 128-bit ones do.
    fn inputs_hash_for(&self, tile: TileId, map: &dyn FaceMapping) -> u64 {
        let source = self
            .base
            .source_key()
            .map_or([0u64; 2], |k| [u64::from_le_bytes(k[..8].try_into().unwrap()), u64::from_le_bytes(k[8..].try_into().unwrap())]);
        let id: Vec<u64> = self.base.implementation_id().bytes().map(u64::from).collect();
        let reads = planet_world_def::records_hash64(&self.items.tile_reads(self.body, &self.layer, tile, STAGE_HALO));
        planet_core::hash::hash_u64s(
            0x0073_7468,
            &[
                self.base.inputs_hash_for(tile, map),
                u64::from(self.base.version()),
                planet_core::hash::hash_u64s(4, &id),
                source[0],
                source[1],
                self.inputs,
                reads,
            ],
        )
    }

    fn body(&self) -> BodyId {
        self.body
    }

    fn surface_id(&self) -> &'static str {
        self.base.surface_id()
    }

    fn climate(&self, map: &dyn FaceMapping, id: TileId) -> crate::climate::ClimateTexture {
        self.base.climate(map, id)
    }

    fn climate_version_word(&self) -> u64 {
        self.base.climate_version_word()
    }

    fn heights_and_climate(
        &self,
        map: &dyn FaceMapping,
        id: TileId,
        dirs: &[Vec3],
        stop: &dyn Fn() -> bool,
    ) -> (Vec<f64>, crate::climate::ClimateTexture) {
        let (base, climate) = self.base.heights_and_climate(map, id, dirs, stop);
        (self.add_stamps(dirs, base, stop), climate)
    }

    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (lo, hi) = self.base.height_bounds(centre, angular_radius);
        let cap = Cap { centre, angle: angular_radius };
        let radius_m = self.items.radius_m(self.body).expect("a stage reads a body that has a radius");
        let items = self.items.items(self.body, &self.layer, cap);
        // The deltas the stamps can add inside the cap, not their declared maxima: a cap on the flat top of a hill moves up as a whole and
        // keeps its extent, which is what the morph interval of a fine tile needs (GEN-026).
        let (mut dlo, mut dhi, mut mag) = (0.0f64, 0.0f64, 0.0f64);
        for item in &items {
            let (a, b) = delta_range(item, cap, radius_m);
            dlo += a;
            dhi += b;
            mag += a.abs().max(b.abs());
        }
        // Each addition of the running height rounds by at most one ulp of the largest magnitude involved.
        let slack = items.len() as f64 * (lo.abs().max(hi.abs()) + mag) * f64::EPSILON * 2.0 + (dlo.abs() + dhi.abs()) * 8.0 * f64::EPSILON;
        (lo + dlo - slack, hi + dhi + slack)
    }

    fn water_cull_level(&self) -> Option<f64> {
        self.base.water_cull_level()
    }

    fn source_key(&self) -> Option<[u8; 16]> {
        None
    }

    fn upstream(&self) -> Option<&dyn HeightField> {
        Some(self.base.as_ref())
    }

    fn stage_params(&self) -> Option<(&str, u32)> {
        Some((&self.layer, self.stack_index))
    }
    fn item_revision(&self) -> u64 {
        planet_core::hash::hash_u64s(0x0072_6576, &[self.base.item_revision(), self.items.revision()]) | 1
    }

    /// No item of the layer touches the tile (the footprint it declares is empty): the stage adds nothing there, so its heights, climate
    /// and bounds are the base's bit for bit and its keys are the base's (STRM-029, GEN-027).
    ///
    /// Only for tile ids made on the cell grid's mapping (the footprint is made there, `CELL_MAPPING_ID`): on another mapping the same id
    /// is another place, and the stage does not claim it. Never for a root tile (level 0): the normal stencil of a coarse mesh (4 cells)
    /// reaches past the declared cap there, and no root is stored or drawn (the render floor is 3).
    fn passes_through(&self, tile: TileId, map: &dyn FaceMapping) -> bool {
        tile.level() > 0
            && map.id() == planet_world_def::cells::CELL_MAPPING_ID
            && self.items.footprint_empty(self.body, &self.layer, tile, STAGE_HALO)
    }
}

impl StampStage<'_> {
    /// The base heights of `dirs` with the stamps added; elements after `stop` turned true are 0.0 (GEN-020).
    fn add_stamps(&self, dirs: &[Vec3], base: Vec<f64>, stop: &dyn Fn() -> bool) -> Vec<f64> {
        let items = self.items_for(dirs);
        let radius_m = self.items.radius_m(self.body).expect("a stage reads a body that has a radius");
        dirs.iter().zip(base).map(|(&d, b)| if stop() { 0.0 } else { apply_stamps(&items, radius_m, d, b) }).collect()
    }
}
