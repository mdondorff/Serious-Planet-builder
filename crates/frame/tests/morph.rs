//! Crack-free morphing between levels (REND-009, LOD-004), degenerate cameras (LOD-007), parameter validation (LOD-006).

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_terrain, scripted_views, select_nodes_in_view, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::mesh::tile_mesh;
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;

fn lod() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

// spec: REND-009, LOD-004
#[test]
fn morph_closes_the_gap_between_levels_in_every_scripted_view() {
    // The shader formula in f64: a vertex distance is measured to its height-free reference position.
    let map = TangentWarp;
    let configs = [
        ("test", lod()),
        ("skeleton", LodParams { cells: 16, viewport_h_px: 192.0, tau_px: 4.0, max_level: 26, ..LodParams::earth_1080p() }),
    ];
    let (mut coarse_cases, mut fine_cases) = (0, 0);
    for (cfg, params) in configs {
        for (name, cam) in scripted_views(R, 4.0 / 3.0, 1).into_iter().take(3) {
            let req = TerrainRequest {
                camera: cam,
                size: (128, 96),
                view: TerrainView::Level,
                lod: params.clone(),
                face_mapping: "tangent-v1".into(),
                seed: 1,
            };
            let plan = plan_terrain(&req).unwrap();
            let leaves: BTreeMap<TileId, usize> = plan.nodes.iter().enumerate().map(|(i, n)| (n.tile, i)).collect();
            let n = params.cells as usize;
            for (idx, d) in plan.nodes.iter().enumerate() {
                if d.level == 0 {
                    continue;
                }
                let mesh = tile_mesh(1, &map, d.tile, params.cells, R);
                let morph_at = |i: usize, j: usize| {
                    let v = mesh.vertices[j * (n + 1) + i];
                    let rel = mesh.origin.0 - cam.position.0
                        + Vec3::new(f64::from(v.ref_pos[0]), f64::from(v.ref_pos[1]), f64::from(v.ref_pos[2]));
                    ((rel.length() - f64::from(d.morph_start_m)) / f64::from((d.morph_end_m - d.morph_start_m).max(1e-6))).clamp(0.0, 1.0)
                };
                for side in Side::ALL {
                    let nb = d.tile.neighbor(&map, side);
                    let border: Vec<(usize, usize)> = (0..=n)
                        .map(|k| match side {
                            Side::East => (n, k),
                            Side::West => (0, k),
                            Side::North => (k, n),
                            Side::South => (k, 0),
                        })
                        .collect();
                    let coarser = nb.parent().is_some_and(|p| leaves.contains_key(&p));
                    let finer = !leaves.contains_key(&nb) && nb.children().is_some_and(|c| c.iter().any(|x| leaves.contains_key(x)));
                    if coarser {
                        let worst = border.iter().map(|&(i, j)| morph_at(i, j)).fold(1.0f64, f64::min);
                        assert!(
                            worst > 0.999,
                            "{cfg}/{name}: node {idx} {:?} (level {}) borders a coarser leaf on {side:?} but its border vertices only morph to {worst}",
                            d.tile,
                            d.level
                        );
                        coarse_cases += 1;
                    } else if finer {
                        let worst = border.iter().map(|&(i, j)| morph_at(i, j)).fold(0.0f64, f64::max);
                        assert!(
                            worst < 1e-3,
                            "{cfg}/{name}: node {idx} {:?} (level {}) borders finer leaves on {side:?} but its border vertices already morph ({worst})",
                            d.tile,
                            d.level
                        );
                        fine_cases += 1;
                    }
                }
            }
        }
    }
    assert!(coarse_cases > 50 && fine_cases > 50, "too few level transitions exercised: {coarse_cases} coarser, {fine_cases} finer");
}

// spec: LOD-007
#[test]
fn degenerate_cameras_do_not_panic() {
    let fov = 1.0;
    let over_pole = Camera::looking_at_centre(PlanetFixed(Vec3::new(0.0, 0.0, 2.0e7)), Vec3::new(0.0, 0.0, 1.0), fov, 1.3, 0.1);
    let straight_down = Camera::at_surface_point(
        PlanetFixed(Vec3::new(R, 0.0, 0.0)),
        Vec3::new(0.0, 0.0, 1.0),
        core::f64::consts::FRAC_PI_2,
        fov,
        1.3,
        0.1,
    );
    let hint_parallel = Camera::at_surface_point(PlanetFixed(Vec3::new(R, 0.0, 0.0)), Vec3::new(1.0, 0.0, 0.0), 0.2, fov, 1.3, 0.1);
    for (name, cam) in [("over-pole", over_pole), ("straight-down", straight_down), ("hint-parallel-to-up", hint_parallel)] {
        assert!(cam.view_projection().iter().all(|v| v.is_finite()), "{name}");
        let fr = cam.frustum();
        assert!(fr.normals.iter().all(|n| n.x.is_finite() && n.y.is_finite() && n.z.is_finite()), "{name}");
        assert!(!select_nodes_in_view(&TangentWarp, &cam, &lod()).nodes.is_empty(), "{name}");
    }
}

// spec: LOD-006
#[test]
fn unusable_lod_parameters_are_rejected_with_advice() {
    let (_, cam) = scripted_views(R, 4.0 / 3.0, 1)[3];
    let bad = LodParams { cells: 32, viewport_h_px: 96.0, tau_px: 4.0, ..lod() };
    let req =
        TerrainRequest { camera: cam, size: (128, 96), view: TerrainView::Face, lod: bad, face_mapping: "tangent-v1".into(), seed: 1 };
    let e = plan_terrain(&req).unwrap_err();
    assert!(e.to_string().contains("morph interval") && e.to_string().contains("tau_px"), "{e}");
    assert!(LodParams::earth_1080p().validate().is_ok());
    assert!(lod().validate().is_ok());
}
