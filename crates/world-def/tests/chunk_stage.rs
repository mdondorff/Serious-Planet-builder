//! GEN-039: the `chunk_stage` setting of a layer: `Under` by default (M8-7), `OnTop` representable; both are accepted.

use planet_world_def::{BodyId, ChunkStagePosition, WorldDefinition};

// spec: GEN-039, WDEF-018
#[test]
fn a_layer_without_the_setting_is_under_and_the_definition_hashes_as_it_always_did() {
    let mut d = WorldDefinition::single_planet(6_371_000.0, 7);
    let before = d.hash();
    assert_eq!(d.chunk_stage(BodyId::PLANET, "stamps"), ChunkStagePosition::Under);
    assert!(d.check_chunk_stages(BodyId::PLANET).is_ok());
    // Setting the default is no setting.
    d.set_chunk_stage(BodyId::PLANET, "stamps", ChunkStagePosition::Under);
    assert_eq!(d.hash(), before);
}

// spec: GEN-039, WDEF-018
#[test]
fn on_top_is_representable_changes_the_hash_and_both_positions_are_accepted() {
    let mut d = WorldDefinition::single_planet(6_371_000.0, 7);
    let plain = d.hash();
    d.set_chunk_stage(BodyId::PLANET, "placed", ChunkStagePosition::OnTop);
    assert_eq!(d.chunk_stage(BodyId::PLANET, "placed"), ChunkStagePosition::OnTop);
    assert_ne!(d.hash(), plain, "the setting is part of the definition");
    assert!(d.check_chunk_stages(BodyId::PLANET).is_ok());
    // Back to the default: the hash is the plain one again.
    d.set_chunk_stage(BodyId::PLANET, "placed", ChunkStagePosition::Under);
    assert_eq!(d.hash(), plain);
}

// spec: GEN-039, WDEF-018
#[test]
fn the_chain_order_is_in_the_hash_of_a_definition_with_items_and_not_in_one_without() {
    use planet_world_def::stamp::{Stamp, StampKind};
    use planet_world_def::{ItemSpec, CHAIN_ORDER_ERA};
    let at = planet_core::Vec3::new(1.0, 0.2, 0.1).normalized();
    let mut d = WorldDefinition::single_planet(6_371_000.0, 7);
    // No item, no stamp to order: the hash is the nodes' hash as it always was.
    assert_eq!(d.hash(), d.base_hash());
    let item =
        ItemSpec { id: 1, layer: "stamps".into(), priority: 0, dir: at, content_hash: 0, reach_m: 1000.0, max_delta_m: 10.0, stamp: None }
            .with_stamp(Stamp::new(StampKind::Hill, [10.0, 0.0, 0.0]));
    d.put_item_spec(BodyId::PLANET, item).unwrap();
    let under = d.hash();
    const { assert!(CHAIN_ORDER_ERA >= 2, "the era of the Under default is not the era of the OnTop default (1)") };
    // The same world with its layer on top is another key: the effective chain order is hashed, not only an explicit entry.
    d.set_chunk_stage(BodyId::PLANET, "stamps", ChunkStagePosition::OnTop);
    assert_ne!(d.hash(), under);
    d.set_chunk_stage(BodyId::PLANET, "stamps", ChunkStagePosition::Under);
    assert_eq!(d.hash(), under, "back to the default: the same key");
}
