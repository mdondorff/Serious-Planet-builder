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
    let mut scale = 1.0f32;
    for _ in 0..40 {
        let d = 0.1 * scale;
        scale *= 1.5;
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
            worst = worst.max(((gx - ex) * (gx - ex) + (gy - ey) * (gy - ey)).sqrt());
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

// spec: REND-013
#[test]
fn every_planned_draw_expects_the_mesh_of_its_own_seed() {
    use planet_generators::mesh::mesh_key;
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[3];
    let plan_for = |seed: u64| {
        plan_terrain(&TerrainRequest {
            camera,
            size: (128, 96),
            view: TerrainView::Level,
            lod: lod(),
            face_mapping: "tangent-v1".into(),
            seed,
        })
        .unwrap()
    };
    let (one, two) = (plan_for(1), plan_for(2));
    assert!(!one.nodes.is_empty());
    for (seed, plan) in [(1, &one), (2, &two)] {
        for n in &plan.nodes {
            assert_eq!(
                n.mesh_key,
                mesh_key(&TangentWarp, seed, n.tile, plan.cells, R),
                "{:?}: key does not follow the seed {seed}",
                n.tile
            );
        }
    }
    let shared =
        one.nodes.iter().filter_map(|a| two.nodes.iter().find(|b| b.tile == a.tile).map(|b| (a, b))).next().expect("a common tile");
    assert_ne!(shared.0.mesh_key, shared.1.mesh_key, "the same tile must expect different meshes for different seeds");
}

fn plan_of(index: usize, view: TerrainView, seed: u64) -> planet_frame::TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, seed)[index];
    plan_terrain(&TerrainRequest { camera, size: (128, 96), view, lod: lod(), face_mapping: "tangent-v1".into(), seed }).unwrap()
}

fn check_snapshot(file: &str, text: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(file);
    if std::env::var("PLANET_UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        std::fs::write(&path, text).unwrap();
    }
    let expected =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{file} missing; run with PLANET_UPDATE_SNAPSHOTS=1 and review the diff"));
    if text != expected {
        let diff: Vec<String> = text
            .lines()
            .zip(expected.lines())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(6)
            .map(|(i, (a, b))| format!("  line {}: now `{a}` / snapshot `{b}`", i + 1))
            .collect();
        panic!(
            "{file} differs from the committed snapshot ({} vs {} lines):\n{}",
            text.lines().count(),
            expected.lines().count(),
            diff.join("\n")
        );
    }
}

// spec: TEST-009, TEST-008
#[test]
fn the_terrain_plan_text_form_round_trips_exactly() {
    for index in [0usize, 3, 5, 8] {
        let plan = plan_of(index, TerrainView::Level, 1);
        let back = planet_frame::TerrainPlan::from_text(&plan.to_text()).unwrap();
        assert_eq!(back, plan, "view {index}: plan changed in a text round trip");
        assert_eq!(back.plan_hash(), plan.plan_hash());
        assert!(back.diff(&plan).is_empty());
    }
    let e = planet_frame::TerrainPlan::from_text("planet-terrainplan 9\n").unwrap_err();
    assert!(e.contains("unsupported"), "{e}");
    assert!(planet_frame::TerrainPlan::from_text("planet-terrainplan 1\nsize 1 1\nbogus\n").unwrap_err().contains("cannot parse"));
}

