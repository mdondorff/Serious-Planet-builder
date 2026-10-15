//! LOD-008: selection and morph see the terrain height, so the camera is never inside the drawn surface.
//!
//! The drawn surface is rebuilt on the CPU the way the shader draws it: the leaf's mesh, each vertex morphed towards the
//! parent grid by its distance to the camera and the plan's interval.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{
    plan_terrain_field, scripted_views_field, select_nodes_field_cached, LodParams, SelectedNode, SelectionCache, TerrainRequest,
    TerrainView,
};
use planet_generators::height::{BodyId, HeightField, Reference};
use planet_generators::mesh::{grid_indices, tile_mesh_field};
use planet_generators::stress::Steep;

const R: f64 = 6_371_000.0;
const NEAR: f64 = 0.1;

fn reference_params() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

/// The steep field's relief at tile scale needs a larger ratio of split distance to tile size than the reference
/// field's, or no crack-free morph interval exists (the plan rejects such parameters).
fn steep_params() -> LodParams {
    LodParams { cells: 4, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

/// The leaf's grid vertices as the shader draws them (morphed by their distance to the camera), camera-relative, f64.
fn drawn_vertices(field: &dyn HeightField, d: &SelectedNode, cam: PlanetFixed, cells: u32) -> Vec<Vec3> {
    let mesh = tile_mesh_field(field, &TangentWarp, d.tile, cells, R);
    // The plan draws these distances as f32.
    let (start, end) = (f64::from(d.morph_start_m as f32), f64::from(d.morph_end_m as f32));
    mesh.vertices
        .iter()
        .map(|v| {
            let at = |p: [f32; 3]| mesh.origin.0 + Vec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
            let (fine, coarse) = (at(v.pos), at(v.coarse));
            let dist = (fine - cam.0).length();
            let m = ((dist - start) / (end - start).max(1e-6)).clamp(0.0, 1.0);
            fine * (1.0 - m) + coarse * m - cam.0
        })
        .collect()
}

/// Closest point of triangle `abc` to the origin (Ericson, Real-Time Collision Detection 5.1.5), as a distance.
fn origin_distance_to_triangle(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let (ab, ac) = (b - a, c - a);
    let (d1, d2) = (ab.dot(-a), ac.dot(-a));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a.length();
    }
    let bp = -b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b.length();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (a + ab * (d1 / (d1 - d3))).length();
    }
    let cp = -c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c.length();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (a + ac * (d2 / (d2 - d6))).length();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return (b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)))).length();
    }
    let denom = 1.0 / (va + vb + vc);
    (a + ab * (vb * denom) + ac * (vc * denom)).length()
}

/// Smallest distance from the camera (the origin) to the drawn surface of one leaf.
fn distance_to_drawn_surface(drawn: &[Vec3], cells: u32) -> f64 {
    grid_indices(cells)
        .chunks(3)
        .map(|t| origin_distance_to_triangle(drawn[t[0] as usize], drawn[t[1] as usize], drawn[t[2] as usize]))
        .fold(f64::MAX, f64::min)
}

/// Greatest `s` (metres along `up` from the camera) at which the radial line through the camera crosses the drawn leaf.
fn drawn_surface_offset(drawn: &[Vec3], cam: PlanetFixed, cells: u32) -> Option<f64> {
    let up = cam.0.normalized();
    let mut best: Option<f64> = None;
    for tri in grid_indices(cells).chunks(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| drawn[i as usize]);
        let normal = (b - a).cross(c - a);
        let denom = normal.dot(up);
        if denom.abs() < 1e-30 {
            continue;
        }
        let s = normal.dot(a) / denom;
        let p = up * s;
        // Barycentric test of the hit point.
        let (v0, v1, v2) = (b - a, c - a, p - a);
        let (d00, d01, d11, d20, d21) = (v0.dot(v0), v0.dot(v1), v1.dot(v1), v2.dot(v0), v2.dot(v1));
        let den = d00 * d11 - d01 * d01;
        if den.abs() < 1e-30 {
            continue;
        }
        let (u, w) = ((d11 * d20 - d01 * d21) / den, (d00 * d21 - d01 * d20) / den);
        // Views on a tile border put the radial line exactly on a shared edge.
        let eps = 1e-6;
        if u >= -eps && w >= -eps && u + w <= 1.0 + eps {
            best = Some(best.map_or(s, |x: f64| x.max(s)));
        }
    }
    best
}

