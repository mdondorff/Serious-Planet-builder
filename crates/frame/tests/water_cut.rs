//! Tier A: the cut of the water nodes (ATMO-020): the horizon test and the height test of `plan_water`, as pure functions at their
//! boundaries and through the plan.

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::look::water::WaterLook;
use planet_frame::water::{
    plan_water, tile_above_water, tile_beyond_horizon, visible_distance_m, WaterPlan, WaterRequest, WaterSource, WaterTileCache, WaterView,
};
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::global::ReadError;
use planet_generators::height::{HeightField, Reference};
use planet_generators::water::{WaterBodyId, WaterSample};
use planet_world_def::{LiquidKindId, LiquidTable};

const R: f64 = 6_371_000.0;

/// The unit direction at central angle `theta` from +x in the xy plane.
fn dir_at(theta: f64) -> Vec3 {
    Vec3::new(libm::cos(theta), libm::sin(theta), 0.0)
}

/// The central angle at which a point of the sphere `r` lies at distance `d` from a camera on +x at radius `c`.
fn angle_for(c: f64, r: f64, d: f64) -> f64 {
    libm::acos((c * c + r * r - d * d) / (2.0 * c * r))
}

// spec: ATMO-020
#[test]
fn a_tile_at_the_horizon_of_its_level_is_kept_just_inside_and_cut_just_beyond_for_a_camera_at_1_m_at_3_km_and_at_orbit() {
    for (height, level) in [(1.0, 0.0), (3.0, 0.0), (2_000.0, 0.0), (20_000_000.0, 0.0), (1.0, 37.5), (3.0, -140.0)] {
        for (rise, chord) in [(0.0, 0.0), (0.0, 800.0), (1.7, 800.0), (4.0, 20_000.0)] {
            let shift = rise * 1.8;
            let r = R + level;
            let camera = Vec3::new(r + height, 0.0, 0.0);
            let reach = visible_distance_m(R, level, height, rise, 0.0) + chord * (r / R) + shift;
            // Exact on the tangent: no rise, no chord, no shift.
            let tangent = libm::sqrt(2.0 * r * height + height * height);
            assert!((visible_distance_m(R, level, height, 0.0, 0.0) - tangent).abs() < 1.0e-6 * tangent.max(1.0));
            // Orbit: the sphere is only partly in sight; the angle must exist.
            let theta = |d: f64| angle_for(r + height, r, d);
            let (inside, outside) = (theta(reach - 0.5), theta(reach + 0.5));
            assert!(inside.is_finite() && outside.is_finite(), "height {height}: no tile at the reach");
            let cut = |th: f64| tile_beyond_horizon(camera, dir_at(th), chord, R, level, rise, shift, 0.0);
            assert!(!cut(inside), "h {height} level {level} rise {rise} chord {chord}: a tile 0.5 m inside the reach is kept");
            assert!(cut(outside), "h {height} level {level} rise {rise} chord {chord}: a tile 0.5 m beyond the reach is cut");
            // The tile under the camera is never cut.
            assert!(!cut(0.0));
        }
    }
}

// spec: ATMO-020
#[test]
fn the_rise_of_the_waves_moves_the_horizon_out_by_the_tangent_of_the_raised_surface() {
    let (level, height) = (0.0, 1.0);
    let camera = Vec3::new(R + height, 0.0, 0.0);
    // 6 km out on the sphere: beyond the 3.6 km horizon of a camera at 1 m, inside the 3.6 + 5.0 km of a 2 m rise.
    let theta = 6_000.0 / R;
    assert!(tile_beyond_horizon(camera, dir_at(theta), 0.0, R, level, 0.0, 0.0, 0.0));
    assert!(!tile_beyond_horizon(camera, dir_at(theta), 0.0, R, level, 2.0, 0.0, 0.0));
    // The visible distance grows with the level's sphere and with the rise, never shrinks.
    let mut last = 0.0;
    for rise in [0.0, 0.1, 0.5, 2.0, 10.0] {
        let v = visible_distance_m(R, level, height, rise, 0.0);
        assert!(v > last || rise == 0.0);
        last = v;
    }
}

