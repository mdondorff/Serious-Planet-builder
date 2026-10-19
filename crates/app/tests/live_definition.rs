//! STRM-028: the terrain reads the current definition. An editor edit swaps the definition of a shared `WorldTerrain`; the next field has
//! the stamps, a field built before keeps what it was built from (a job in flight), and a definition with another base is refused.

use std::sync::Arc;

use planet::worldfield::WorldTerrain;
use planet_core::Vec3;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;
const RES: u32 = 16;

fn base() -> WorldDefinition {
    let mut d = WorldDefinition::single_planet(R, 5);
    d.set_height_field("cpu-ref-height-v2");
    d
}

fn at() -> Vec3 {
    Vec3::new(1.0, 0.2, 0.1).normalized()
}

fn hill(id: u128, layer: &str, amount: f64) -> ItemSpec {
    ItemSpec { id, layer: layer.into(), priority: 0, dir: at(), content_hash: 0, reach_m: 3_000.0, max_delta_m: 60.0, stamp: None }
        .with_stamp(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]))
}

// spec: STRM-028
#[test]
fn a_swapped_definition_reaches_the_next_field_and_a_field_in_flight_keeps_its_own() {
    let world = Arc::new(WorldTerrain::open(base(), RES).unwrap());
    let plain = world.field().unwrap();
    let h0 = plain.height(at());
    assert_eq!(world.definition().revision(), world.definition().revision(), "a snapshot is stable");

    let mut edited = (*world.definition()).clone();
    edited.put_item_spec(BodyId::PLANET, hill(1, "stamps", 40.0)).unwrap();
    let before = world.definition().revision();
    world.set_definition(Arc::new(edited.clone())).unwrap();
    assert_ne!(world.definition().revision(), before, "the swap is visible");

    let stamped = world.field().unwrap();
    assert!(
        (stamped.height(at()) - (h0 + 40.0)).abs() < 1e-6,
        "the next field has the hill at its top: {} vs {}",
        stamped.height(at()),
        h0
    );
    assert_eq!(plain.height(at()), h0, "the field built before the swap keeps the definition it was built from");
    assert_ne!(stamped.item_revision(), 0);
    assert_eq!(plain.item_revision(), 0, "a field without stamps tracks no items");

    // A second edit and a delete come through the same way; two layers stack.
    edited.put_item_spec(BodyId::PLANET, hill(2, "intent", 10.0)).unwrap();
    world.set_definition(Arc::new(edited.clone())).unwrap();
    let two = world.field().unwrap();
    assert!(two.height(at()) > stamped.height(at()), "a second layer adds to the first");
    edited.remove_item(BodyId::PLANET, 1);
    edited.remove_item(BodyId::PLANET, 2);
    world.set_definition(Arc::new(edited)).unwrap();
    assert_eq!(world.field().unwrap().height(at()), h0, "deleting the items restores the heights");
}

// spec: STRM-028
#[test]
fn a_definition_with_another_base_is_refused_and_changes_nothing() {
    let world = WorldTerrain::open(base(), RES).unwrap();
    let revision = world.definition().revision();
    let mut other = base();
    other.set_body_radius(BodyId::PLANET, R * 2.0);
    let e = world.set_definition(Arc::new(other)).unwrap_err();
    assert!(e.contains("another base"), "{e}");
    assert_eq!(world.definition().revision(), revision);
}
