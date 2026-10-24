//! GEN-039: the `chunk_stage` setting of a layer: `OnTop` by default, `Under` representable and refused with the layer named.

use planet_world_def::{BodyId, ChunkStagePosition, WorldDefinition};

// spec: GEN-039, WDEF-018
#[test]
fn a_layer_without_the_setting_is_on_top_and_the_definition_hashes_as_it_always_did() {
    let mut d = WorldDefinition::single_planet(6_371_000.0, 7);
    let before = d.hash();
    assert_eq!(d.chunk_stage(BodyId::PLANET, "stamps"), ChunkStagePosition::OnTop);
    assert!(d.check_chunk_stages(BodyId::PLANET).is_ok());
    // Setting the default is no setting.
    d.set_chunk_stage(BodyId::PLANET, "stamps", ChunkStagePosition::OnTop);
    assert_eq!(d.hash(), before);
}

// spec: GEN-039, WDEF-018
#[test]
fn under_is_representable_changes_the_hash_and_is_refused_naming_the_layer() {
    let mut d = WorldDefinition::single_planet(6_371_000.0, 7);
    let plain = d.hash();
    d.set_chunk_stage(BodyId::PLANET, "placed", ChunkStagePosition::Under);
    assert_eq!(d.chunk_stage(BodyId::PLANET, "placed"), ChunkStagePosition::Under);
    assert_ne!(d.hash(), plain, "the setting is part of the definition");
    let e = d.check_chunk_stages(BodyId::PLANET).unwrap_err();
    assert!(e.contains("layer 'placed'") && e.contains("under") && e.contains("on-top"), "{e}");
    // Another layer's setting does not hide it, and another body is not refused.
    d.set_chunk_stage(BodyId::PLANET, "stamps", ChunkStagePosition::OnTop);
    assert!(d.check_chunk_stages(BodyId::PLANET).is_err());
    assert!(d.check_chunk_stages(BodyId(5)).is_ok());
    // Back to the default: accepted, and the hash is the plain one again.
    d.set_chunk_stage(BodyId::PLANET, "placed", ChunkStagePosition::OnTop);
    assert!(d.check_chunk_stages(BodyId::PLANET).is_ok());
    assert_eq!(d.hash(), plain);
}
