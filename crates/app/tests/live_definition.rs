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
fn the_stages_stack_in_the_order_of_the_layer_stack_each_at_its_place_in_it_and_a_lower_layer_re_keys_nothing_above() {
    let world = WorldTerrain::open(base(), RES).unwrap();
    let mut def = (*world.definition()).clone();
    def.put_item_spec(BodyId::PLANET, hill(1, "stamps", 40.0)).unwrap();
    def.put_item_spec(BodyId::PLANET, hill(2, "zeta", 5.0)).unwrap();
    def.put_item_spec(BodyId::PLANET, hill(3, "alpha", 5.0)).unwrap();
    assert_eq!(def.stamp_layers(BodyId::PLANET), ["stamps", "alpha", "zeta"], "the stack first, then other names in name order");
    world.set_definition(Arc::new(def.clone())).unwrap();
    let field = world.field().unwrap();
    let stage = |f: &dyn planet_generators::height::HeightField| f.stage_params().map(|(l, i)| (l.to_string(), i));
    let mid = field.upstream().unwrap();
    let low = mid.upstream().unwrap();
    assert_eq!(
        (stage(&*field), stage(mid), stage(low)),
        (Some(("zeta".into(), 8)), Some(("alpha".into(), 7)), Some(("stamps".into(), 3))),
        "top to bottom, each at its place in the stack; layers outside it follow the seven of the stack"
    );
    // A stamp in a lower layer shifts nobody: the stage of `stamps` keeps its index and so its keys.
    def.put_item_spec(BodyId::PLANET, hill(4, "intent", 5.0)).unwrap();
    world.set_definition(Arc::new(def)).unwrap();
    let field = world.field().unwrap();
    let above = field.upstream().unwrap().upstream().unwrap();
    assert_eq!(stage(above), Some(("stamps".into(), 3)), "the stage of stamps keeps its index and so its keys");
    assert_eq!(stage(above.upstream().unwrap()), Some(("intent".into(), 2)), "the new layer sits below it");
}

// spec: STRM-029
#[test]
fn the_first_stamp_of_a_world_re_keys_only_the_tiles_it_touches_and_the_rest_keep_the_plain_worlds_keys() {
    use planet_core::cube::TangentWarp;
    use planet_core::TileId;
    use planet_generators::height::cache_key_field;

    let key = |world: &WorldTerrain, tile: TileId| cache_key_field(&*world.field().unwrap(), &TangentWarp, tile, 32);
    let near = TileId::from_direction(&TangentWarp, at(), 12);
    let far = TileId::from_direction(&TangentWarp, -at(), 12);
    let world = WorldTerrain::open(base(), RES).unwrap();
    let (far_before, near_before) = (key(&world, far), key(&world, near));
    let mut edited = (*world.definition()).clone();
    edited.put_item_spec(BodyId::PLANET, hill(1, "stamps", 40.0)).unwrap();
    world.set_definition(Arc::new(edited)).unwrap();
    // The stage appeared with the stamp; a tile out of its reach is keyed as the plain world keys it, the tile under it is not.
    assert_eq!(key(&world, far), far_before, "a tile out of reach keeps the key it had in the plain world");
    assert_ne!(key(&world, near), near_before, "the tile under the stamp is re-keyed");
    // A second layer changes nothing out of reach either.
    let mut more = (*world.definition()).clone();
    more.put_item_spec(BodyId::PLANET, hill(2, "intent", 10.0)).unwrap();
    world.set_definition(Arc::new(more)).unwrap();
    assert_eq!(key(&world, far), far_before);
}

// spec: STRM-028
#[test]
fn a_definition_with_another_base_is_refused_and_changes_nothing() {
    let world = WorldTerrain::open(base(), RES).unwrap();
    let revision = world.definition().revision();
    let mut other = base();
    other.set_body_radius(BodyId::PLANET, R * 2.0);
    let e = world.set_definition(Arc::new(other)).unwrap_err();
    assert!(e.contains("another base") && e.matches("hash ").count() == 1 && e.contains("made from"), "{e}");
    assert_eq!(world.definition().revision(), revision);
}