fn check_field(field: &dyn HeightField, label: &str, params: LodParams) {
    let map = TangentWarp;
    let mut failures = Vec::new();
    for (name, cam) in scripted_views_field(R, 4.0 / 3.0, field) {
        let req = TerrainRequest {
            camera: cam,
            size: (128, 96),
            view: TerrainView::Level,
            lod: params.clone(),
            face_mapping: "tangent-v1".into(),
            seed: field.seed(),
        };
        let plan = plan_terrain_field(&req, field).unwrap();
        // The invariants concern the surface the selection puts under and around the camera, whether or not the frustum
        // keeps it in the draw set: the tight cut (LOD-007) drops a nadir leaf that looks away from the image, so the leaves
        // come from the selection without the frustum, and a leaf the plan lacks must lie wholly outside the frustum.
        let selection = select_nodes_field_cached(&map, cam.position, field, &params, &mut SelectionCache::new());
        let under = TileId::from_direction(&map, cam.position.0.normalized(), 26);
        let contains = |t: TileId| std::iter::successors(Some(under), |x| x.parent()).any(|x| x == t);
        let Some(found) = selection.nodes.iter().find(|d| contains(d.tile)) else {
            // Looking obliquely from high up, the horizon may leave the nadir out; near the ground it must not.
            let altitude = cam.position.0.length() - R;
            if altitude < 10_000.0 {
                failures.push(format!("{label} {name}: no leaf under the camera"));
            }
            continue;
        };
        // A leaf the plan draws is judged with the plan's own morph interval (equalised over the drawn leaves of its level).
        let mut leaf = *found;
        let in_plan = plan.nodes.iter().find(|n| n.tile == found.tile);
        if let Some(n) = in_plan {
            leaf.morph_start_m = f64::from(n.morph_start_m);
            leaf.morph_end_m = f64::from(n.morph_end_m);
        }
        let d = &leaf;
        if in_plan.is_none() {
            let frustum = cam.frustum();
            let mesh = tile_mesh_field(field, &TangentWarp, d.tile, params.cells, R);
            let seen = mesh
                .vertices
                .iter()
                .any(|v| frustum.contains_point(mesh.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]))));
            if seen {
                failures.push(format!("{label} {name}: leaf {:?} under the camera is missing from the plan with a vertex in view", d.tile));
            }
        }
        let drawn = drawn_vertices(field, d, cam.position, params.cells);
        let Some(s) = drawn_surface_offset(&drawn, cam.position, params.cells) else {
            failures.push(format!("{label} {name}: the radial line through the camera misses leaf {:?}", d.tile));
            continue;
        };
        // Height of the camera above the exact terrain, and the projected sample spacing of the leaf there.
        let dir = cam.position.0.normalized();
        let above_terrain = cam.position.0.length() - (R + field.height(dir));
        let spacing = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << d.tile.level()) / f64::from(params.cells);
        let px = spacing * params.viewport_h_px / (2.0 * libm::tan(60f64.to_radians() / 2.0) * above_terrain.max(NEAR));
        eprintln!(
            "{label} {name}: leaf level {}, drawn surface {:.3} m below the camera, {above_terrain:.1} m above terrain, {px:.2} px",
            d.tile.level(),
            -s
        );
        if s > -NEAR {
            failures.push(format!(
                "{label} {name}: camera is {:.3} m above the drawn surface of leaf {:?} (level {}); need at least the near plane {NEAR} m",
                -s,
                d.tile,
                d.tile.level()
            ));
        }
        // The nearest drawn surface among the leaf and every node whose volume reaches the camera: a surface that rises
        // beside the camera is as bad as one under it.
        let mut nearest = distance_to_drawn_surface(&drawn, params.cells);
        for other in selection.nodes.iter().filter(|o| o.tile != d.tile) {
            let reach = 2.0 * params.node_radius_bound(other.tile.level());
            let centre = other.tile.center_direction(&map);
            let origin = centre * (R + field.height(centre));
            if (origin - cam.position.0).length() < 1.5 * reach {
                let mut o = *other;
                if let Some(n) = plan.nodes.iter().find(|n| n.tile == other.tile) {
                    o.morph_start_m = f64::from(n.morph_start_m);
                    o.morph_end_m = f64::from(n.morph_end_m);
                }
                let v = drawn_vertices(field, &o, cam.position, params.cells);
                nearest = nearest.min(distance_to_drawn_surface(&v, params.cells));
            }
        }
        if nearest < NEAR {
            failures.push(format!(
                "{label} {name}: the drawn surface is {nearest:.3} m from the camera, inside the near plane {NEAR} m (leaf {:?}, level {})",
                d.tile,
                d.tile.level()
            ));
        }
        if d.tile.level() < params.max_level && px > params.tau_px * 1.0001 {
            failures.push(format!(
                "{label} {name}: leaf {:?} (level {}) projects its sample spacing to {px:.2} px at the camera height {above_terrain:.1} m; tau_px is {}",
                d.tile, d.tile.level(), params.tau_px
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// spec: LOD-008
#[test]
fn camera_is_above_the_drawn_surface_in_every_scripted_view_on_the_reference_field() {
    check_field(&Reference(1), "reference", reference_params());
}

// spec: LOD-008
// spec: TEST-015
#[test]
fn camera_is_above_the_drawn_surface_in_every_scripted_view_on_the_steep_field() {
    check_field(&Steep(1), "steep", steep_params());
}

// spec: LOD-008
// spec: TEST-015
#[test]
fn terrain_too_steep_for_the_parameters_is_a_plan_error_naming_the_fix() {
    // Production-like parameters (cells 32, 240 pixels of tile detail per ratio) leave the steep field's neighbourhoods no
    // crack-free morph interval at ground level: the plan must say so, not narrow the interval.
    let field = Steep(1);
    let lod = LodParams { cells: 32, viewport_h_px: 1440.0, tau_px: 6.0, max_level: 26, ..LodParams::earth_1080p() };
    let (name, cam) = scripted_views_field(R, 16.0 / 9.0, &field)[5];
    let req = TerrainRequest { camera: cam, size: (256, 144), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 };
    let e = plan_terrain_field(&req, &field).expect_err(name).to_string();
    assert!(e.contains("morph interval") && e.contains("viewport_h_px / tau_px 240") && e.contains("reduce cells"), "{e}");
    // The reference field plans the same request.
    assert!(plan_terrain_field(&req, &Reference(1)).is_ok());
}

fn above_terrain(field: &dyn HeightField, h: f64) -> PlanetFixed {
    let dir = Vec3::new(0.8, 0.5, 0.33).normalized();
    PlanetFixed(dir * (R + field.height(dir) + h))
}

/// Recede from `lo` to `hi` metres above the terrain in 3 % steps and check the leaf under the camera: morph in [0, 1],
/// levels never increase, and every merge happens from a nearly fully morphed leaf to a barely morphed one. The sweep
/// starts one step below `lo` so the first step of a chunk is compared with its predecessor: the chunks together check
/// the heights one sweep from 30 m to 200 km would (30 * 1.03^k, up to rounding), split only so the test tier runs
/// them in parallel. Returns the number of merges seen.
fn morph_sweep(field: &dyn HeightField, lo: f64, hi: f64) -> usize {
    let params = LodParams::earth_1080p();
    let nadir = above_terrain(field, 0.0).0.normalized();
    let mut prev: Option<(u8, f32)> = None;
    let (mut h, mut merges, mut step) = (lo / 1.03, 0, 0u32);
    let mut cache = planet_frame::SelectionCache::new();
    while h < hi {
        let cam = above_terrain(field, h);
        // The sweep shares one warm cache, as a flight does; every 16th step is also selected cold and must be identical.
        let sel = planet_frame::select_nodes_field_cached(&TangentWarp, cam, field, &params, &mut cache);
        if step.is_multiple_of(16) {
            assert_eq!(sel, planet_frame::select_nodes_field(&TangentWarp, cam, field, &params), "warm and cold differ at height {h}");
        }
        step += 1;
        let n = sel.nodes.iter().find(|n| n.tile.contains(&TangentWarp, nadir)).expect("a leaf contains the nadir");
        assert!((0.0..=1.0).contains(&n.morph), "morph {} at height {h}", n.morph);
        if let Some((pl, pm)) = prev {
            assert!(n.tile.level() <= pl, "level must not increase while receding (height {h})");
            if n.tile.level() < pl {
                // One level per merge, so the level span over the chunk bounds equals the number of merges (see the guard test).
                assert_eq!(n.tile.level() + 1, pl, "merge at height {h} drops more than one level");
                assert!(pm >= 0.8 && n.morph <= 0.5, "merge at height {h}: morph {pm} before, {} after", n.morph);
                merges += 1;
            }
        }
        prev = Some((n.tile.level(), n.morph));
        h *= 1.03;
    }
    merges
}

/// Ten consecutive chunks (balanced by measured cost, not by height) of the sweep from 30 m to 200 km (3 % steps): each starts where the last ended.
fn chunk_bounds(i: u32) -> (f64, f64) {
    const STEPS: [u32; 11] = [0, 15, 30, 48, 68, 92, 125, 170, 215, 255, 299];
    let at = |k: u32| (0..k).fold(30.0f64, |h, _| h * 1.03);
    (at(STEPS[i as usize]), if i == 9 { 2.0e5 } else { at(STEPS[i as usize + 1]) })
}

macro_rules! morph_sweep_tests {
    ($($name:ident: $field:expr, $chunk:expr;)*) => {$(
        // spec: LOD-004
        #[test]
        fn $name() {
            let (lo, hi) = chunk_bounds($chunk);
            morph_sweep(&$field, lo, hi);
        }
    )*};
}

morph_sweep_tests! {
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_0: Reference(1), 0;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_1: Reference(1), 1;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_2: Reference(1), 2;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_3: Reference(1), 3;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_4: Reference(1), 4;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_5: Reference(1), 5;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_6: Reference(1), 6;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_7: Reference(1), 7;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_8: Reference(1), 8;
    morph_is_bounded_and_merges_do_not_pop_reference_chunk_9: Reference(1), 9;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_0: Steep(1), 0;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_1: Steep(1), 1;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_2: Steep(1), 2;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_3: Steep(1), 3;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_4: Steep(1), 4;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_5: Steep(1), 5;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_6: Steep(1), 6;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_7: Steep(1), 7;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_8: Steep(1), 8;
    morph_is_bounded_and_merges_do_not_pop_steep_chunk_9: Steep(1), 9;
}

// spec: LOD-004
#[test]
fn the_chunked_morph_sweep_covers_many_level_changes_from_ground_to_orbit() {
    // The unchunked sweep asserted at least 8 merges per field; chunks cannot count across their bounds, so the level under
    // the camera at the chunk bounds must fall monotonically and span at least 8 levels.
    let params = LodParams::earth_1080p();
    for field in [&Reference(1) as &dyn HeightField, &Steep(1)] {
        let nadir = above_terrain(field, 0.0).0.normalized();
        let level_at = |h: f64| {
            let sel = planet_frame::select_nodes_field(&TangentWarp, above_terrain(field, h), field, &params);
            sel.nodes.iter().find(|n| n.tile.contains(&TangentWarp, nadir)).expect("a leaf contains the nadir").tile.level()
        };
        let levels: Vec<u8> = (0..10).map(|i| level_at(chunk_bounds(i).0)).chain([level_at(chunk_bounds(9).1)]).collect();
        assert!(levels.windows(2).all(|w| w[1] <= w[0]), "levels {levels:?} must not increase while receding");
        assert!(levels[0] >= levels[10] + 8, "levels {levels:?}: expected at least 8 level changes");
    }
}

// spec: LOD-008
#[test]
fn a_leafs_morph_interval_does_not_depend_on_the_camera() {
    // Static tile geometry only: otherwise a neighbour splitting would move a leaf's interval and pop its border.
    let params = LodParams::earth_1080p();
    let field = Reference(1);
    let a = planet_frame::select_nodes_field(&TangentWarp, above_terrain(&field, 300.0), &field, &params);
    let b = planet_frame::select_nodes_field(&TangentWarp, above_terrain(&field, 450.0), &field, &params);
    let common: Vec<_> = a.nodes.iter().filter_map(|x| b.nodes.iter().find(|y| y.tile == x.tile).map(|y| (x, y))).collect();
    assert!(common.len() > 50, "only {} common leaves", common.len());
    for (x, y) in common {
        assert_eq!((x.morph_start_m, x.morph_end_m), (y.morph_start_m, y.morph_end_m), "{:?}", x.tile);
    }
}

// spec: LOD-008
#[test]
fn the_reference_field_plans_at_production_parameters_along_the_whole_flight() {
    // A plan error means nothing is drawn (CON-22): the reference field must never hit the steepness rejection.
    let lod = LodParams { cells: 32, viewport_h_px: 1440.0, tau_px: 6.0, max_level: 26, ..LodParams::earth_1080p() };
    let frames = planet_frame::flight_path(R, 16.0 / 9.0, 1, 40);
    for (i, cam) in frames.into_iter().enumerate() {
        let req = TerrainRequest {
            camera: cam,
            size: (256, 144),
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        };
        plan_terrain_field(&req, &Reference(1)).unwrap_or_else(|e| panic!("flight frame {i}: {e}"));
    }
}

// spec: LOD-008
#[test]
fn same_level_neighbours_share_one_morph_interval_on_rugged_terrain() {
    // Shared border vertices morph by each tile's own formula: unequal intervals would open the border.
    let field = Steep(1);
    let params = steep_params();
    for (name, cam) in scripted_views_field(R, 4.0 / 3.0, &field) {
        let sel = planet_frame::select_nodes_in_view_field(&TangentWarp, &cam, &field, &params);
        let mut per_level: std::collections::BTreeMap<u8, (f64, f64)> = Default::default();
        for n in sel.nodes.iter().filter(|n| n.tile.level() > 0) {
            let e = per_level.entry(n.tile.level()).or_insert((n.morph_start_m, n.morph_end_m));
            assert_eq!(*e, (n.morph_start_m, n.morph_end_m), "{name}: level {} has two intervals", n.tile.level());
        }
    }
}

// spec: LOD-008
#[test]
fn the_node_radius_bound_covers_the_worst_tiles_on_every_face_and_level() {
    // The ratio peaks at cube-corner tiles and approaches sqrt(2/3) = 0.81650 from below as the level grows, so random
    // sampling cannot see a bound that is slightly too small; check the corners, edge middles and centres exactly.
    let params = LodParams::earth_1080p();
    assert!(params.node_radius_bound(20) > (2.0f64 / 3.0).sqrt());
    for face in 0..6u8 {
        for level in 0..=params.max_level {
            let last = ((1u64 << level) - 1) as u32;
            let mid = if level == 0 { 0 } else { 1u32 << (level - 1) };
            for &x in &[0, mid, last] {
                for &y in &[0, mid, last] {
                    let t = TileId::new(planet_core::Face(face), level, x, y).unwrap();
                    let (_, r) = planet_frame::lod::bounding_sphere(&TangentWarp, t, &params);
                    assert!(
                        r <= params.node_radius_bound(level),
                        "{t:?}: chord radius {r} exceeds the bound {}",
                        params.node_radius_bound(level)
                    );
                }
            }
        }
    }
}

// spec: LOD-008
#[test]
fn the_node_radius_bound_covers_the_bounding_sphere_of_every_tile() {
    let params = LodParams::earth_1080p();
    let mut rng = planet_core::hash::SplitMix64(5);
    for level in 1..=20u8 {
        for _ in 0..400 {
            let (x, y) = ((rng.next_u64() % (1u64 << level)) as u32, (rng.next_u64() % (1u64 << level)) as u32);
            let t = TileId::new(planet_core::Face((rng.next_u64() % 6) as u8), level, x, y).unwrap();
            let (_, r) = planet_frame::lod::bounding_sphere(&TangentWarp, t, &params);
            assert!(r <= params.node_radius_bound(level), "{t:?}: chord radius {r} exceeds the bound {}", params.node_radius_bound(level));
        }
    }
}

// spec: LOD-008
// spec: TEST-012
#[test]
fn a_warm_selection_cache_never_changes_a_plan() {
    // The cache holds pure per-tile work: plans from a cache that has seen many frames (and other fields) must equal
    // plans from nothing, bit for bit.
    let lod = LodParams { cells: 32, viewport_h_px: 1440.0, tau_px: 6.0, max_level: 26, ..LodParams::earth_1080p() };
    let field = Reference(1);
    let mut cache = planet_frame::SelectionCache::new();
    let (_, start) = scripted_views_field(R, 16.0 / 9.0, &field)[5];
    let mut cam = start;
    for i in 0..12 {
        cam.position = PlanetFixed(
            cam.position.0 + Vec3::new(0.0, 0.0, 0.0) + cam.position.0.normalized().cross(Vec3::new(0.0, 0.0, 1.0)).normalized() * 3.0,
        );
        let req = TerrainRequest {
            camera: cam,
            size: (256, 144),
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        };
        let warm = planet_frame::plan_terrain_cached(&req, &field, &mut cache).unwrap();
        let cold = plan_terrain_field(&req, &field).unwrap();
        assert_eq!(warm.summary(), cold.summary(), "frame {i}");
        assert_eq!(warm, cold, "frame {i}");
        if i == 6 {
            // Another field and another radius must not be served from the Reference entries.
            let steep = Steep(1);
            let other =
                TerrainRequest { seed: 1, lod: LodParams { cells: 4, viewport_h_px: 192.0, tau_px: 2.5, ..lod.clone() }, ..req.clone() };
            let w = planet_frame::plan_terrain_cached(&other, &steep, &mut cache);
            let c = plan_terrain_field(&other, &steep);
            assert_eq!(w.map(|p| p.summary()), c.map(|p| p.summary()));
        }
    }
    assert!(!cache.is_empty());
}

/// `Reference(seed)` with another body and another recorded read set: same heights, other identity.
struct Variant {
    inner: Reference,
    inputs: u64,
    body: BodyId,
}

impl HeightField for Variant {
    fn height(&self, dir: Vec3) -> f64 {
        self.inner.height(dir)
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.inner.implementation_id()
    }
    fn version(&self) -> u32 {
        self.inner.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m()
    }
    fn inputs_hash(&self) -> u64 {
        self.inputs
    }
    fn body(&self) -> BodyId {
        self.body
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        self.inner.height_bounds(centre, angular_radius)
    }
}

// spec: TEST-012
#[test]
fn the_selection_cache_starts_over_for_another_body_or_another_read_set() {
    let lod = reference_params();
    let (_, cam) = scripted_views_field(R, 4.0 / 3.0, &Reference(1))[4];
    let req = TerrainRequest { camera: cam, size: (128, 96), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 };
    let base = Variant { inner: Reference(1), inputs: Reference(1).inputs_hash(), body: BodyId::PLANET };
    let other_inputs = Variant { inner: Reference(1), inputs: 99, body: BodyId::PLANET };
    let other_body = Variant { inner: Reference(1), inputs: base.inputs, body: BodyId(1) };
    let mut cache = planet_frame::SelectionCache::new();
    planet_frame::plan_terrain_cached(&req, &base, &mut cache).unwrap();
    let after_base = cache.misses();
    planet_frame::plan_terrain_cached(&req, &base, &mut cache).unwrap();
    assert_eq!(cache.misses(), after_base, "the same identity is served from the cache");
    for (what, field) in [("read set", &other_inputs), ("body", &other_body)] {
        let before = cache.misses();
        planet_frame::plan_terrain_cached(&req, field, &mut cache).unwrap();
        assert!(cache.misses() > before, "{what}: the cache must have started over");
        planet_frame::plan_terrain_cached(&req, &base, &mut cache).unwrap();
    }
}

// spec: TEST-012
#[test]
fn every_part_of_the_selection_cache_key_invalidates_and_warm_frames_reuse() {
    let lod = LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let (_, cam) = scripted_views_field(R, 4.0 / 3.0, &Reference(1))[4];
    let request = |seed: u64, lod: &LodParams| TerrainRequest {
        camera: cam,
        size: (128, 96),
        view: TerrainView::Level,
        lod: lod.clone(),
        face_mapping: "tangent-v1".into(),
        seed,
    };
    let mut cache = planet_frame::SelectionCache::new();
    let first = request(1, &lod);
    planet_frame::plan_terrain_cached(&first, &Reference(1), &mut cache).unwrap();
    // A warm frame computes nothing new.
    let misses = cache.misses();
    assert!(misses > 100);
    planet_frame::plan_terrain_cached(&first, &Reference(1), &mut cache).unwrap();
    assert_eq!(cache.misses(), misses, "an identical plan must be served entirely from the cache");
    // Each part of the key, changed alone, gives the plan of a cold computation (and refills the cache).
    let smaller = LodParams { radius_m: 6_000_000.0, ..lod.clone() };
    let cases: [(&str, TerrainRequest, Box<dyn HeightField>); 3] = [
        ("seed", request(2, &lod), Box::new(Reference(2))),
        ("radius", request(1, &smaller), Box::new(Reference(1))),
        ("implementation", request(1, &lod), Box::new(Steep(1))),
    ];
    for (what, req, field) in cases {
        let before = cache.misses();
        let warm = planet_frame::plan_terrain_cached(&req, field.as_ref(), &mut cache);
        let cold = plan_terrain_field(&req, field.as_ref());
        assert!(cache.misses() > before, "{what}: the cache must have started over");
        assert_eq!(warm.map(|p| p.summary()), cold.map(|p| p.summary()), "{what}");
        // Back to the first key: also a fresh start, never entries of the other key.
        planet_frame::plan_terrain_cached(&first, &Reference(1), &mut cache).unwrap();
    }
}
