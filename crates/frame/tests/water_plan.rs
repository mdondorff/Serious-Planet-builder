//! The plan's water level W comes from the water-level field (ADR 0041 point 1, ADR 0039 point 3): GPU-free plan data.

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::look::LitPlan;
use planet_generators::global::{bake, GridSource};
use planet_generators::ocean::{tiled_ocean, EARTH_OCEAN_MIN_AREA_FRACTION};
use planet_generators::water::{SeaProducer, WaterField};
use planet_world_def::{BodyId, SeaParams, WorldDefinition};

// spec: LOOK-007
// spec: GEN-023
#[test]
fn the_plan_shades_against_the_level_the_water_field_gives_and_the_default_text_is_unchanged() {
    // The default: a sea at level 0 leaves the plan text, and so its hash, as they were before the field existed.
    let plain = LitPlan::fixed(14.0);
    assert_eq!(plain.with_water_level(0.0).to_text(), plain.to_text());
    assert!(!plain.to_text().contains("water"));

    let grid = bake(3, &TangentWarp, 64, 2).unwrap();
    let (_, ocean) = tiled_ocean(&grid, &TangentWarp, 16, 2, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    let src = GridSource(&grid);
    let mut def = WorldDefinition::single_planet(6_371_000.0, 3);
    let default_sea = def.sea(BodyId::PLANET).unwrap();
    for sea in [default_sea, SeaParams { level_m: -120.5, ..default_sea }] {
        let producer = SeaProducer::new(&ocean, sea);
        let field = WaterField::new(&TangentWarp, &src, vec![&producer]);
        // Over the sea, over land and from orbit the sea's level is the plan's water level.
        let up = Vec3::new(0.3, -0.5, 0.81).normalized();
        let level = field.plan_water(up).unwrap().level_m;
        assert_eq!(level, sea.level_m);
        let plan = plain.with_water_level(level);
        assert_eq!(plan.water_level_m, sea.level_m as f32);
        let tokens_ok = plan.to_text().contains(" water ") == (sea.level_m != 0.0);
        assert!(tokens_ok, "{}", plan.to_text());
        let text = plan.to_text();
        let tokens: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(LitPlan::from_tokens(&tokens[1..]).unwrap(), plan, "the level round-trips through the plan text");
    }
    // The definition's node moves the level the plan gets.
    def.set_sea(BodyId::PLANET, &SeaParams { level_m: 40.0, ..default_sea });
    assert_eq!(def.sea(BodyId::PLANET).unwrap().level_m, 40.0);
}
