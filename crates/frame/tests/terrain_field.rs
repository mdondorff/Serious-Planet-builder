//! GEN-016: the terrain field plans at production parameters (cells 32, viewport / tau = 240) without steepness rejection
//! (ADR 0016, ADR 0028 point 6).

use planet_core::cube::TangentWarp;
use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::global::{bake, GridSource};
use planet_generators::height::HeightField;
use planet_generators::terrain::{TerrainGenerator, MAX_ABS_HEIGHT_M};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;

// spec: GEN-016
#[test]
fn the_standard_flight_plans_at_production_parameters_without_steepness_rejection() {
    let def = WorldDefinition::single_planet(R, 7);
    let ctx = GenContext::new(&def);
    let seed = ctx.body_seed(BodyId::PLANET).unwrap();
    let grid = bake(seed, &TangentWarp, 256, 4).unwrap();
    let src = GridSource(&grid);
    let ctx = GenContext::new(&def);
    let field = TerrainGenerator.field(&ctx, BodyId::PLANET, &src, [3; 16]).unwrap();
    let params = LodParams { max_height_m: MAX_ABS_HEIGHT_M, cells: 32, viewport_h_px: 1080.0, tau_px: 4.5, ..LodParams::earth_1080p() };
    assert_eq!(params.viewport_h_px / params.tau_px, 240.0);
    let mut deepest = 0;
    for (name, cam) in scripted_views_field(R, 16.0 / 9.0, &field) {
        let req = TerrainRequest {
            camera: cam,
            size: (192, 108),
            view: TerrainView::Level,
            lod: params.clone(),
            face_mapping: "tangent-v1".into(),
            seed: field.seed(),
        };
        eprintln!("planning {name}");
        let plan = plan_terrain_field(&req, &field).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!plan.nodes.is_empty(), "{name}");
        deepest = deepest.max(plan.nodes.iter().map(|n| n.level).max().unwrap());
    }
    assert!(field.error().is_none(), "{:?}", field.error());
    // The ground views reach the deepest levels: an early stop in planning would show up here.
    assert_eq!(
        deepest, params.max_level,
        "the ground views reach the production finest level (22 at cells 32, LOD-010): an early stop in planning would show up here"
    );
}

// spec: GEN-017
#[test]
fn the_eroded_field_plans_the_standard_flight_at_production_parameters_with_its_real_bounds() {
    use planet_generators::erosion::{ErosionGenerator, MAX_ABS_HEIGHT_M};
    let def = WorldDefinition::single_planet(R, 7);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    let grid = bake(seed, &TangentWarp, 256, 4).unwrap();
    let src = GridSource(&grid);
    let field = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, [3; 16]).unwrap();
    let params = LodParams { max_height_m: MAX_ABS_HEIGHT_M, cells: 32, viewport_h_px: 1080.0, tau_px: 4.5, ..LodParams::earth_1080p() };
    assert_eq!(params.viewport_h_px / params.tau_px, 240.0);
    let mut deepest = 0;
    for (name, cam) in scripted_views_field(R, 16.0 / 9.0, &field) {
        let req = TerrainRequest {
            camera: cam,
            size: (192, 108),
            view: TerrainView::Level,
            lod: params.clone(),
            face_mapping: "tangent-v1".into(),
            seed: field.seed(),
        };
        let plan = plan_terrain_field(&req, &field).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!plan.nodes.is_empty(), "{name}");
        deepest = deepest.max(plan.nodes.iter().map(|n| n.level).max().unwrap());
    }
    assert!(field.error().is_none(), "{:?}", field.error());
    assert_eq!(
        deepest, params.max_level,
        "the ground views reach the production finest level (22 at cells 32, LOD-010): an early stop in planning would show up here"
    );
}