// spec: ATMO-020
#[test]
fn the_sag_of_a_coarse_drawn_surface_moves_the_horizon_out() {
    let camera = Vec3::new(R + 2.0, 0.0, 0.0);
    // A cell of 10 km: a sag of 8 m under the sphere, a horizon some 10 km further.
    let sag = planet_frame::water::surface_sag_m(R, 10_000.0);
    assert!((sag - 8.0).abs() < 0.2, "{sag}");
    let theta = 9_000.0 / R;
    assert!(tile_beyond_horizon(camera, dir_at(theta), 0.0, R, 0.0, 0.0, 0.0, 0.0));
    assert!(!tile_beyond_horizon(camera, dir_at(theta), 0.0, R, 0.0, 0.0, 0.0, sag));
}

// spec: ATMO-020
#[test]
fn a_camera_at_or_under_the_level_or_a_lake_above_the_camera_never_cuts() {
    let far = dir_at(std::f64::consts::PI * 0.9);
    // Under the sea, at it, and (barely) above it.
    for (cam_r, level, cut) in [(R - 30.0, 0.0, false), (R, 0.0, false), (R - 1.0e-9, 0.0, false), (R + 1.0, 0.0, true)] {
        let camera = Vec3::new(cam_r, 0.0, 0.0);
        assert_eq!(tile_beyond_horizon(camera, far, 100.0, R, level, 1.0, 1.8, 0.0), cut, "camera at {cam_r}");
    }
    // A lake at +200 m above a camera at +100 m: this tile's own level is above the camera.
    let camera = Vec3::new(R + 100.0, 0.0, 0.0);
    assert!(!tile_beyond_horizon(camera, far, 100.0, R, 200.0, 1.0, 1.8, 0.0));
    // The sea of the same camera is cut: the cut is per tile level.
    assert!(tile_beyond_horizon(camera, far, 100.0, R, 0.0, 1.0, 1.8, 0.0));
}

// spec: ATMO-020
#[test]
fn a_tile_whose_lowest_terrain_reaches_the_level_plus_the_rise_carries_no_visible_water_and_one_below_it_does() {
    assert!(tile_above_water(2.0, 0.0, 1.5));
    assert!(tile_above_water(1.5001, 0.0, 1.5), "at the bound, with the f32 margin, the water cannot reach the terrain");
    assert!(!tile_above_water(4000.0 + 1.0e-4, 4000.0, 0.0), "a bound 0.1 mm over a high level is inside the f32 rounding");
    assert!(!tile_above_water(1.4999, 0.0, 1.5));
    assert!(!tile_above_water(-3000.0, 0.0, 1.5));
    // A lake's level is the tile's own, not the sea's.
    assert!(!tile_above_water(120.0, 130.0, 1.0));
    assert!(tile_above_water(131.01, 130.0, 1.0));
}

struct HeightSea<'a> {
    field: &'a Reference,
    level: f64,
}

impl WaterSource for HeightSea<'_> {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok((self.field.height(dir) <= self.level).then_some(WaterSample {
            level_m: self.level,
            body: WaterBodyId::SEA,
            kind: LiquidKindId::WATER,
        }))
    }
}

/// Every tile carries water, whatever the terrain: the cut alone decides (the case #248 left: every node of the plan).
struct Everywhere(f64);

impl WaterSource for Everywhere {
    fn water_at(&self, _: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok(Some(WaterSample { level_m: self.0, body: WaterBodyId::SEA, kind: LiquidKindId::WATER }))
    }
}

fn lod() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

fn terrain(index: usize) -> TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    plan_terrain(&TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: lod(),
        face_mapping: "tangent-v1".into(),
        seed: 1,
    })
    .unwrap()
}

fn water(src: &dyn WaterSource, plan: &TerrainPlan, cut: Option<&dyn HeightField>, cache: &mut WaterTileCache) -> WaterPlan {
    let (table, look, lod) = (LiquidTable::earth(), WaterLook::EARTH, lod());
    let req =
        WaterRequest { source: src, map: &TangentWarp, liquids: &table, look: &look, lod: &lod, time_s: 3.0, view: WaterView::Lit, cut };
    plan_water(&req, cache, plan).unwrap()
}

