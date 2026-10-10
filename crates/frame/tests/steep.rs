//! TEST-015: level transitions and precision on the steep stress field (generators::stress::Steep).

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_terrain_field, scripted_views_field, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
use planet_generators::mesh::tile_mesh_field;
use planet_generators::stress::Steep;
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;
const STEEP: Steep = Steep(1);

fn clip_f32(m: &[f32; 16], p: [f32; 3]) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    for (r, o) in out.iter_mut().enumerate() {
        *o = m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r];
    }
    out
}

// spec: TEST-015
#[test]
fn morph_closes_the_gap_between_levels_on_steep_terrain_in_every_scripted_view() {
    let map = TangentWarp;
    let params = LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let n = params.cells as usize;
    let (mut coarse_cases, mut fine_cases) = (0, 0);
    for (name, cam) in scripted_views_field(R, 4.0 / 3.0, &STEEP) {
        let req = TerrainRequest {
            camera: cam,
            size: (128, 96),
            view: TerrainView::Level,
            lod: params.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        };
        let plan = plan_terrain_field(&req, &STEEP).unwrap();
        let leaves: BTreeMap<TileId, usize> = plan.nodes.iter().enumerate().map(|(i, d)| (d.tile, i)).collect();
        for d in plan.nodes.iter().filter(|d| d.level > 0) {
            let mesh = tile_mesh_field(&STEEP, &map, d.tile, params.cells, R);
            let morph_at = |i: usize, j: usize| {
                let v = mesh.vertices[j * (n + 1) + i];
                let rel =
                    mesh.origin.0 - cam.position.0 + Vec3::new(f64::from(v.ref_pos[0]), f64::from(v.ref_pos[1]), f64::from(v.ref_pos[2]));
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
                if nb.parent().is_some_and(|p| leaves.contains_key(&p)) {
                    let worst = border.iter().map(|&(i, j)| morph_at(i, j)).fold(1.0f64, f64::min);
                    assert!(
                        worst > 0.999,
                        "{name}: {:?} (level {}) borders a coarser leaf on {side:?} but only morphs to {worst}",
                        d.tile,
                        d.level
                    );
                    coarse_cases += 1;
                } else if !leaves.contains_key(&nb) && nb.children().is_some_and(|c| c.iter().any(|x| leaves.contains_key(x))) {
                    let worst = border.iter().map(|&(i, j)| morph_at(i, j)).fold(0.0f64, f64::max);
                    assert!(
                        worst < 1e-3,
                        "{name}: {:?} (level {}) borders finer leaves on {side:?} but already morphs ({worst})",
                        d.tile,
                        d.level
                    );
                    fine_cases += 1;
                }
            }
        }
    }
    assert!(coarse_cases > 50 && fine_cases > 50, "too few level transitions: {coarse_cases} coarser, {fine_cases} finer");
}

// spec: TEST-015
#[test]
fn steep_vertices_project_within_a_quarter_pixel_at_one_metre() {
    // The highest steep terrain among random directions, the camera 1 m above it moving in 0.1 mm steps, through the
    // shader path in f32 (origin_rel + local offset, morph mix, f32 matrix), against the exact f64 projection.
    let map = TangentWarp;
    let mut rng = SplitMix64(31);
    let mut best = (f64::MIN, Vec3::new(1.0, 0.0, 0.0));
    for _ in 0..4000 {
        let d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized();
        let h = STEEP.height(d);
        if h > best.0 {
            best = (h, d);
        }
    }
    let (w, h) = (1920.0f64, 1080.0f64);
    let mut worst = 0.0f64;
    let mesh = tile_mesh_field(&STEEP, &map, TileId::from_direction(&map, best.1, 26), 16, R);
    for (vi, vj, m) in [(8usize, 8usize, 0.0f32), (9, 8, 0.5), (9, 9, 1.0)] {
        let v = mesh.vertices[vj * 17 + vi];
        let fine_world = mesh.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]));
        let coarse_world = mesh.origin.0 + Vec3::new(f64::from(v.coarse[0]), f64::from(v.coarse[1]), f64::from(v.coarse[2]));
        let exact_world = fine_world * f64::from(1.0 - m) + coarse_world * f64::from(m);
        let up = fine_world.normalized();
        let along = (Vec3::new(1.0, 0.0, 0.0) - up * up.x).normalized();
        for step in 0..200 {
            let cam_pos = PlanetFixed(fine_world + up * 1.0 + along * (step as f64 * 1e-4));
            let cam = Camera::at_surface_point(cam_pos, Vec3::new(0.0, 0.0, 1.0), 45f64.to_radians(), 60f64.to_radians(), w / h, 0.1);
            let rel = [
                (mesh.origin.0.x - cam.position.0.x) as f32,
                (mesh.origin.0.y - cam.position.0.y) as f32,
                (mesh.origin.0.z - cam.position.0.z) as f32,
            ];
            let p = [0, 1, 2].map(|k| (rel[k] + v.pos[k]) * (1.0 - m) + (rel[k] + v.coarse[k]) * m);
            let c = clip_f32(&cam.view_projection(), p);
            let exact = cam.project(exact_world - cam.position.0);
            let px = |x: f64, wv: f64, size: f64| x / wv * size / 2.0;
            let (gx, gy) = (px(f64::from(c[0]), f64::from(c[3]), w), px(f64::from(c[1]), f64::from(c[3]), h));
            let (ex, ey) = (px(exact[0], exact[3], w), px(exact[1], exact[3], h));
            worst = worst.max(((gx - ex) * (gx - ex) + (gy - ey) * (gy - ey)).sqrt());
        }
    }
    eprintln!("steep terrain height {:.0} m: worst jitter {worst} px", best.0);
    assert!(best.0 > 4000.0, "the test must exercise high steep terrain, found only {:.0} m", best.0);
    assert!(worst < 0.25, "projected position jitter {worst} px at 1 m on steep terrain; budget 0.25 px (CON-13)");
}
