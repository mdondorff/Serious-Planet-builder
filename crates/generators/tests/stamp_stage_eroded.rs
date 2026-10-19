//! GEN-026: the stamp stage over the eroded field (an in-memory bake): a tile generated alone equals the same samples cut from its parent's
//! grid, whatever the order of generation (the tile-level test of M3 gate condition 2, GEN-017).

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{Side, TileId, Vec3};
use planet_generators::global::{bake, GridSource};
use planet_generators::height::{generate_tile_field, HeightField};
use planet_generators::height_fields::{build_field, HeightFieldKind};
use planet_generators::stamp::StampStage;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, GenContext, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

fn eroded() -> Box<dyn HeightField> {
    let grid = bake(7, &TangentWarp, 16, 2).unwrap();
    let def = WorldDefinition::single_planet(R, 7);
    let src = Box::leak(Box::new(GridSource(Box::leak(Box::new(grid)))));
    build_field(HeightFieldKind::Eroded, &GenContext::new(&def), BodyId::PLANET, src, [1; 16]).unwrap()
}

fn at_distance(anchor: Vec3, hint: Vec3, d_m: f64) -> Vec3 {
    let t = (hint - anchor * anchor.dot(hint)).normalized();
    let a = d_m / R;
    (anchor * libm::cos(a) + t * libm::sin(a)).normalized()
}

fn item(id: u128, priority: i32, at: Vec3, reach_m: f64, max_delta_m: f64, stamp: Stamp) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority, dir: at, content_hash: 0, reach_m, max_delta_m, stamp: None }
        .with_stamp(stamp)
}

// spec: GEN-026
#[test]
fn a_stamped_tile_over_the_eroded_field_equals_the_same_samples_cut_from_its_parents_grid_whatever_the_generation_order() {
    let at = dir(1.0, 0.2, 0.1);
    let mut def = WorldDefinition::single_planet(R, 7);
    for i in [
        item(1, 0, at, 3_000.0, 60.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0])),
        item(2, 1, at_distance(at, dir(0.0, 1.0, 0.0), 700.0), 1_500.0, 60.0, Stamp::new(StampKind::Crater, [25.0, 0.0, 0.0])),
        item(3, 2, at_distance(at, dir(0.0, 0.0, 1.0), 1_000.0), 2_000.0, 200.0, Stamp::new(StampKind::Plateau, [90.0, 0.0, 0.0])),
    ] {
        def.put_item_spec(BodyId::PLANET, i).unwrap();
    }
    let base: Arc<dyn HeightField> = Arc::from(eroded());
    let s = StampStage::new(Arc::clone(&base), Arc::new(def), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1).unwrap();
    let parent = TileId::from_direction(&TangentWarp, at, 15);
    let whole = generate_tile_field(&s, &TangentWarp, parent, 32);
    for child in parent.children().unwrap() {
        let own = generate_tile_field(&s, &TangentWarp, child, 16);
        let (cx, cy) = (child.x() - 2 * parent.x(), child.y() - 2 * parent.y());
        for l in 0..=16u32 {
            for k in 0..=16u32 {
                assert_eq!(own.at(k, l).to_bits(), whole.at(16 * cx + k, 16 * cy + l).to_bits(), "child {child:?} sample ({k}, {l})");
            }
        }
    }
    // The neighbours first, then the tile: the same bits.
    for side in Side::ALL {
        let _ = generate_tile_field(&s, &TangentWarp, parent.neighbor(&TangentWarp, side), 32);
    }
    assert_eq!(generate_tile_field(&s, &TangentWarp, parent, 32), whole);
    let unstamped = generate_tile_field(base.as_ref(), &TangentWarp, parent, 32);
    assert!(whole.heights.iter().zip(&unstamped.heights).any(|(a, b)| a != b), "the stamps act in this tile");
}
