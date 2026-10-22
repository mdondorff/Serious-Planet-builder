//! WDEF-017: the relief scale of a body.

use planet_world_def::toml_io::{parse_definition, to_toml};
use planet_world_def::{BodyId, GenContext, WorldDefinition, RELIEF_SCALE_RANGE};

fn with(scale: f64) -> WorldDefinition {
    let mut d = WorldDefinition::single_planet(6_371_000.0, 5);
    d.set_relief_scale(BodyId::PLANET, scale);
    d
}

// spec: WDEF-017
#[test]
fn the_scale_defaults_to_one_is_kept_in_range_and_refuses_what_is_outside() {
    assert_eq!(WorldDefinition::single_planet(6_371_000.0, 5).relief_scale(BodyId::PLANET), Ok(1.0));
    for ok in [1.0, 2.5, 3.0, RELIEF_SCALE_RANGE.1] {
        assert_eq!(with(ok).relief_scale(BodyId::PLANET), Ok(ok));
    }
    for bad in [0.0, 0.25, 0.99, 4.01, -1.0, f64::NAN, f64::INFINITY] {
        let e = with(bad).relief_scale(BodyId::PLANET).unwrap_err();
        assert!(e.0.contains("body.0.relief_scale"), "the error names the node: {e}");
    }
}

// spec: WDEF-017
#[test]
fn the_scale_is_read_through_the_context_only_when_set_and_the_file_round_trips() {
    let plain = WorldDefinition::single_planet(6_371_000.0, 5);
    let ctx = GenContext::new(&plain);
    assert_eq!(ctx.relief_scale(BodyId::PLANET), Ok(1.0));
    assert!(ctx.read_records().is_empty(), "a world that sets no scale records no read: its keys stay");
    let set = with(2.5);
    let ctx = GenContext::new(&set);
    assert_eq!(ctx.relief_scale(BodyId::PLANET), Ok(2.5));
    assert_eq!(ctx.read_records().len(), 1);
    let three = with(3.0);
    let other = GenContext::new(&three);
    other.relief_scale(BodyId::PLANET).unwrap();
    assert_ne!(ctx.read_set_hash(), other.read_set_hash(), "each scale has its own read set");
    let text = to_toml(&set).unwrap();
    assert!(text.contains("relief_scale"), "{text}");
    assert_eq!(parse_definition(&text).unwrap().relief_scale(BodyId::PLANET), Ok(2.5));
    let by_hand =
        parse_definition("schema_version = 2\n\n[body.0]\nradius_m = 6371000.0\nrelief_scale = 2.0\n\n[world]\nseed = 1\n").unwrap();
    assert_eq!(by_hand.relief_scale(BodyId::PLANET), Ok(2.0));
}
