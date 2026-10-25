//! GEN-036: `height_at_level` is `height` for every field without level-dependent detail, and every wrapper answers it as the field it
//! wraps does (a stage adds its own change on top); a wrapper that forgets to forward the default body would silently drop the detail
//! (learning 2026-10-16).

use std::sync::Arc;

use planet_core::Vec3;
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::{StampStage, STAMP_STAGE_VERSION};
use planet_generators::water::WaterBounded;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

/// A field whose height depends on the level, as a chunk field's does.
struct Leveled;

impl HeightField for Leveled {
    fn height(&self, dir: Vec3) -> f64 {
        100.0 * dir.x
    }
    fn height_at_level(&self, dir: Vec3, level: u8) -> f64 {
        100.0 * dir.x + 1000.0 * f64::from(level)
    }
    fn seed(&self) -> u64 {
        1
    }
    fn implementation_id(&self) -> &'static str {
        "leveled-test"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        10_000.0
    }
}

fn probes() -> Vec<Vec3> {
    [(1.0, 0.2, 0.1), (0.3, -0.9, 0.2), (-0.5, 0.4, 0.8), (0.0, 0.0, 1.0)]
        .iter()
        .map(|&(x, y, z)| Vec3::new(x, y, z).normalized())
        .collect()
}

// spec: GEN-036
#[test]
fn a_field_without_level_detail_answers_every_level_with_its_height() {
    for d in probes() {
        for level in 0..=22u8 {
            assert_eq!(Reference(7).height_at_level(d, level).to_bits(), Reference(7).height(d).to_bits(), "level {level} at {d:?}");
        }
    }
}

// spec: GEN-036
#[test]
fn the_water_bound_answers_the_level_as_its_inner_field_does() {
    let bounded = WaterBounded::new(Box::new(Leveled), -50.0);
    for d in probes() {
        for level in [0u8, 11, 15, 22] {
            assert_eq!(bounded.height_at_level(d, level).to_bits(), Leveled.height_at_level(d, level).to_bits(), "level {level}");
        }
    }
}

// spec: GEN-036
#[test]
fn a_stamp_stage_adds_its_change_to_the_level_of_the_field_below() {
    let at = Vec3::new(1.0, 0.2, 0.1).normalized();
    let mut def = WorldDefinition::single_planet(R, 7);
    let item = ItemSpec {
        id: 1,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: at,
        content_hash: 0,
        reach_m: 3_000.0,
        max_delta_m: 60.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0]));
    def.put_item_spec(BodyId::PLANET, item).unwrap();
    let stage = StampStage::new(Arc::new(Leveled), Arc::new(def), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, STAMP_STAGE_VERSION).unwrap();
    let change = stage.height(at) - Leveled.height(at);
    assert!(change.abs() > 1.0, "the stamp acts at its centre ({change})");
    for level in [0u8, 11, 15, 22] {
        let want = Leveled.height_at_level(at, level) + change;
        assert_eq!(stage.height_at_level(at, level).to_bits(), want.to_bits(), "level {level}");
    }
}
