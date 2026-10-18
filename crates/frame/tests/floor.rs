//! The render floor (LOD-011, LOD-004, ADR 0062 point 3): levels below the floor are never selected, drawn or members of R, floor
//! leaves are roots that never morph, the effective floor is the lower of the floor and the finest level, and floor 0 reproduces the
//! plans recorded before the floor existed. Tier A, no GPU.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::pipeline::{ResidentRecords, ResidentTile};
use planet_frame::{
    check_explicit_floor, cover_selection_bounded, first_empty_morph_interval, floor_tiles, plan_terrain, plan_terrain_cached,
    plan_terrain_field, production_finest_level, residency_set_field, scripted_views, scripted_views_field, Camera, LodParams,
    SelectedNode, SelectionCache, TerrainPlan, TerrainRequest, TerrainView, TileBounds, DEFAULT_RENDER_FLOOR,
};
use planet_generators::height::Reference;
use std::collections::HashMap;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);

/// The production parameters of the standard views (2560 x 1440, tau 6, 32 cells) with render floor `floor`.
fn production(floor: u8) -> LodParams {
    LodParams {
        cells: 32,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 6.0,
        max_level: production_finest_level(32),
        min_level: floor,
        ..LodParams::earth_1080p()
    }
}

/// The two plans draw the same tiles with the same mesh keys, and every node above the floor is equal in every field: only the leaves
/// at the floor differ, by having no parent to morph to (morph 0 and the interval `[1e30, 1e30]`, LOD-004).
fn assert_same_tiles_and_fields_above_the_floor(what: &str, at_floor: &TerrainPlan, at_zero: &TerrainPlan, floor: u8) {
    let key = |p: &TerrainPlan| p.nodes.iter().map(|n| (n.tile, n.mesh_key)).collect::<Vec<_>>();
    assert_eq!(key(at_floor), key(at_zero), "{what}: the same tiles");
    for (a, b) in at_floor.nodes.iter().zip(&at_zero.nodes) {
        if a.level > floor {
            assert_eq!(a, b, "{what}: {:?} above the floor", a.tile);
        } else {
            assert_eq!((a.morph, a.morph_start_m, a.morph_end_m), (0.0, 1.0e30, 1.0e30), "{what}: {:?} at the floor", a.tile);
            assert_eq!((a.origin, a.mesh_key, a.shore_ramp_min_m), (b.origin, b.mesh_key, b.shore_ramp_min_m), "{what}: {:?}", a.tile);
        }
    }
}

fn aspect() -> f64 {
    f64::from(SIZE.0) / f64::from(SIZE.1)
}

fn plan_of(camera: Camera, lod: LodParams) -> TerrainPlan {
    plan_terrain_field(
        &TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: TangentWarp.id().to_string(), seed: 1 },
        &Reference(1),
    )
    .unwrap()
}

fn views() -> Vec<(&'static str, Camera)> {
    scripted_views_field(R, aspect(), &Reference(1))
}

/// A camera 100,000 km above the planet's centre direction (0.3, 0.5, 0.8), looking at the centre.
fn far_camera() -> Camera {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    Camera::looking_at_centre(PlanetFixed(dir * (R + 1.0e8)), Vec3::new(0.0, 0.0, 1.0), 60f64.to_radians(), aspect(), 0.1)
}

// spec: LOD-011
#[test]
fn the_production_floor_is_level_3_and_has_384_roots_in_ascending_order() {
    assert_eq!(DEFAULT_RENDER_FLOOR, 3);
    assert_eq!(LodParams::earth_1080p().min_level, 3);
    let roots = floor_tiles(&production(3));
    assert_eq!(roots.len(), 384);
    assert!(roots.windows(2).all(|w| w[0] < w[1]) && roots.iter().all(|t| t.level() == 3));
    let faces = floor_tiles(&production(0));
    assert_eq!(faces.len(), 6);
    assert!(faces.iter().all(|t| t.level() == 0));
}

