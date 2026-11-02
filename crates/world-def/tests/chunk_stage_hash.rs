//! TEST-075: the definition hash of a world is what it was before the chunk stage was removed (ADR 0086, decision 3).

use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ChunkStagePosition, ItemSpec, WorldDefinition, CHAIN_ORDER_ERA};

fn with_hill(mut d: WorldDefinition) -> WorldDefinition {
    let at = planet_core::Vec3::new(1.0, 0.2, 0.1).normalized();
    let item =
        ItemSpec { id: 1, layer: "stamps".into(), priority: 0, dir: at, content_hash: 0, reach_m: 1000.0, max_delta_m: 10.0, stamp: None }
            .with_stamp(Stamp::new(StampKind::Hill, [10.0, 0.0, 0.0]));
    d.put_item_spec(BodyId::PLANET, item).unwrap();
    d
}

// spec: TEST-075
#[test]
fn a_definition_hashes_to_the_value_it_had_before_the_chunk_stage_was_removed() {
    // Values computed on main before PR 5 of the removal; a store built from these definitions opens
    // without a rebuild only while they stay.
    assert_eq!(CHAIN_ORDER_ERA, 2);
    let plain = WorldDefinition::single_planet(6_371_000.0, 7);
    assert_eq!(plain.hash(), PLAIN, "a definition without items");
    let under = with_hill(plain.clone());
    assert_eq!(under.hash(), UNDER, "a definition with an item and the default (Under) chunk stage");
    let mut on_top = with_hill(plain);
    on_top.set_chunk_stage(BodyId::PLANET, "stamps", ChunkStagePosition::OnTop);
    assert_eq!(on_top.hash(), ON_TOP, "the same with the layer set OnTop: the setting stays parseable and hashed");
}

const PLAIN: u64 = 7_100_824_357_386_557_140;
const UNDER: u64 = 13_555_216_015_731_634_793;
const ON_TOP: u64 = 7_079_277_651_562_047_971;