// spec: ATMO-020
#[test]
fn the_cut_plan_is_the_full_plan_less_nodes_the_kept_nodes_are_bit_for_bit_the_full_ones_and_the_tiles_answers_are_the_same() {
    let field = Reference(1);
    let src = Everywhere(0.0);
    let mut dropped_somewhere = false;
    for (index, name) in [(3, "aerial-20km"), (4, "low-2km"), (5, "ground-200m"), (6, "ground-1m")] {
        let plan = terrain(index);
        let mut full_cache = WaterTileCache::new();
        let mut cut_cache = WaterTileCache::new();
        let full = water(&src, &plan, None, &mut full_cache);
        let cut = water(&src, &plan, Some(&field), &mut cut_cache);
        assert_eq!(full.nodes.len(), plan.nodes.len(), "{name}: every node carries water in this source");
        assert!(cut.nodes.len() <= full.nodes.len());
        // The kept nodes are the full plan's, in order, bit for bit.
        let mut it = full.nodes.iter();
        for n in &cut.nodes {
            assert!(it.any(|f| f == n), "{name}: node {} of the cut plan is not in the full plan, or out of order", n.node);
        }
        // The tile answers (queries) are the same: the cut is made after them, the book and the memo are untouched.
        assert_eq!(cut_cache.counters(), full_cache.counters(), "{name}");
        assert_eq!(cut_cache.len(), full_cache.len(), "{name}");
        assert_eq!(cut_cache.cut_horizon + cut_cache.cut_height, (full.nodes.len() - cut.nodes.len()) as u64, "{name}");
        dropped_somewhere |= cut.nodes.len() < full.nodes.len();
        // A pure function of the camera, the tile and the level: a second plan, with the memo warm or cold, is the same.
        assert_eq!(water(&src, &plan, Some(&field), &mut cut_cache), cut, "{name}: warm");
        assert_eq!(water(&src, &plan, Some(&field), &mut WaterTileCache::new()), cut, "{name}: cold");
        println!(
            "{name}: {} of {} nodes kept ({} horizon, {} height)",
            cut.nodes.len(),
            full.nodes.len(),
            cut_cache.cut_horizon,
            cut_cache.cut_height
        );
    }
    assert!(dropped_somewhere, "no view dropped a node");
}

// spec: ATMO-020
#[test]
fn a_tile_that_floods_is_never_cut_and_a_camera_under_the_level_cuts_nothing_by_the_horizon() {
    let field = Reference(1);
    for index in [3usize, 4, 5, 6] {
        let plan = terrain(index);
        // The sea at 0 m: the nodes the pass floods anywhere (the lowest terrain of the tile under the level) must all stay.
        let src = HeightSea { field: &field, level: 0.0 };
        let full = water(&src, &plan, None, &mut WaterTileCache::new());
        let cut = water(&src, &plan, Some(&field), &mut WaterTileCache::new());
        let lod = lod();
        for f in &full.nodes {
            let tile = plan.nodes[f.node as usize].tile;
            let e = planet_frame::lod::tile_extent(&TangentWarp, tile, &lod);
            let lowest = field.height_bounds(e.dir, e.angular_radius).0;
            let kept = cut.nodes.iter().any(|c| c.node == f.node);
            if lowest < 0.0 && !kept {
                // A tile whose terrain dips under the level is cut only by the horizon, by the predicate of the spec.
                let look = WaterLook::EARTH;
                let table = LiquidTable::earth();
                let liquid = planet_frame::look::water::LiquidAppearance::new(0, table.get(LiquidKindId::WATER).unwrap(), &look);
                let rise = liquid.amplitude_sum();
                assert!(
                    tile_beyond_horizon(
                        plan.camera.position.0,
                        e.dir,
                        e.chord_radius_m,
                        R,
                        0.0,
                        rise,
                        rise * (1.0 + look.choppy),
                        planet_frame::water::surface_sag_m(R, lod.sample_spacing(plan.nodes[f.node as usize].level))
                    ),
                    "tile {tile:?} floods (lowest {lowest} m) and is cut, but is inside the horizon"
                );
            }
        }
    }
    // A camera under the sea level: the sea of the whole plan stays (the horizon test is off; the height test alone remains).
    let (_, mut camera) = scripted_views(R, 4.0 / 3.0, 1)[6];
    camera.position = planet_core::PlanetFixed(camera.position.0.normalized() * (R - 20.0));
    let plan = plan_terrain(&TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: lod(),
        face_mapping: "tangent-v1".into(),
        seed: 1,
    })
    .unwrap();
    let src = Everywhere(0.0);
    let mut cache = WaterTileCache::new();
    water(&src, &plan, Some(&field), &mut cache);
    assert_eq!(cache.cut_horizon, 0, "under the level nothing is beyond the horizon");
}