// spec: LOD-011
#[test]
fn levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes() {
    let mut total_other_nodes = 0usize;
    for (name, camera) in views() {
        let (floor3, floor0) = (plan_of(camera, production(3)), plan_of(camera, production(0)));
        assert!(floor3.nodes.iter().all(|n| n.level >= 3), "{name}: a node below the floor is drawn");
        if name == "orbit-20000km" {
            assert_eq!(floor0.nodes.len(), 52, "{name}: the plan recorded before the floor");
            assert_eq!(floor3.nodes.len(), 181, "{name}: the 181 level-3 nodes of the visible hemisphere");
            assert!(floor3.nodes.iter().all(|n| n.level == 3));
        } else {
            assert_same_tiles_and_fields_above_the_floor(name, &floor3, &floor0, 3);
            total_other_nodes += floor3.nodes.len();
        }
    }
    assert_eq!(total_other_nodes, 11_661, "the nine other standard views");
}

// spec: LOD-011
#[test]
fn a_camera_beyond_the_standard_orbit_draws_at_most_the_384_floor_tiles_and_r_never_holds_a_tile_above_the_floor() {
    let lod = production(3);
    let plan = plan_of(far_camera(), lod.clone());
    assert!(plan.nodes.len() <= 384, "{} nodes", plan.nodes.len());
    assert!(plan.nodes.iter().all(|n| n.level == 3));
    for (name, camera) in views().into_iter().chain([("far-100000km", far_camera())]) {
        let r = residency_set_field(&TangentWarp, camera.position, &Reference(1), &lod, &mut SelectionCache::new());
        assert!(r.tiles.iter().all(|e| e.tile.level() >= 3), "{name}: R holds a tile above the floor");
        assert!(r.is_sorted_and_closed(), "{name}: R is sorted and closed under parents down to the floor");
        // Every leaf of the selection is in R, and so is the draw set of the view.
        let plan = plan_of(camera, lod.clone());
        assert!(plan.nodes.iter().all(|n| r.contains(n.tile)), "{name}: the draw set is inside R");
        if name == "orbit-20000km" {
            assert_eq!(r.len(), 181, "R at orbit is the 181 floor leaves and no ancestor");
        }
    }
}

// spec: LOD-011, LOD-004
#[test]
fn floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs() {
    let (_, orbit) = views().into_iter().next().unwrap();
    let plan = plan_of(orbit, production(3));
    assert!(!plan.nodes.is_empty());
    for n in &plan.nodes {
        assert_eq!(n.morph, 0.0, "{:?}: a floor leaf morphs", n.tile);
        assert_eq!((n.morph_start_m, n.morph_end_m), (1.0e30f32, 1.0e30f32), "{:?}", n.tile);
    }
    // The interval [1e30, 1e30] is empty on a leaf above the floor and exempt only at the floor.
    let tile = |level: u8| TileId::new(planet_core::Face::ALL[0], level, 0, 0).unwrap();
    let leaf = |level: u8| SelectedNode { tile: tile(level), morph: 0.0, distance_m: 1.0, morph_start_m: 1.0e30, morph_end_m: 1.0e30 };
    assert!(first_empty_morph_interval(&[leaf(3)], 3).is_none(), "a leaf at the floor is exempt");
    assert!(first_empty_morph_interval(&[leaf(3), leaf(4)], 3).is_some_and(|n| n.tile.level() == 4), "a leaf above the floor is refused");
    assert!(first_empty_morph_interval(&[leaf(0)], 0).is_none() && first_empty_morph_interval(&[leaf(1)], 0).is_some());
}