// spec: TEST-009, TEST-008
#[test]
fn every_plan_field_changes_the_text_form() {
    let base = plan_of(3, TerrainView::Level, 1);
    let h = base.plan_hash();
    let mut variants: Vec<(&str, planet_frame::TerrainPlan)> = Vec::new();
    let mut v = base.clone();
    v.size.0 += 1;
    variants.push(("size", v));
    let mut v = base.clone();
    v.view = TerrainView::Depth;
    variants.push(("view", v));
    let mut v = base.clone();
    v.cells += 1;
    variants.push(("cells", v));
    let mut v = base.clone();
    v.clear_color[0] ^= 1;
    variants.push(("clear_color", v));
    let mut v = base.clone();
    v.height_range[1] += 1.0;
    variants.push(("height_range", v));
    let mut v = base.clone();
    v.view_projection[5] = f32::from_bits(v.view_projection[5].to_bits() ^ 1);
    variants.push(("view_projection (one ulp)", v));
    let mut v = base.clone();
    v.camera.position.0.x = f64::from_bits(v.camera.position.0.x.to_bits() ^ 1);
    variants.push(("camera position (one f64 ulp)", v));
    let mut v = base.clone();
    v.camera.near_m *= 2.0;
    variants.push(("camera near", v));
    let mut v = base.clone();
    v.nodes[0].color[1] ^= 1;
    variants.push(("node colour", v));
    let mut v = base.clone();
    v.nodes[0].mesh_key ^= 1;
    variants.push(("node mesh_key", v));
    let mut v = base.clone();
    v.nodes[0].morph_end_m += 1.0;
    variants.push(("node morph interval", v));
    let mut v = base.clone();
    v.nodes[0].origin.0.z = f64::from_bits(v.nodes[0].origin.0.z.to_bits() ^ 1);
    variants.push(("node origin (one f64 ulp)", v));
    let mut v = base.clone();
    v.nodes.pop();
    variants.push(("node count", v));
    for (what, plan) in variants {
        assert_ne!(plan.plan_hash(), h, "the text form ignores the {what}");
        assert!(!plan.diff(&base).is_empty(), "diff does not report the {what}");
    }
}

// spec: TEST-009, TEST-008
#[test]
fn terrain_plans_match_the_committed_snapshots() {
    // Every field of every plan over the ten scripted views is covered by the hash of its text form; one small plan is
    // committed in full so a change can be reviewed line by line.
    let mut summary = String::new();
    for (i, (name, _)) in scripted_views(R, 4.0 / 3.0, 1).iter().enumerate() {
        let p = plan_of(i, TerrainView::Level, 1);
        summary += &format!("{name}: {} nodes, plan {:016x}\n", p.nodes.len(), p.plan_hash());
    }
    // The face view colours nodes by cube face and the depth view differs only in the view field: cover both.
    for (i, view) in [(0usize, TerrainView::Face), (3, TerrainView::Face), (8, TerrainView::Depth)] {
        let p = plan_of(i, view, 1);
        summary +=
            &format!("{} [{}]: {} nodes, plan {:016x}\n", scripted_views(R, 4.0 / 3.0, 1)[i].0, view.name(), p.nodes.len(), p.plan_hash());
    }
    check_snapshot("terrain_plans.txt", &summary);
    check_snapshot("terrain_plan_orbit.txt", &plan_of(0, TerrainView::Level, 1).to_text());
}

