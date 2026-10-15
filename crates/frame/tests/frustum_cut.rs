//! LOD-007, tier A: the frustum cut uses each tile's own height bounds, cheaply. The depth-pass oracle over the ten standard
//! views at production parameters is tier B (`oracle_frustum_cut.rs`).

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::{
    plan_terrain, scripted_views, scripted_views_field, select_nodes_field_cached, select_nodes_in_view_field, Camera, LodParams,
    SelectionCache, TerrainRequest, TerrainView,
};
use planet_generators::height::Reference;
use planet_generators::mesh::tile_mesh_field;
use planet_generators::stress::Steep;

#[path = "support/frustum_cut.rs"]
mod support;
use support::{cameras, check_field, dropped, R};

// spec: LOD-007
#[test]
fn a_cut_tile_has_no_vertex_inside_the_frustum() {
    // Every eighth dropped tile of every camera here, every second one in the tier-B twin `oracle_frustum_cut_sweep.rs`.
    check_field(&Reference(1), scripted_views(R, 16.0 / 9.0, 1), 8, 14, 6.0, 8, &|_| true);
}

// spec: LOD-007
#[test]
fn a_cut_tile_has_no_vertex_inside_the_frustum_on_the_steep_field() {
    // Steep terrain (|h| up to 8 km) is the case where a global height pad and a tight per-tile bound differ most. The steep
    // meshes cost most of this tier's time, so the tier-A slice is the ground-200m and cube-corner-1km views at their own
    // heading and the 35 degree variant, every eighth dropped tile; the tier-B twin `oracle_frustum_cut_sweep.rs` runs all
    // four headings of both views at every second tile.
    let field = Steep(1);
    let views =
        scripted_views_field(R, 16.0 / 9.0, &field).into_iter().filter(|(n, _)| ["ground-200m", "cube-corner-1km"].contains(n)).collect();
    check_field(&field, views, 8, 11, 2.0, 8, &|name| !name.ends_with("/5") && !name.ends_with("/70"));
}

// spec: LOD-007
#[test]
fn the_cut_drops_tiles_the_global_height_pad_kept() {
    // Near the ground the global pad (max_height_m, 9 km) keeps almost the whole hemisphere's tiles; the tile's own height
    // bounds do not. At ground-1m, production parameters, the plan holds well under half of the old 8,687 nodes.
    let size = (2560u32, 1440u32);
    let lod = LodParams { cells: 32, viewport_h_px: f64::from(size.1), tau_px: 6.0, max_level: 26, ..LodParams::earth_1080p() };
    let (_, camera) = scripted_views(R, f64::from(size.0) / f64::from(size.1), 1)[6];
    let req = TerrainRequest { camera, size, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 };
    let n = plan_terrain(&req).unwrap().nodes.len();
    assert!(n < 4_000, "ground-1m plans {n} nodes: the frustum cut should use the tile's own height bounds (old pad: 8,687)");
}

// spec: LOD-007
#[test]
fn the_first_stage_pad_covers_the_skirts_of_the_coarse_levels() {
    // Skirts of the coarse levels hang kilometres below their tile (12 km at level 6, more above): the cheap first test must
    // pad them too. Leaves of the whole-sphere selection up to level 6 that the cut dropped have every vertex outside one
    // side plane, from the sky and from the ground.
    let field = Reference(1);
    let lod = LodParams { cells: 4, viewport_h_px: 360.0, tau_px: 40.0, max_level: 6, cull_horizon: false, ..LodParams::earth_1080p() };
    let mut checked = 0;
    for (name, cam) in
        cameras(scripted_views(R, 16.0 / 9.0, 1)).into_iter().filter(|(n, _)| !n.starts_with("ground") && !n.starts_with("low"))
    {
        let all: Vec<TileId> = select_nodes_field_cached(&TangentWarp, cam.position, &field, &lod, &mut SelectionCache::new())
            .nodes
            .iter()
            .map(|n| n.tile)
            .collect();
        let kept: Vec<TileId> = select_nodes_in_view_field(&TangentWarp, &cam, &field, &lod).nodes.iter().map(|n| n.tile).collect();
        let frustum = cam.frustum();
        for tile in dropped(&all, &kept) {
            let mesh = tile_mesh_field(&field, &TangentWarp, tile, 4, R);
            let points: Vec<Vec3> = mesh
                .vertices
                .iter()
                .map(|v| mesh.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2])))
                .collect();
            assert!(
                frustum.normals.iter().any(|n| points.iter().all(|p| (*p - frustum.origin).dot(*n) < 0.0)),
                "{name}: dropped tile {tile:?} (level {}) has vertices inside every side plane",
                tile.level()
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "checked {checked}");
}

// spec: LOD-008
#[test]
fn turning_the_camera_in_place_does_not_change_the_morph_start_of_leaves_in_both_views() {
    for (label, field) in [("ref", &Reference(1) as &dyn planet_generators::height::HeightField), ("steep", &Steep(1))] {
        let lod = LodParams { cells: 8, viewport_h_px: 360.0, tau_px: 2.0, max_level: 11, ..LodParams::earth_1080p() };
        for (name, cam) in scripted_views_field(R, 16.0 / 9.0, field).into_iter().filter(|(n, _)| ["low-2km", "ground-200m"].contains(n)) {
            let mut sel = Vec::new();
            for hint in [Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.15, 1.0), Vec3::new(0.0, -0.15, 1.0)] {
                let c = Camera::at_surface_point(cam.position, hint, 20f64.to_radians(), cam.fov_y_rad, cam.aspect, cam.near_m);
                sel.push(select_nodes_in_view_field(&TangentWarp, &c, field, &lod));
            }
            let (mut diffs, mut common) = (0, 0);
            for n in &sel[0].nodes {
                for o in &sel[1..] {
                    if let Some(m) = o.nodes.iter().find(|m| m.tile == n.tile) {
                        common += 1;
                        if m.morph_start_m != n.morph_start_m {
                            diffs += 1;
                        }
                    }
                }
            }
            assert!(common > 30, "{label} {name}: only {common} leaves in common");
            assert_eq!(diffs, 0, "{label} {name}: {diffs} of {common} shared leaves change their morph start with the heading");
        }
    }
}