// spec: LOD-004
#[test]
fn a_leaf_above_the_floor_morphs_from_its_parent_in_a_sweep_and_the_floor_leaf_never_does() {
    // Walk a camera from 20,000 km down to 2 km: floor leaves keep morph 0 and the interval at every step, the finer leaves'
    // morph never decreases as the camera recedes (checked by the existing sweeps at floor 0; here the floor's own invariant).
    let (_, orbit) = views().into_iter().next().unwrap();
    let dir = orbit.position.0.normalized();
    for altitude_km in [20_000.0, 8_000.0, 3_000.0, 1_000.0, 300.0, 100.0, 30.0] {
        let camera = Camera::looking_at_centre(
            PlanetFixed(dir * (R + altitude_km * 1000.0)),
            Vec3::new(0.0, 0.0, 1.0),
            60f64.to_radians(),
            aspect(),
            0.1,
        );
        let plan = plan_of(camera, production(3));
        for n in plan.nodes.iter().filter(|n| n.level == 3) {
            assert_eq!((n.morph, n.morph_start_m, n.morph_end_m), (0.0, 1.0e30, 1.0e30), "{altitude_km} km {:?}", n.tile);
        }
        for n in plan.nodes.iter().filter(|n| n.level > 3) {
            assert!(
                n.morph_start_m < n.morph_end_m && n.morph_end_m < 1.0e30,
                "{altitude_km} km {:?}: a finer leaf has a real interval",
                n.tile
            );
        }
    }
}

// spec: LOD-011
#[test]
fn a_finest_level_below_the_default_floor_needs_no_flag() {
    // max_level 2: the effective floor is 2, a low view draws the tiles of the floor-0 plan (every tile is drawn at level 2 anyway).
    for max_level in [0u8, 1, 2] {
        let mut lod = production(DEFAULT_RENDER_FLOOR);
        lod.max_level = max_level;
        assert_eq!(lod.floor_level(), max_level, "the effective floor is the lower of the two");
        assert_eq!(floor_tiles(&lod).len(), 6 * 4usize.pow(u32::from(max_level)));
    }
    let mut at_floor = production(DEFAULT_RENDER_FLOOR);
    at_floor.max_level = 2;
    let mut at_zero = production(0);
    at_zero.max_level = 2;
    for (name, camera) in views().into_iter().filter(|(n, _)| ["ground-200m", "low-2km", "ground-1m"].contains(n)) {
        let (a, b) = (plan_of(camera, at_floor.clone()), plan_of(camera, at_zero.clone()));
        assert!(!a.nodes.is_empty() && a.nodes.iter().all(|n| n.level == 2), "{name}");
        assert_same_tiles_and_fields_above_the_floor(name, &a, &b, 2);
    }
}

// spec: LOD-011
#[test]
fn an_explicit_floor_above_the_finest_level_is_refused_naming_both() {
    let e = check_explicit_floor(5, 4).unwrap_err();
    assert!(e.contains("--floor-level 5") && e.contains("finest level 4"), "{e}");
    assert!(check_explicit_floor(4, 4).is_ok() && check_explicit_floor(0, 0).is_ok() && check_explicit_floor(3, 22).is_ok());
}

// spec: LOD-011, TEST-008
#[test]
fn floor_zero_reproduces_the_plans_recorded_before_the_floor() {
    // The ten scripted views and the three view variants of the plan snapshot, planned at floor 0, equal what was committed before
    // render-floor-3 (`terrain_plans_floor0.txt`, the former `terrain_plans.txt`): node counts and plan hashes.
    let lod = LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, min_level: 0, ..LodParams::earth_1080p() };
    let plan_in = |index: usize, view: TerrainView| {
        let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
        plan_terrain(&TerrainRequest { camera, size: (128, 96), view, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 1 })
            .unwrap()
    };
    let mut summary = String::new();
    for (i, (name, _)) in scripted_views(R, 4.0 / 3.0, 1).iter().enumerate() {
        let p = plan_in(i, TerrainView::Level);
        summary += &format!("{name}: {} nodes, plan {:016x}\n", p.nodes.len(), p.plan_hash());
    }
    for (i, view) in [(0usize, TerrainView::Face), (3, TerrainView::Face), (8, TerrainView::Depth)] {
        let p = plan_in(i, view);
        summary +=
            &format!("{} [{}]: {} nodes, plan {:016x}\n", scripted_views(R, 4.0 / 3.0, 1)[i].0, view.name(), p.nodes.len(), p.plan_hash());
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/terrain_plans_floor0.txt");
    let expected = std::fs::read_to_string(&path).expect("the floor-0 record of the plans");
    assert_eq!(summary, expected, "a plan at floor 0 differs from the plan recorded before the floor existed");
}