// spec: REND-030
#[test]
fn every_node_carries_the_least_shore_ramp_of_its_level_and_the_text_form_keeps_it() {
    let p = plan_of(4, TerrainView::Level, 1);
    assert!(!p.nodes.is_empty());
    let lod = lod();
    for n in &p.nodes {
        assert_eq!(n.shore_ramp_min_m, lod.shore_ramp_min_m(n.level) as f32, "level {}", n.level);
    }
    // The text carries it (`shore_ramp`), the round trip keeps it, and a recorded plan without the token reads as 0.
    let text = p.to_text();
    assert!(text.lines().filter(|l| l.starts_with("node ")).all(|l| l.contains(" shore_ramp ")));
    assert_eq!(planet_frame::TerrainPlan::from_text(&text).unwrap(), p);
    let old: String = text
        .lines()
        .map(|l| if l.starts_with("node ") { l.split(" shore_ramp ").next().unwrap().to_string() } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = planet_frame::TerrainPlan::from_text(&old).unwrap();
    assert!(parsed.nodes.iter().all(|n| n.shore_ramp_min_m == 0.0), "a plan recorded before the field ramps by the look's blend alone");
}

// spec: TEST-009, TEST-008
#[test]
fn a_terrain_bundle_round_trips_and_detects_corruption() {
    use planet_frame::TerrainBundle;
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[4];
    let request =
        TerrainRequest { camera, size: (128, 96), view: TerrainView::Morph, lod: lod(), face_mapping: "tangent-v1".into(), seed: 7 };
    let plan = plan_terrain(&request).unwrap();
    let bundle = TerrainBundle {
        generator_version: 2,
        implementation_id: "cpu-ref-height-v2".into(),
        mesh_version: planet_generators::mesh::MESH_VERSION,
        definition_hash: 0,
        adapter: "test-adapter".into(),
        world: None,
        request: request.clone(),
        plan: plan.clone(),
        frame: None,
    };
    let text = bundle.to_text();
    assert_eq!(TerrainBundle::from_text(&text).unwrap(), bundle);
    // The recorded request replans to the recorded plan (the replay contract).
    assert!(plan_terrain(&bundle.request).unwrap().diff(&bundle.plan).is_empty());
    // A changed value inside the plan is caught by the recorded hash.
    let corrupt = text.replacen(
        "
cells 16
",
        "
cells 17
",
        1,
    );
    assert_ne!(corrupt, text);
    assert!(TerrainBundle::from_text(&corrupt).unwrap_err().contains("corrupt"), "corruption must be detected");
    // An unparsable value is reported, too.
    assert!(TerrainBundle::from_text(&text.replacen("mesh_key ", "mesh_key 1", 1)).is_err());
    assert!(TerrainBundle::from_text("planet-terrain-repro 9\n---\n").unwrap_err().contains("unsupported"));
}

// spec: TEST-008, LOD-011
#[test]
fn a_bundle_records_the_render_floor_and_one_without_min_level_replays_as_floor_0() {
    use planet_frame::TerrainBundle;
    // The orbit view, whose plan depends on the floor (14 nodes at floor 0, 181 at floor 3 at this size).
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[0];
    let bundle_of = |floor: u8| {
        let request = TerrainRequest {
            camera,
            size: (128, 96),
            view: TerrainView::Level,
            lod: LodParams { min_level: floor, ..lod() },
            face_mapping: "tangent-v1".into(),
            seed: 7,
        };
        let plan = plan_terrain(&request).unwrap();
        TerrainBundle {
            generator_version: 2,
            implementation_id: "cpu-ref-height-v2".into(),
            mesh_version: planet_generators::mesh::MESH_VERSION,
            definition_hash: 0,
            adapter: "test-adapter".into(),
            world: None,
            request,
            plan,
            frame: None,
        }
    };
    for floor in [3u8, 0] {
        let bundle = bundle_of(floor);
        let text = bundle.to_text();
        assert!(text.contains(&format!("max_level 26 min_level {floor} morph_band")), "the lod line names the floor: {text}");
        let back = TerrainBundle::from_text(&text).unwrap();
        assert_eq!(back.request.lod.min_level, floor);
        assert_eq!(back, bundle, "floor {floor}: the bundle round trips");
        assert!(plan_terrain(&back.request).unwrap().diff(&back.plan).is_empty(), "floor {floor}: the replayed plan is the recorded one");
    }
    // A bundle written before the floor has no `min_level`: it replays as floor 0, the plan it recorded.
    let old = bundle_of(0).to_text().replacen(" min_level 0", "", 1);
    assert!(!old.contains("min_level"));
    let back = TerrainBundle::from_text(&old).unwrap();
    assert_eq!(back.request.lod.min_level, 0);
    assert!(plan_terrain(&back.request).unwrap().diff(&back.plan).is_empty(), "replays as the floor-0 plan it recorded");
    // The same text of a floor-3 plan without the token replays as floor 0 and so reports the difference in numbers.
    let stripped = bundle_of(3).to_text().replacen(" min_level 3", "", 1);
    let back = TerrainBundle::from_text(&stripped).unwrap();
    assert_eq!(back.request.lod.min_level, 0);
    assert!(!plan_terrain(&back.request).unwrap().diff(&back.plan).is_empty());
}

// spec: TEST-008
#[test]
fn a_world_bundle_records_the_definition_and_bake_and_a_version_one_bundle_still_reads() {
    use planet_frame::{BundleWorld, TerrainBundle};
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[4];
    let request =
        TerrainRequest { camera, size: (128, 96), view: TerrainView::Level, lod: lod(), face_mapping: "tangent-v1".into(), seed: 7 };
    let plan = plan_terrain(&request).unwrap();
    let world = BundleWorld {
        height_field: "cpu-erosion-v1".into(),
        global_res: 256,
        definition_toml: "schema_version = 1\n[body.0]\nradius_m = 6371000.0\n[world]\nseed = 5\n".into(),
    };
    let bundle = TerrainBundle {
        generator_version: 2,
        implementation_id: "cpu-ref-height-v2".into(),
        mesh_version: planet_generators::mesh::MESH_VERSION,
        definition_hash: 0xabc,
        adapter: "test-adapter".into(),
        world: Some(world.clone()),
        request,
        plan,
        frame: None,
    };
    let text = bundle.to_text();
    assert!(text.starts_with("planet-terrain-repro 2\n"), "{}", text.lines().next().unwrap());
    assert!(text.contains("height_field cpu-erosion-v1\nglobal_res 256\n"), "{text}");
    let back = TerrainBundle::from_text(&text).unwrap();
    assert_eq!(back.world.as_ref(), Some(&world), "the definition text survives with its newlines");
    assert_eq!(back, bundle);
    // A version-1 bundle (before worlds) is the seed-only reference field: it reads, with no world.
    let seed_only = TerrainBundle { world: None, ..bundle.clone() };
    let v1 = seed_only.to_text().replacen("planet-terrain-repro 2", "planet-terrain-repro 1", 1);
    assert_eq!(TerrainBundle::from_text(&v1).unwrap(), seed_only);
    // A version-1 bundle cannot carry a world, and a half-recorded world is refused.
    let v1_world = text.replacen("planet-terrain-repro 2", "planet-terrain-repro 1", 1);
    assert!(TerrainBundle::from_text(&v1_world).unwrap_err().contains("version 1"));
    let half = text.replacen("global_res 256\n", "", 1);
    assert!(TerrainBundle::from_text(&half).unwrap_err().contains("world"));
}

// spec: TEST-011
#[test]
fn plan_counts_stay_within_the_budget_over_the_scripted_views() {
    use planet_frame::CountBudget;
    // Production-like parameters at 1440p: cells 32, tau 6 px, the ten scripted views from orbit to 1 m.
    let size = (2560u32, 1440u32);
    let lod = LodParams {
        cells: 32,
        viewport_h_px: f64::from(size.1),
        tau_px: 6.0,
        max_level: planet_frame::production_finest_level(32),
        ..LodParams::earth_1080p()
    };
    let mut worst = 0;
    for (name, camera) in scripted_views(R, f64::from(size.0) / f64::from(size.1), 1) {
        let plan = plan_terrain(&TerrainRequest {
            camera,
            size,
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        })
        .unwrap();
        let c = plan.counts();
        // The counts follow from the plan: one draw call per node, equal triangle counts per node, one buffer per tile.
        assert_eq!(c.draw_calls as usize, plan.nodes.len(), "{name}");
        // Per-tile constants at cells = 32 (the budget below is derived from them): 2 x 32^2 grid triangles plus 4 skirt strips of
        // 32 quads; the upload of a tile is its compact slot (GEN-025, 9,168 B at cells 32).
        assert_eq!(c.triangles, plan.nodes.len() as u64 * 2_304, "{name}");
        assert_eq!(c.upload_bytes, u64::from(c.resident_tiles) * planet_generators::compact::slot_bytes(32), "{name}");
        assert_eq!(c.resident_tiles as usize, plan.nodes.len(), "{name}: every node of a plan is a distinct tile");
        c.check(&CountBudget::REFERENCE_1440P).unwrap_or_else(|e| panic!("{name}: {e}"));
        worst = worst.max(c.draw_calls);
    }
    assert!(worst > 1_000, "the workload must reach a high node count to mean something (worst {worst})");
    // A budget that is exceeded names the count and the budget.
    let tight = CountBudget { max_draw_calls: 10, ..CountBudget::REFERENCE_1440P };
    let (_, camera) = scripted_views(R, f64::from(size.0) / f64::from(size.1), 1)[6];
    let plan =
        plan_terrain(&TerrainRequest { camera, size, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap();
    let e = plan.counts().check(&tight).unwrap_err();
    assert!(e.contains("draw calls") && e.contains("> budget 10"), "{e}");
}

/// The node count of each standard view over the seed-only reference field (the field the goldens keep, GEN-018) at the
/// production-like parameters of the test above, pinned; the default world's (eroded field) pins are in
/// `crates/app/tests/world_view_pins.rs`. The other counts
/// follow from it (2,304 triangles and 1,221 x 48 GPU vertex bytes per tile). Raising a pin is a decision: say why in the
/// change that does it (CON-20); a view that drops well below its pin should have the pin lowered with it.
/// Re-pinned 2026-10-15 by `tight-frustum-cut` (LOD-007): the frustum cut uses each tile's own height bounds, not the global
/// 9 km pad, so every view from aerial-200km down draws fewer nodes (ground-1m 8,687 to 2,897). high-2000km rises 424 to 428:
/// the first test now also pads the skirts, which hang 12 km and more below the coarse tiles and exceeded the old 9 km pad.
/// Re-pinned 2026-10-15 by `finest-level-cap` (LOD-010, declared: finest level 22): the standard views are planned at the production
/// finest level (`production_finest_level(32)` = 22), which only ground-1m reaches: 2,897 to 2,378 nodes (-17.9 %); every other view is
/// unchanged. The level-26 reference is pinned beside (`REFERENCE_L26_GROUND_1M`).
/// Re-pinned 2026-10-18 by `render-floor-3` (LOD-011, ADR 0062 point 3, declared): orbit-20000km draws the 181 level-3 tiles of the
/// whole visible hemisphere instead of 52 level-2 tiles (+129 nodes, SPIKE-12 Q5); the other nine views draw the nodes they drew at
/// floor 0 (`the_floor_changes_the_orbit_view_only_...` in `floor.rs`).
const VIEW_NODE_PINS: [(&str, u32); 10] = [
    ("orbit-20000km", 181),
    ("high-2000km", 428),
    ("aerial-200km", 630),
    ("aerial-20km", 892),
    ("low-2km", 1279),
    ("ground-200m", 1694),
    ("ground-1m", 2378),
    ("north-pole-1km", 1180),
    ("cube-corner-1km", 1606),
    ("face-edge-1km", 1574),
];

// spec: TEST-011
#[test]
fn every_standard_view_stays_within_its_own_pinned_counts() {
    use planet_frame::CountBudget;
    let size = (2560u32, 1440u32);
    let lod = LodParams {
        cells: 32,
        viewport_h_px: f64::from(size.1),
        tau_px: 6.0,
        max_level: planet_frame::production_finest_level(32),
        ..LodParams::earth_1080p()
    };
    let views = scripted_views(R, f64::from(size.0) / f64::from(size.1), 1);
    assert_eq!(views.len(), VIEW_NODE_PINS.len(), "a standard view was added or removed: pin it");
    let mut failures = Vec::new();
    for ((name, camera), (pinned_name, pin)) in views.into_iter().zip(VIEW_NODE_PINS) {
        assert_eq!(name, pinned_name, "the pins are in the order of the standard views");
        let plan = plan_terrain(&TerrainRequest {
            camera,
            size,
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        })
        .unwrap();
        let budget = CountBudget {
            max_draw_calls: pin,
            max_triangles: u64::from(pin) * 2_304,
            max_resident_tiles: pin,
            max_upload_bytes: u64::from(pin) * planet_generators::compact::slot_bytes(32),
        };
        if let Err(e) = plan.counts().check(&budget) {
            failures.push(format!("{name}: {e} (nodes {} against pin {pin})", plan.nodes.len()));
        }
        // A pin stays tight: a view more than 10 % under its pin has its pin lowered with it.
        if u64::from(pin) > plan.nodes.len() as u64 * 10 / 9 {
            failures.push(format!("{name}: {} nodes is more than 10 % under its pin {pin}: lower the pin", plan.nodes.len()));
        }
    }
    assert!(failures.is_empty(), "plan counts above their pins:\n{}", failures.join("\n"));
}

/// The ground-1m node count at the reference finest level 26 (the cap before SPIKE-07 and ADR 0033 point 6): the old pin, kept so the
/// SPIKE-07 comparisons stay reproducible with `--finest-level 26`.
const REFERENCE_L26_GROUND_1M: u32 = 2897;

// spec: TEST-011, LOD-010
#[test]
fn the_reference_finest_level_26_keeps_its_ground_1m_pin_and_only_that_view_differs() {
    let size = (2560u32, 1440u32);
    let at = |max_level| LodParams { cells: 32, viewport_h_px: f64::from(size.1), tau_px: 6.0, max_level, ..LodParams::earth_1080p() };
    let views = scripted_views(R, f64::from(size.0) / f64::from(size.1), 1);
    for ((name, camera), (_, pin)) in views.into_iter().zip(VIEW_NODE_PINS) {
        let nodes = |lod: LodParams| {
            plan_terrain(&TerrainRequest { camera, size, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 })
                .unwrap()
                .nodes
                .len() as u32
        };
        let (l22, l26) = (nodes(at(22)), nodes(at(26)));
        if name == "ground-1m" {
            assert_eq!((l22, l26), (pin, REFERENCE_L26_GROUND_1M), "ground-1m at the production cap 22 and at the reference 26");
        } else {
            assert_eq!(l22, l26, "{name}: no tile finer than 22 is drawn, so the cap changes nothing");
        }
    }
}

// spec: TEST-008
#[test]
fn a_frame_bundle_records_the_drawn_plan_with_its_stand_ins_lit_and_water_and_reads_back_equal() {
    use planet_frame::look::file::LookParams;
    use planet_frame::look::LitPlan;
    use planet_frame::pipeline::DrawKind;
    use planet_frame::water::empty_water_plan;
    use planet_frame::{DrawnFrame, TerrainBundle};
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[4];
    let request =
        TerrainRequest { camera, size: (128, 96), view: TerrainView::Level, lod: lod(), face_mapping: "tangent-v1".into(), seed: 7 };
    let request_plan = plan_terrain(&request).unwrap();
    // The frame drawn: the first node stands in for children that were not resident (a parent: its own geometry, no morph), the
    // viewer's lit part and a water part on it.
    let mut drawn = request_plan.clone();
    let parent = drawn.nodes[0].tile.parent().expect("a node below the roots has a parent");
    drawn.nodes[0].tile = parent;
    drawn.nodes[0].level = parent.level();
    drawn.nodes[0].morph_start_m = 1.0e30;
    drawn.nodes[0].morph_end_m = 1.0e30;
    drawn.lit = Some(LitPlan::fixed(14.0));
    drawn.water = Some(empty_water_plan(&LookParams::default().water));
    let frame = DrawnFrame { kind: DrawKind::Stale, substituted: 1, blank: 0 };
    let bundle = TerrainBundle {
        generator_version: 2,
        implementation_id: "cpu-ref-height-v2".into(),
        mesh_version: planet_generators::mesh::MESH_VERSION,
        definition_hash: 0,
        adapter: "test-adapter".into(),
        world: None,
        request,
        plan: drawn.clone(),
        frame: Some(frame),
    };
    let text = bundle.to_text();
    assert!(text.starts_with("planet-terrain-repro 3\n") && text.contains("\ndrawn stale substituted 1 blank 0\n"), "{text}");
    let back = TerrainBundle::from_text(&text).unwrap();
    assert_eq!(back, bundle);
    assert_eq!(back.plan.nodes[0].tile, parent, "the stand-in survives");
    assert!(back.plan.lit.is_some() && back.plan.water.is_some(), "the lit and the water part survive");
    assert_ne!(back.plan.plan_hash(), request_plan.plan_hash(), "a frame plan is not the plan of its request");
    // A corrupted plan is refused by its hash; a bundle without a frame is still written as version 2.
    let corrupt = text.replacen(&format!("node {:016x}", parent.raw()), &format!("node {:016x}", parent.parent().unwrap().raw()), 1);
    assert!(TerrainBundle::from_text(&corrupt).unwrap_err().contains("corrupt"), "a tampered plan is refused");
    let plain = TerrainBundle { frame: None, plan: request_plan, ..bundle };
    assert!(plain.to_text().starts_with("planet-terrain-repro 2\n"));
    assert!(TerrainBundle::from_text(&plain.to_text()).unwrap().frame.is_none());
    // Version 3 text with an unknown frame kind is refused naming it.
    let odd = text.replacen("drawn stale", "drawn sideways", 1);
    assert!(TerrainBundle::from_text(&odd).unwrap_err().contains("sideways"));
}
