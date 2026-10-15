//! TEST-011 for the default world: the per-view counts of the ten standard views planned over the eroded field (GEN-018,
//! M4 item 7d). `crates/frame/tests/terrain.rs` pins the same views over the reference field, which the goldens keep.

use planet::worldfield::WorldTerrain;
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_frame::{plan_terrain_field, scripted_views_field, CountBudget, LodParams, TerrainRequest, TerrainView};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
/// The bake the pins are made over: the default `--global-res` of a run. The counts depend on it (the eroded field's height
/// bounds and slopes come from the bake), so it is part of the pin.
const PIN_BAKE_RES: u32 = 256;

/// Nodes per standard view, the default world (eroded field over a 256-cell bake), cells 32, tau 6 px, 1440p. The other counts
/// follow from it (2,304 triangles and 1,221 x 44 vertex bytes per tile). Pinned 2026-10-15 by `world-field-everywhere` (the
/// default world is the eroded field); raising a pin is a decision: say why in the change that does it (CON-20).
const EROSION_VIEW_NODE_PINS: [(&str, u32); 10] = [
    ("orbit-20000km", 52),
    ("high-2000km", 430),
    ("aerial-200km", 634),
    ("aerial-20km", 900),
    ("low-2km", 1299),
    ("ground-200m", 1719),
    ("ground-1m", 2936),
    ("north-pole-1km", 1128),
    ("cube-corner-1km", 1594),
    ("face-edge-1km", 1551),
];

// spec: TEST-011
#[test]
fn every_standard_view_of_the_default_world_stays_within_its_own_pinned_counts() {
    let world = WorldTerrain::open(WorldDefinition::single_planet(R, 1), PIN_BAKE_RES).unwrap();
    assert_eq!(world.id(), "cpu-erosion-v1", "the default world is the eroded field");
    let field = world.field().unwrap();
    let size = (2560u32, 1440u32);
    let lod = LodParams {
        cells: 32,
        viewport_h_px: f64::from(size.1),
        tau_px: 6.0,
        max_level: 26,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    let views = scripted_views_field(R, f64::from(size.0) / f64::from(size.1), field.as_ref());
    assert_eq!(views.len(), EROSION_VIEW_NODE_PINS.len(), "a standard view was added or removed: pin it");
    let mut failures = Vec::new();
    let mut report = Vec::new();
    for ((name, camera), (pinned_name, pin)) in views.into_iter().zip(EROSION_VIEW_NODE_PINS) {
        assert_eq!(name, pinned_name, "the pins are in the order of the standard views");
        let req = TerrainRequest {
            camera,
            size,
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: TangentWarp.id().to_string(),
            seed: field.seed(),
        };
        let plan = plan_terrain_field(&req, field.as_ref()).unwrap();
        let c = plan.counts();
        assert_eq!(c.draw_calls as usize, plan.nodes.len(), "{name}");
        assert_eq!(c.triangles, plan.nodes.len() as u64 * 2_304, "{name}");
        assert_eq!(c.upload_bytes, u64::from(c.resident_tiles) * 1_221 * 44, "{name}");
        assert_eq!(c.resident_tiles as usize, plan.nodes.len(), "{name}: every node of a plan is a distinct tile");
        c.check(&CountBudget::REFERENCE_1440P).unwrap_or_else(|e| panic!("{name}: {e}"));
        report.push(format!("(\"{name}\", {}),", plan.nodes.len()));
        let budget = CountBudget {
            max_draw_calls: pin,
            max_triangles: u64::from(pin) * 2_304,
            max_resident_tiles: pin,
            max_upload_bytes: u64::from(pin) * 1_221 * 44,
        };
        if let Err(e) = c.check(&budget) {
            failures.push(format!("{name}: {e} (nodes {} against pin {pin})", plan.nodes.len()));
        }
        // A pin stays tight: a view more than 10 % under its pin has its pin lowered with it.
        if u64::from(pin) > plan.nodes.len() as u64 * 10 / 9 {
            failures.push(format!("{name}: {} nodes is more than 10 % under its pin {pin}: lower the pin", plan.nodes.len()));
        }
    }
    assert!(failures.is_empty(), "plan counts against their pins:\n{}\nmeasured:\n{}", failures.join("\n"), report.join("\n"));
}