/// A book that holds the answers of every node of `plan` (water everywhere at `level`) and no minimum.
fn answered_book(plan: &TerrainPlan, level: f64) -> std::sync::Arc<planet_frame::water::WaterBook> {
    let book = std::sync::Arc::new(planet_frame::water::WaterBook::new());
    for n in &plan.nodes {
        book.fill(&Everywhere(level), &TangentWarp, n.tile, R).unwrap();
    }
    book
}

/// The lowest own vertex a tile job would leave, the same for every tile of `plan`.
fn set_mins(book: &planet_frame::water::WaterBook, plan: &TerrainPlan, min: f32) {
    for n in &plan.nodes {
        book.set_min_height(n.tile, min);
    }
}

fn rise() -> f64 {
    let table = LiquidTable::earth();
    planet_frame::look::water::LiquidAppearance::new(0, table.get(LiquidKindId::WATER).unwrap(), &WaterLook::EARTH).amplitude_sum()
}

/// The lowest f32 for which the height test cuts at `level` and the f32 below it (the boundary within one ulp).
fn boundary(level: f64, rise: f64) -> (f32, f32) {
    let mut m = (level + rise - 1.0) as f32;
    while !tile_above_water(f64::from(m), level, rise) {
        m = f32::from_bits(if m >= 0.0 { m.to_bits() + 1 } else { m.to_bits() - 1 });
    }
    let below = f32::from_bits(if m > 0.0 { m.to_bits() - 1 } else { m.to_bits() + 1 });
    (m, below)
}

// spec: ATMO-020
#[test]
fn the_exact_minimum_cuts_a_node_at_its_bound_and_keeps_it_one_ulp_below_for_any_level_and_a_camera_at_1_m() {
    let field = Reference(1);
    let plan = terrain(6);
    let rise = rise();
    let mut exercised = 0;
    for level in [0.0, 37.5, -140.0, 4000.0, 9000.0] {
        let (cut_at, kept_below) = boundary(level, rise);
        assert!(tile_above_water(f64::from(cut_at), level, rise) && !tile_above_water(f64::from(kept_below), level, rise));
        let loose = water(&Everywhere(level), &plan, Some(&field), &mut WaterTileCache::new());
        // A node the horizon and the loose bound keep: the tile under the camera.
        let Some(first) = loose.nodes.first() else { continue };
        exercised += 1;
        let target = first.node;
        let tile = plan.nodes[target as usize].tile;
        let book = answered_book(&plan, level);
        for (min, cut) in [(cut_at, true), (kept_below, false), (cut_at + 1.0, true), (-50.0, false)] {
            book.set_min_height(tile, min);
            let mut cache = WaterTileCache::with_book(std::sync::Arc::clone(&book));
            let exact = water(&Everywhere(level), &plan, Some(&field), &mut cache);
            assert_eq!(!exact.nodes.iter().any(|n| n.node == target), cut, "level {level} min {min}: node cut or kept");
            assert_eq!(cache.cut_exact, u64::from(cut), "level {level} min {min}");
            // Every other node is the loose plan's, bit for bit.
            let others: Vec<_> = loose.nodes.iter().filter(|n| n.node != target).collect();
            assert_eq!(exact.nodes.iter().filter(|n| n.node != target).collect::<Vec<_>>(), others);
        }
    }
    assert!(exercised >= 2, "{exercised} levels exercised");
}

