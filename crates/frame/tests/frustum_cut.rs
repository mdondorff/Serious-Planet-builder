//! LOD-007, tier A: the frustum cut uses each tile's own height bounds, cheaply. The depth-pass oracle over the ten standard
//! views at production parameters is tier B (`oracle_frustum_cut.rs`).

use std::collections::BTreeSet;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::{
    plan_terrain, scripted_views, scripted_views_field, select_nodes_field_cached, select_nodes_in_view_field, Camera, LodParams,
    SelectionCache, TerrainRequest, TerrainView,
};
use planet_generators::height::Reference;
use planet_generators::mesh::tile_mesh_field;
use planet_generators::stress::Steep;

const R: f64 = 6_371_000.0;

/// The tiles of `all` that the cut dropped for real: not in the cut selection, with no ancestor and no descendant in it (a
/// leaf that balancing replaced by its children, or the reverse, is still drawn).
fn dropped(all: &[TileId], kept: &[TileId]) -> Vec<TileId> {
    let kept_set: BTreeSet<TileId> = kept.iter().copied().collect();
    let mut related: BTreeSet<TileId> = BTreeSet::new();
    for &k in kept {
        let mut t = k;
        while let Some(p) = t.parent() {
            related.insert(p);
            t = p;
        }
    }
    all.iter()
        .copied()
        .filter(|&t| {
            let mut a = t;
            while let Some(p) = a.parent() {
                if kept_set.contains(&p) {
                    return false;
                }
                a = p;
            }
            !kept_set.contains(&t) && !related.contains(&t)
        })
        .collect()
}

/// Cameras: every standard view and its pitch and heading variants, so that tiles sit at the edges of the frustum.
fn cameras(views: Vec<(&'static str, Camera)>) -> Vec<(String, Camera)> {
    let mut out = Vec::new();
    for (name, cam) in views {
        out.push((name.to_string(), cam));
        if name == "orbit-20000km" {
            continue;
        }
        for (hint, pitch) in [(Vec3::new(0.0, 0.0, 1.0), 5.0f64), (Vec3::new(0.0, 1.0, 0.0), 35.0), (Vec3::new(1.0, 0.3, -0.5), 70.0)] {
            let c = Camera::at_surface_point(cam.position, hint, pitch.to_radians(), cam.fov_y_rad, cam.aspect, cam.near_m);
            out.push((format!("{name}/{hint:?}/{pitch}"), c));
        }
    }
    out
}

fn check_field(
    field: &dyn planet_generators::height::HeightField,
    views: Vec<(&'static str, Camera)>,
    cells: u32,
    max_level: u8,
    tau: f64,
) {
    let lod = LodParams { cells, viewport_h_px: 360.0, tau_px: tau, max_level, ..LodParams::earth_1080p() };
    let mut checked = 0usize;
    for (name, cam) in cameras(views) {
        let lod = LodParams { max_height_m: lod.max_height_m.max(field.max_abs_height_m()), ..lod.clone() };
        let all: Vec<TileId> = select_nodes_field_cached(&TangentWarp, cam.position, field, &lod, &mut SelectionCache::new())
            .nodes
            .iter()
            .map(|n| n.tile)
            .collect();
        let kept: Vec<TileId> = select_nodes_in_view_field(&TangentWarp, &cam, field, &lod).nodes.iter().map(|n| n.tile).collect();
        let frustum = cam.frustum();
        for (i, t) in dropped(&all, &kept).into_iter().enumerate() {
            if i % 2 != 0 {
                continue;
            }
            // Every vertex of the cut tile, skirts and morph targets included, lies outside one and the same side plane (what
            // a sphere test guarantees; "outside some plane each" would also pass for a tile straddling the frustum).
            let mesh = tile_mesh_field(field, &TangentWarp, t, cells, R);
            let points: Vec<Vec3> = mesh
                .vertices
                .iter()
                .flat_map(|v| [v.pos, v.coarse])
                .map(|p| mesh.origin.0 + Vec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])))
                .collect();
            assert!(
                frustum.normals.iter().any(|n| points.iter().all(|p| (*p - frustum.origin).dot(*n) < 0.0)),
                "{name}: cut tile {t:?} has vertices on the inside of every side plane"
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "the cameras must cut enough tiles to mean something (checked {checked})");
}

// spec: LOD-007
#[test]
fn a_cut_tile_has_no_vertex_inside_the_frustum() {
    check_field(&Reference(1), scripted_views(R, 16.0 / 9.0, 1), 8, 14, 6.0);
}

// spec: LOD-007
#[test]
fn a_cut_tile_has_no_vertex_inside_the_frustum_on_the_steep_field() {
    // Steep terrain (|h| up to 8 km) is the case where a global height pad and a tight per-tile bound differ most.
    let field = Steep(1);
    let views =
        scripted_views_field(R, 16.0 / 9.0, &field).into_iter().filter(|(n, _)| ["ground-200m", "cube-corner-1km"].contains(n)).collect();
    check_field(&field, views, 8, 11, 2.0);
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
