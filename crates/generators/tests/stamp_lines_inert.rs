//! Tier A: until the line stage (GEN-028) a ridge or a channel adds no height and no tile bound (WDEF-011): the stamp stage reads the line
//! kinds and leaves the base as it is.

use planet_core::Vec3;
use planet_generators::stamp::apply_stamps;
use planet_world_def::footprint::Cap;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;

// spec: WDEF-011
#[test]
fn a_line_item_adds_no_height_to_the_base_before_the_line_stage() {
    let at = Vec3::new(0.8, 0.45, 0.35).normalized();
    let next = (at + Vec3::new(0.0, 0.0, 1e-4)).normalized();
    let mut def = WorldDefinition::single_planet(R, 1);
    for (id, kind) in [(1u128, StampKind::Ridge), (2, StampKind::Channel)] {
        let spec = ItemSpec {
            id,
            layer: "items".into(),
            priority: 0,
            dir: at,
            content_hash: 0,
            reach_m: 5_000.0,
            max_delta_m: 100.0,
            stamp: None,
        }
        .with_stamp(Stamp::new(kind, [40.0, 200.0, 0.0]).with_points(&[at, next]));
        def.put_item_spec(BodyId::PLANET, spec).unwrap();
    }
    let items = def.footprint_items_cap(BodyId::PLANET, "items", Cap { centre: at, angle: 0.01 }).unwrap();
    assert_eq!(items.len(), 2);
    for d in [at, next, (at + next).normalized()] {
        assert_eq!(apply_stamps(&items, R, d, 123.5), 123.5, "the base height is unchanged on the line");
    }
}