// spec: ATMO-020
#[test]
fn a_dry_basin_under_a_dry_rim_is_cut_by_the_exact_minimum_where_the_loose_bound_keeps_it_and_a_tile_without_a_minimum_keeps_the_loose_verdict(
) {
    let field = Reference(1);
    for index in [3usize, 4, 5, 6] {
        let plan = terrain(index);
        let src = Everywhere(0.0);
        let loose = water(&src, &plan, Some(&field), &mut WaterTileCache::new());
        // A book with answers and no minimum (roots, a job that did not run) is the loose cut.
        let book = answered_book(&plan, 0.0);
        let mut cache = WaterTileCache::with_book(std::sync::Arc::clone(&book));
        assert_eq!(water(&src, &plan, Some(&field), &mut cache), loose, "view {index}");
        assert_eq!(cache.cut_exact, 0);
        // Every tile's job left a minimum 30 m over the level: all cut.
        set_mins(&book, &plan, 30.0);
        let mut cache = WaterTileCache::with_book(std::sync::Arc::clone(&book));
        let dry = water(&src, &plan, Some(&field), &mut cache);
        assert!(dry.nodes.is_empty(), "view {index}: tiles 30 m over the level show no water");
        assert_eq!(cache.cut_horizon + cache.cut_height, plan.nodes.len() as u64);
        assert_eq!(cache.cut_exact, loose.nodes.len() as u64, "view {index}: the exact minimum cuts every node the loose cut kept");
        // Minimums under the level change nothing.
        set_mins(&book, &plan, -3.0);
        assert_eq!(water(&src, &plan, Some(&field), &mut WaterTileCache::with_book(book)), loose, "view {index}");
    }
}

// spec: ATMO-020
#[test]
fn a_camera_under_the_level_is_cut_by_the_exact_minimum_alike_and_a_late_minimum_is_found_by_a_later_plan() {
    let field = Reference(1);
    let (_, mut camera) = scripted_views(R, 4.0 / 3.0, 1)[6];
    camera.position = planet_core::PlanetFixed(camera.position.0.normalized() * (R - 20.0));
    let plan = plan_terrain(&TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: lod(),
        face_mapping: "tangent-v1".into(),
        seed: 1,
    })
    .unwrap();
    let src = Everywhere(0.0);
    let book = answered_book(&plan, 0.0);
    let mut cache = WaterTileCache::with_book(std::sync::Arc::clone(&book));
    let before = water(&src, &plan, Some(&field), &mut cache);
    assert_eq!(cache.cut_exact, 0);
    // The jobs' minimums arrive after the first plan (the tile cache memoised the answers, not the missing minimums).
    set_mins(&book, &plan, 25.0);
    let after = water(&src, &plan, Some(&field), &mut cache);
    assert!(after.nodes.is_empty() && !before.nodes.is_empty());
    assert_eq!(cache.cut_exact, before.nodes.len() as u64);
    // The cut changes no answer: no field query was made by either plan.
    assert_eq!(cache.counters().queries, 0);
}

// spec: ATMO-020
#[test]
fn the_book_keeps_a_minimum_only_beside_a_wet_answer() {
    use planet_core::{Face, TileId};
    let book = planet_frame::water::WaterBook::new();
    let (wet, dry) = (TileId::new(Face(0), 4, 3, 3).unwrap(), TileId::new(Face(0), 4, 4, 4).unwrap());
    book.set_min_height(wet, 1.0);
    assert_eq!(book.min_height(wet), None, "no answer yet: no minimum");
    book.fill(&Everywhere(0.0), &TangentWarp, wet, R).unwrap();
    book.fill(&HeightSea { field: &Reference(1), level: -1.0e6 }, &TangentWarp, dry, R).unwrap();
    book.set_min_height(wet, 1.0);
    book.set_min_height(dry, 1.0);
    book.set_min_height(wet, f32::NAN);
    assert_eq!((book.min_height(wet), book.min_height(dry)), (Some(1.0), None));
}

// spec: ATMO-020
#[test]
fn a_minimum_the_book_changes_later_is_the_one_the_next_plan_cuts_by_for_a_tile_rebuilt_in_another_format() {
    let field = Reference(1);
    let plan = terrain(6);
    let src = Everywhere(0.0);
    let book = answered_book(&plan, 0.0);
    let loose = water(&src, &plan, Some(&field), &mut WaterTileCache::new());
    let mut cache = WaterTileCache::with_book(std::sync::Arc::clone(&book));
    // The first format\'s tiles leave a minimum over the bound: every node is cut.
    set_mins(&book, &plan, 25.0);
    assert!(water(&src, &plan, Some(&field), &mut cache).nodes.is_empty());
    // The tiles are rebuilt in the other format; its lowest vertex is under the bound: the same cache keeps every node again.
    set_mins(&book, &plan, -1.0);
    assert_eq!(water(&src, &plan, Some(&field), &mut cache), loose);
    assert_eq!((cache.cut_exact, cache.no_minimum), (0, 0));
}
