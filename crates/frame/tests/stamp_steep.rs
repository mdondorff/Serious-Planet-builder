//! GEN-026 (Bounds): a steep base field with stamps under the camera plans at the fine levels (16 to 18 and below) without an empty morph
//! interval, whatever the view: the stage's bounds follow the delta the stamps add inside a tile, not their declared maximum.

use std::sync::Arc;

use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
use planet_generators::stamp::{StampStage, STAMP_STAGE_VERSION};
use planet_generators::stress::Steep;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn stamp(id: u128, layer: &str, at: planet_core::Vec3, kind: StampKind, amount: f64, reach_m: f64, bound: f64) -> ItemSpec {
    ItemSpec { id, layer: layer.into(), priority: 0, dir: at, content_hash: 0, reach_m, max_delta_m: bound, stamp: None }
        .with_stamp(Stamp::new(kind, [amount, 0.0, 0.0]))
}

// spec: GEN-026
#[test]
fn a_steep_base_with_stamps_under_the_camera_plans_down_to_the_fine_levels_without_an_empty_morph_interval() {
    let steep = Steep(1);
    let views = scripted_views_field(R, 4.0 / 3.0, &steep);
    let mut deepest = 0u8;
    for (index, (name, camera)) in views.iter().enumerate().filter(|(_, (n, _))| *n == "ground-200m" || *n == "ground-1m") {
        let at = camera.position.0.normalized();
        let mut def = WorldDefinition::single_planet(R, 1);
        // A hill and a crater on the ground under the camera, a plateau a little off, bounds of 60 m each.
        def.put_item_spec(BodyId::PLANET, stamp(1, DEFAULT_ITEM_LAYER, at, StampKind::Hill, 50.0, 800.0, 60.0)).unwrap();
        let side = planet_core::Vec3::new(at.y, -at.x, 0.0).normalized();
        let near = (at * libm::cos(300.0 / R) + side * libm::sin(300.0 / R)).normalized();
        def.put_item_spec(BodyId::PLANET, stamp(2, DEFAULT_ITEM_LAYER, near, StampKind::Crater, 40.0, 500.0, 60.0)).unwrap();
        let base: Arc<dyn HeightField> = Arc::new(steep);
        let field = StampStage::new(base, Arc::new(def), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, STAMP_STAGE_VERSION).unwrap();
        // The parameters of the steep tests: a small grid and a large ratio of split distance to tile size leave room to morph.
        let lod =
            LodParams { cells: 4, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, max_height_m: 20_000.0, ..LodParams::earth_1080p() };
        let req =
            TerrainRequest { camera: *camera, size: (128, 96), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 };
        let plan = plan_terrain_field(&req, &field).unwrap_or_else(|e| panic!("view {index} '{name}': {e}"));
        let finest = plan.nodes.iter().map(|n| n.level).max().unwrap();
        deepest = deepest.max(finest);
    }
    assert!(deepest >= 18, "the views reach the fine levels with the stamps in reach: the deepest tile is level {deepest}");
}
