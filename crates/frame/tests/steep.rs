//! TEST-015: level transitions and precision on the steep stress field (generators::stress::Steep).

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{plan_terrain_field, scripted_views_field, Camera, LodParams, PlanError, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
use planet_generators::mesh::tile_mesh_field;
use planet_generators::stress::Steep;

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
fn height_fields_that_do_not_fit_the_request_are_plan_errors() {
    let (_, cam) = scripted_views_field(R, 4.0 / 3.0, &STEEP)[3];
    let lod = LodParams { cells: 4, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let req = TerrainRequest {
        camera: cam,
        size: (128, 96),
        view: TerrainView::Level,
        lod: lod.clone(),
        face_mapping: "tangent-v1".into(),
        seed: 2,
    };
    assert!(matches!(plan_terrain_field(&req, &STEEP), Err(PlanError::HeightField(m)) if m.contains("seed")));
    let low = TerrainRequest { seed: 1, lod: LodParams { max_height_m: 5000.0, ..lod }, ..req };
    let e = plan_terrain_field(&low, &STEEP).unwrap_err();
    assert!(e.to_string().contains("max_height_m"), "{e}");
}

// spec: TEST-015
#[test]
fn steep_vertices_project_within_a_quarter_pixel_at_one_metre() {
    // The highest steep terrain among random directions. The tile is the leaf that selection actually picks under a
    // camera 1 m above that point (production-like 1080p parameters), not a finest-level tile: its larger offsets are the
    // f32 worst case. The camera moves in 0.1 mm steps, through the shader path in f32 (origin_rel + local offset, morph
    // mix, f32 matrix), against the exact f64 projection.
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
    let ground = best.1 * (R + best.0);
    let lod = LodParams { cells: 8, viewport_h_px: h, tau_px: 6.0, max_level: 26, ..LodParams::earth_1080p() };
    let probe = Camera::at_surface_point(
        PlanetFixed(ground + best.1 * 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        45f64.to_radians(),
        60f64.to_radians(),
        w / h,
        0.1,
    );
    let req =
        TerrainRequest { camera: probe, size: (1920, 1080), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 };
    let plan = plan_terrain_field(&req, &STEEP).unwrap();
    let under = TileId::from_direction(&map, best.1, 26);
    let contains = |t: TileId| std::iter::successors(Some(under), |x| x.parent()).any(|x| x == t);
    let leaf = plan.nodes.iter().map(|d| d.tile).find(|t| contains(*t)).expect("a leaf under the camera");
    let mesh = tile_mesh_field(&STEEP, &map, leaf, 8, R);
    // The grid vertex nearest the camera's ground point and two neighbours (plain, half- and fully-morphed).
    let (mut ni, mut nj, mut nd) = (0usize, 0usize, f64::MAX);
    for j in 0..=8usize {
        for i in 0..=8usize {
            let v = mesh.vertices[j * 9 + i];
            let p = mesh.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]));
            if p.distance(ground) < nd {
                (ni, nj, nd) = (i, j, p.distance(ground));
            }
        }
    }
    let (ni, nj) = (ni.clamp(0, 7), nj.clamp(0, 7));
    eprintln!(
        "jitter probe: leaf level {} (cell {:.1} m)",
        leaf.level(),
        R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << leaf.level()) / 8.0
    );
    let mut worst = 0.0f64;
    for (vi, vj, m) in [(ni, nj, 0.0f32), (ni + 1, nj, 0.5), (ni + 1, nj + 1, 1.0)] {
        let v = mesh.vertices[vj * 9 + vi];
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
    eprintln!("steep terrain height {:.0} m, leaf level {}: worst jitter {worst} px", best.0, leaf.level());
    assert!(best.0 > 4000.0, "the test must exercise high steep terrain, found only {:.0} m", best.0);
    assert!(worst < 0.25, "projected position jitter {worst} px at 1 m on steep terrain; budget 0.25 px (CON-13)");
}
