//! GEN-016, GEN-026: the field chain has one home (M8 R1) and assembles what the viewer used to assemble in `app`: the base alone for a
//! world without stamps, the stamp stage over the base for a stamped one, the water bound over either on request.

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_generators::chain::{field_chain, OwnedBake};
use planet_generators::global::bake;
use planet_generators::height::HeightField;
use planet_generators::height_fields::{build_field, HeightFieldKind};
use planet_generators::stamp::{StampStage, STAMP_STAGE_VERSION};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, GenContext, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER, LAYER_STACK};

const R: f64 = 6_371_000.0;

fn probes() -> Vec<Vec3> {
    [(1.0, 0.2, 0.1), (0.3, -0.9, 0.2), (-0.5, 0.4, 0.8), (0.0, 0.0, 1.0), (0.7, 0.7, 0.1)]
        .iter()
        .map(|&(x, y, z)| Vec3::new(x, y, z).normalized())
        .collect()
}

fn same(a: &dyn HeightField, b: &dyn HeightField) {
    assert_eq!(a.inputs_hash(), b.inputs_hash(), "inputs hash");
    assert_eq!(a.implementation_id(), b.implementation_id(), "implementation id");
    assert_eq!(a.version(), b.version(), "version");
    for d in probes() {
        assert_eq!(a.height(d).to_bits(), b.height(d).to_bits(), "height at {d:?}");
    }
}

// spec: GEN-016
#[test]
fn a_world_without_stamps_is_its_base_field_and_the_water_bound_wraps_it_on_request() {
    let src = OwnedBake(bake(7, &TangentWarp, 16, 2).unwrap());
    let def = Arc::new(WorldDefinition::single_planet(R, 7));
    let base = build_field(HeightFieldKind::Eroded, &GenContext::new(&def), BodyId::PLANET, &src, [1; 16]).unwrap();
    let chain = field_chain(HeightFieldKind::Eroded, &def, &src, [1; 16], false).unwrap();
    same(chain.as_ref(), base.as_ref());
    let bounded = field_chain(HeightFieldKind::Eroded, &def, &src, [1; 16], true).unwrap();
    same(bounded.as_ref(), chain.as_ref());
    assert_eq!(bounded.water_cull_level(), Some(def.sea(BodyId::PLANET).unwrap().level_m), "the bound reports the sea level");
    assert_eq!(chain.water_cull_level(), base.water_cull_level(), "without the bound the base decides");
}

// spec: GEN-026
#[test]
fn a_stamped_world_is_the_stamp_stage_over_the_base_field() {
    let src = OwnedBake(bake(7, &TangentWarp, 16, 2).unwrap());
    let mut def = WorldDefinition::single_planet(R, 7);
    let at = Vec3::new(1.0, 0.2, 0.1).normalized();
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
    let def = Arc::new(def);
    let base: Arc<dyn HeightField + '_> =
        Arc::from(build_field(HeightFieldKind::Eroded, &GenContext::new(&def), BodyId::PLANET, &src, [1; 16]).unwrap());
    let by_hand =
        StampStage::new(Arc::clone(&base), def.clone(), BodyId::PLANET, DEFAULT_ITEM_LAYER, LAYER_STACK.len() as u32, STAMP_STAGE_VERSION)
            .unwrap();
    let chain = field_chain(HeightFieldKind::Eroded, &def, &src, [1; 16], false).unwrap();
    same(chain.as_ref(), &by_hand);
    assert_ne!(chain.height(at).to_bits(), base.height(at).to_bits(), "the stamp acts at its centre");
}

// spec: GEN-039
#[test]
fn without_a_chunk_stage_the_layers_stack_in_the_order_of_adr_0007_whatever_their_chunk_stage_says() {
    use planet_world_def::stamp::{Stamp, StampKind};
    use planet_world_def::ItemSpec;
    let src = OwnedBake(bake(7, &TangentWarp, 16, 2).unwrap());
    let at = Vec3::new(1.0, 0.2, 0.1).normalized();
    let item = |id: u128, layer: &str, kind: StampKind, a: f64| {
        ItemSpec { id, layer: layer.into(), priority: 0, dir: at, content_hash: 0, reach_m: 3_000.0, max_delta_m: 500.0, stamp: None }
            .with_stamp(Stamp::new(kind, [a, 0.0, 0.0]))
    };
    let mut def = WorldDefinition::single_planet(R, 7);
    // A hill in `stamps` (stack index 3) and a plateau in `placed` (index 5) over the same place: the plateau is applied last in the stack.
    def.put_item_spec(BodyId::PLANET, item(1, "stamps", StampKind::Hill, 40.0)).unwrap();
    def.put_item_spec(BodyId::PLANET, item(2, "placed", StampKind::Plateau, 120.0)).unwrap();
    let plain = field_chain(HeightFieldKind::Eroded, &Arc::new(def.clone()), &src, [1; 16], false).unwrap();
    // `under` is the default: setting it explicitly builds the same field.
    def.set_chunk_stage(BodyId::PLANET, "stamps", planet_world_def::ChunkStagePosition::Under);
    let under = field_chain(HeightFieldKind::Eroded, &Arc::new(def.clone()), &src, [1; 16], false).unwrap();
    same(plain.as_ref(), under.as_ref());
    // The layers set to different positions: with no chunk stage the order is still the stack's, so the heights are the plain ones.
    def.set_chunk_stage(BodyId::PLANET, "stamps", planet_world_def::ChunkStagePosition::OnTop);
    let mixed = field_chain(HeightFieldKind::Eroded, &Arc::new(def), &src, [1; 16], false).unwrap();
    assert_eq!(plain.height(at).to_bits(), mixed.height(at).to_bits(), "a hill under a plateau, whatever the setting says");
    same(plain.as_ref(), mixed.as_ref());
}