/// R of a position at floor 3 with every tile of it resident (records from the field), with the bounds R carries.
fn resident_r(position: PlanetFixed, lod: &LodParams) -> (HashMap<TileId, ResidentTile>, TileBounds) {
    let field = Reference(1);
    let r = residency_set_field(&TangentWarp, position, &field, lod, &mut SelectionCache::new());
    let records = ResidentRecords::new(&field, &TangentWarp, lod);
    (r.tiles.iter().map(|e| (e.tile, records.record(e.tile))).collect(), r.bounds)
}

// spec: LOD-011, STRM-016
#[test]
fn at_floor_3_the_cover_with_r_resident_equals_the_plan_and_with_only_the_roots_resident_it_is_never_blank() {
    let lod = production(3);
    for (name, camera) in views().into_iter().chain([("far-100000km", far_camera())]) {
        let (set, bounds) = resident_r(camera.position, &lod);
        let cover = cover_selection_bounded(&TangentWarp, &camera, &lod, &|t| set.get(&t).map(|r| r.origin), &bounds);
        let plan = plan_of(camera, lod.clone());
        let (mut a, mut b): (Vec<TileId>, Vec<TileId>) = (cover.clone(), plan.nodes.iter().map(|n| n.tile).collect());
        a.sort();
        b.sort();
        assert_eq!(a, b, "{name}: with R resident the cover is the plan");
        assert!(cover.iter().all(|t| t.level() >= 3), "{name}");
        // Only the 384 roots resident: the cover is the visible roots, level 3 only, a subset of the resident tiles.
        let records = ResidentRecords::new(&Reference(1), &TangentWarp, &lod);
        let roots: HashMap<TileId, ResidentTile> = floor_tiles(&lod).into_iter().map(|t| (t, records.record(t))).collect();
        let cover = cover_selection_bounded(&TangentWarp, &camera, &lod, &|t| roots.get(&t).map(|r| r.origin), &bounds);
        assert!(!cover.is_empty() && cover.iter().all(|t| t.level() == 3 && roots.contains_key(t)), "{name}: the visible roots");
    }
}

// spec: LOD-009, LOD-011
#[test]
fn a_change_of_the_render_floor_starts_the_reused_values_over_and_every_plan_equals_the_plan_of_a_fresh_cache() {
    let (_, camera) = views().into_iter().nth(1).unwrap();
    let request = |floor: u8| TerrainRequest {
        camera,
        size: SIZE,
        view: TerrainView::Level,
        lod: production(floor),
        face_mapping: TangentWarp.id().to_string(),
        seed: 1,
    };
    let field = Reference(1);
    let mut cache = SelectionCache::new();
    let warm0 = plan_terrain_cached(&request(0), &field, &mut cache).unwrap();
    let misses = cache.misses();
    assert!(misses > 0);
    assert_eq!(plan_terrain_cached(&request(0), &field, &mut cache).unwrap(), warm0, "a warm plan is the first plan");
    assert_eq!(cache.misses(), misses, "and computes no tile again");
    // The floor changes: everything is computed again, and the plan is the plan of a fresh cache at that floor.
    let at3 = plan_terrain_cached(&request(3), &field, &mut cache).unwrap();
    assert!(cache.misses() > misses, "the reused values start over when the floor changes");
    assert_eq!(at3, plan_terrain_field(&request(3), &field).unwrap());
    let back = plan_terrain_cached(&request(0), &field, &mut cache).unwrap();
    assert_eq!(back, warm0, "and back again");
}

// spec: LOD-011, STRM-001
#[test]
fn the_roots_of_the_streamer_and_of_the_selection_are_the_same_tiles_at_every_floor() {
    for floor in 0..=4u8 {
        assert_eq!(planet_streaming::floor_roots(floor), floor_tiles(&production(floor)), "floor {floor}");
        assert_eq!(planet_streaming::StreamConfig { floor, ..Default::default() }.roots().len(), 6 * 4usize.pow(u32::from(floor)));
    }
}
