//! LOD-007: the frustum cut uses each tile's own height bounds. Oracle: a CPU depth pass (support/depth_oracle.rs, from
//! SPIKE-05) over every tile the horizon cut leaves; no tile with a visible triangle may be missing from the plan. The depth
//! pass rasterizes at 1280x720 (the plan is for 2560x1440, same aspect), so it is slightly less sensitive than the real frame.

use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_frame::{plan_terrain, scripted_views, select_nodes_field_cached, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::height::Reference;

#[path = "support/depth_oracle.rs"]
mod depth_oracle;

const R: f64 = 6_371_000.0;

// spec: LOD-007
#[test]
fn no_tile_with_a_visible_triangle_is_cut_in_the_standard_views() {
    depth_oracle::self_check();
    let size = (2560u32, 1440u32);
    let lod = LodParams { cells: 32, viewport_h_px: f64::from(size.1), tau_px: 6.0, max_level: 26, ..LodParams::earth_1080p() };
    let field = Reference(1);
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for (name, camera) in scripted_views(R, f64::from(size.0) / f64::from(size.1), 1) {
        // Every tile the horizon cut leaves (no frustum): the superset the oracle looks through.
        let all: Vec<TileId> = select_nodes_field_cached(&TangentWarp, camera.position, &field, &lod, &mut SelectionCache::new())
            .nodes
            .iter()
            .map(|n| n.tile)
            .collect();
        let visible = depth_oracle::visible_tiles(&camera, (1280, 720), &all, lod.cells, R, &field);
        let req = TerrainRequest { camera, size, view: TerrainView::Level, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 1 };
        let plan: std::collections::BTreeSet<TileId> = plan_terrain(&req).unwrap().nodes.iter().map(|n| n.tile).collect();
        let cut: Vec<_> = visible.iter().filter(|t| !plan.contains(t)).collect();
        report.push(format!("{name}: surroundings {} visible {} plan {}", all.len(), visible.len(), plan.len()));
        if !cut.is_empty() {
            failures.push(format!("{name}: {} of {} tiles with a visible triangle are cut, first {:?}", cut.len(), visible.len(), cut[0]));
        }
    }
    println!("{}", report.join("\n"));
    assert!(failures.is_empty(), "the frustum cut removed visible tiles:\n{}", failures.join("\n"));
}
