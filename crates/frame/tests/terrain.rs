//! Camera, reversed-Z projection, frustum selection, the terrain plan, and precision at the far side of the planet.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{plan_terrain, scripted_views, select_nodes, select_nodes_in_view, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::mesh::tile_mesh;

const R: f64 = 6_371_000.0;

fn lod() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

/// Evaluate `view_projection * (p, 1)` in f32 exactly as a shader does (column-major, row-by-row dot products).
fn clip_f32(m: &[f32; 16], p: [f32; 3]) -> [f32; 4] {
    let row = |r: usize| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r];
    [row(0), row(1), row(2), row(3)]
}

// spec: REND-003, LOD-007
#[test]
fn projection_is_reversed_z_with_infinite_far() {
    let cam = Camera {
        position: PlanetFixed(Vec3::ZERO),
        forward: Vec3::new(0.0, 0.0, -1.0),
        up: Vec3::new(0.0, 1.0, 0.0),
        fov_y_rad: 1.0,
        aspect: 1.5,
        near_m: 0.1,
    };
    let m = cam.view_projection();
    let depth = |d: f32| {
        let c = clip_f32(&m, [0.0, 0.0, -d]);
        c[2] / c[3]
    };
    assert!((depth(0.1) - 1.0).abs() < 1e-6, "the near plane maps to 1");
    assert!((depth(1.0) - 0.1).abs() < 1e-6);
    assert!(depth(1.0e7) > 0.0 && depth(1.0e7) < 1e-7, "very far points approach 0 but stay positive: {}", depth(1.0e7));
    let mut prev = f32::MAX;
    for k in 0..40 {
        let d = 0.1 * 1.5f32.powi(k);
        assert!(depth(d) < prev, "depth must decrease with distance (reversed-Z), failed at {d}");
        prev = depth(d);
    }
    // x/y follow the field of view: a point at the top edge of the view lands at ndc y = 1.
    let t = libm::tan(0.5f64);
    let c = clip_f32(&m, [0.0, (t * 10.0) as f32, -10.0]);
    assert!((c[1] / c[3] - 1.0).abs() < 1e-5);
}

// spec: REND-004
#[test]
fn far_side_vertices_project_within_a_quarter_pixel_at_one_metre() {
    // Vertices at the highest terrain found among random directions (largest origin and offset magnitudes), with the
    // camera 1 m away moved in 0.1 mm steps; a plain vertex, a half-morphed one and a fully morphed one.
    let map = TangentWarp;
    let mut rng = planet_core::hash::SplitMix64(31);
    let mut best = (f64::MIN, Vec3::new(1.0, 0.0, 0.0));
    for _ in 0..4000 {
        let d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized();
        let h = planet_generators::height::height_at(1, d);
        if h > best.0 {
            best = (h, d);
        }
    }
    let (w, h) = (1920.0f64, 1080.0f64);
    let mut worst = 0.0f64;
    let tile = TileId::from_direction(&map, best.1, 26);
    let mesh = tile_mesh(1, &map, tile, 16, R);
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
            // Exactly the shader path: origin_rel (f64 difference -> f32) + local offset (f32), mix in f32, then the f32 matrix.
            let rel = [
                (mesh.origin.0.x - cam.position.0.x) as f32,
                (mesh.origin.0.y - cam.position.0.y) as f32,
                (mesh.origin.0.z - cam.position.0.z) as f32,
            ];
            let fine = [rel[0] + v.pos[0], rel[1] + v.pos[1], rel[2] + v.pos[2]];
            let coarse = [rel[0] + v.coarse[0], rel[1] + v.coarse[1], rel[2] + v.coarse[2]];
            let p = [0, 1, 2].map(|k| fine[k] * (1.0 - m) + coarse[k] * m);
            let c = clip_f32(&cam.view_projection(), p);
            let exact = cam.project(exact_world - cam.position.0);
            let px = |x: f64, wv: f64, size: f64| x / wv * size / 2.0;
            let (gx, gy) = (px(f64::from(c[0]), f64::from(c[3]), w), px(f64::from(c[1]), f64::from(c[3]), h));
            let (ex, ey) = (px(exact[0], exact[3], w), px(exact[1], exact[3], h));
            worst = worst.max(((gx - ex).powi(2) + (gy - ey).powi(2)).sqrt());
        }
    }
    eprintln!("terrain height {:.0} m: worst jitter {worst} px", best.0);
    assert!(best.0 > 1500.0, "the test must exercise high terrain, found only {:.0} m", best.0);
    assert!(worst < 0.25, "projected position jitter {worst} px at 1 m on high far-side terrain; budget 0.25 px");
}

// spec: LOD-001, LOD-007
#[test]
fn frustum_selection_is_a_subset_of_the_hemisphere_and_covers_the_view() {
    let m = TangentWarp;
    let params = lod();
    for (name, cam) in scripted_views(R, 4.0 / 3.0, 1).into_iter().skip(1) {
        let all = select_nodes(&m, cam.position, &params);
        let seen = select_nodes_in_view(&m, &cam, &params);
        assert!(seen.nodes.len() < all.nodes.len(), "{name}: the frustum must remove nodes ({} vs {})", seen.nodes.len(), all.nodes.len());
        // Every direction inside the frustum on the reference sphere that is also over the horizon is held by one leaf.
        let f = cam.forward.normalized();
        let r = f.cross(cam.up).normalized();
        let u = r.cross(f);
        let ty = libm::tan(cam.fov_y_rad / 2.0);
        let mut rng = planet_core::hash::SplitMix64(5);
        let mut tested = 0;
        for _ in 0..4000 {
            // A ray through a random point of the image (slightly inside the edges), intersected with the reference sphere.
            let (x, y) = (rng.range(-0.97, 0.97), rng.range(-0.97, 0.97));
            let dir = (f + r * (x * ty * cam.aspect) + u * (y * ty)).normalized();
            let o = cam.position.0;
            let b = o.dot(dir);
            let disc = b * b - (o.dot(o) - R * R);
            if disc < 0.0 {
                continue;
            }
            let t = -b - disc.sqrt();
            if t < 0.0 {
                continue;
            }
            let hit = (o + dir * t).normalized();
            let holders = seen.nodes.iter().filter(|n| n.tile.contains(&m, hit)).count();
            assert_eq!(holders, 1, "{name}: sphere point {hit:?} seen at image ({x:.2}, {y:.2}) is held by {holders} leaves");
            tested += 1;
        }
        assert!(tested > 100, "{name}: only {tested} rays reached the sphere");
    }
}

// spec: LOD-006
#[test]
fn terrain_plan_is_deterministic_and_camera_relative() {
    let views = scripted_views(R, 4.0 / 3.0, 1);
    let (name, cam) = views[4];
    let req =
        TerrainRequest { camera: cam, size: (128, 96), view: TerrainView::Level, lod: lod(), face_mapping: "tangent-v1".into(), seed: 1 };
    let a = plan_terrain(&req).unwrap();
    assert_eq!(a, plan_terrain(&req).unwrap());
    assert!(!a.nodes.is_empty(), "{name}");
    for n in &a.nodes {
        let exact = n.origin.0 - cam.position.0;
        assert_eq!(n.camera_relative_origin, [exact.x as f32, exact.y as f32, exact.z as f32]);
        assert!(n.morph_start_m <= n.morph_end_m);
        if n.level > 0 {
            let split = lod().split_distance(n.level - 1) as f32;
            assert_eq!(n.morph_end_m, split, "the morph ends exactly at the parent's split distance");
        }
    }
    let mut bad = req.clone();
    bad.face_mapping = "nope".into();
    assert!(plan_terrain(&bad).unwrap_err().to_string().contains("nope"));
}

// spec: LOD-002
#[test]
fn terrain_plan_summaries_match_the_committed_snapshot() {
    let mut text = String::new();
    for (name, cam) in scripted_views(R, 4.0 / 3.0, 1) {
        let plan = plan_terrain(&TerrainRequest {
            camera: cam,
            size: (128, 96),
            view: TerrainView::TileId,
            lod: lod(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        })
        .unwrap();
        assert!(plan.nodes.len() < 4000, "{name}: {} nodes", plan.nodes.len());
        text += &format!("{name}: {}\n", plan.summary());
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/terrain_views.txt");
    if std::env::var("PLANET_UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        std::fs::write(&path, &text).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("snapshot missing; run with PLANET_UPDATE_SNAPSHOTS=1");
    let (a, b): (Vec<&str>, Vec<&str>) = (text.lines().collect(), expected.lines().collect());
    let mut diff: Vec<String> =
        a.iter().zip(&b).filter(|(x, y)| x != y).map(|(x, y)| format!("  now:      {x}\n  snapshot: {y}")).collect();
    if a.len() != b.len() {
        diff.push(format!("  number of views changed: {} now, {} in the snapshot", a.len(), b.len()));
    }
    assert!(
        diff.is_empty(),
        "terrain plan changed for {} view(s); review and update the snapshot deliberately:\n{}",
        diff.len(),
        diff.join("\n")
    );
}
