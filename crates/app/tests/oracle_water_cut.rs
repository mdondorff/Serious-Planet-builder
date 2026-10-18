//! Tier B: the cut of the water nodes leaves the picture alone (ATMO-020). Over the in-memory eroded world of 1,024 cells, in the four
//! coast views of the lit perf scenes (coast-2km, coast-3m, and frames of the glide and the descent), the screen pixels whose CPU ray
//! meets the sea sphere where the terrain lies below the level inside the tile of a water node are counted for the full selection and
//! for the cut one and for the cut with the exact minimum of the tiles' own vertices (ATMO-020): the counts are equal, and the cut drops the share of the nodes the ranges below pin. CPU only (no adapter).

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::litscenes::{lit_views, ViewKind};
use planet_frame::look::file::LookParams;
use planet_frame::water::{plan_water, WaterPlan, WaterRequest, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_cached, Camera, LodParams, SelectionCache, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
use planet_world_def::WorldDefinition;
use std::collections::{HashMap, HashSet};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);
/// One ray per 16 x 16 pixels.
const PIXEL_STEP: usize = 16;

/// The point of the sea sphere (level 0) a pixel's ray meets, if any.
fn sea_point(cam: &Camera, x: u32, y: u32) -> Option<Vec3> {
    let (right, up, fwd) = cam.ray_frame();
    let (u, v) = (2.0 * (f64::from(x) + 0.5) / f64::from(SIZE.0) - 1.0, 1.0 - 2.0 * (f64::from(y) + 0.5) / f64::from(SIZE.1));
    let dir = (fwd + right * u + up * v).normalized();
    let from = cam.position.0;
    let (b, l) = (from.dot(dir), from.length());
    let c = (l - R) * (l + R);
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = if c >= 0.0 { -b - disc.sqrt() } else { -b + disc.sqrt() };
    (t > 0.0).then(|| (from + dir * t).normalized())
}

/// Pixels (sampled) whose ray meets the sea where the terrain lies below the level, inside the tile of a node of `water`.
fn covered(cam: &Camera, field: &dyn HeightField, plan: &TerrainPlan, water: &WaterPlan) -> u64 {
    let mut by_level: HashMap<u8, HashSet<TileId>> = HashMap::new();
    for n in &water.nodes {
        let t = plan.nodes[n.node as usize].tile;
        by_level.entry(t.level()).or_default().insert(t);
    }
    let mut n = 0;
    for y in (0..SIZE.1).step_by(PIXEL_STEP) {
        for x in (0..SIZE.0).step_by(PIXEL_STEP) {
            let Some(p) = sea_point(cam, x, y) else { continue };
            if field.height(p) >= 0.0 {
                continue;
            }
            if by_level.iter().any(|(l, set)| set.contains(&TileId::from_direction(&TangentWarp, p, *l))) {
                n += 1;
            }
        }
    }
    n
}

// spec: ATMO-020
#[test]
fn the_cut_water_nodes_cover_the_same_sea_pixels_as_the_full_selection_in_the_four_coast_views_and_drop_between_a_twentieth_and_two_thirds_of_the_nodes(
) {
    let world = WorldTerrain::open(WorldDefinition::single_planet(R, 5), 1024).expect("the world opens").bounding_water();
    let field = world.field().unwrap();
    let o = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).expect("options");
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: o.tau,
        max_level: o.finest_level_for(o.cells),
        radius_m: R,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let is_sea = |d: Vec3| planet::litperf::carries_sea(&world, d);
    let views = lit_views(R, aspect, field.as_ref(), 0.0, &is_sea, 240).expect("the world has a beach");
    let look = LookParams::default();
    let table = world.definition().liquids().unwrap();
    // Per view: the share of the full water nodes the cut keeps, in percent (min, max over the sampled frames), pinned from the runs.
    let mut jobs: HashSet<TileId> = HashSet::new();
    let ranges: [(&str, f64, f64, (f64, f64)); 4] = [
        ("coast-2km", 70.0, 95.0, (45.0, 75.0)),
        ("coast-3m", 35.0, 65.0, (25.0, 50.0)),
        ("coast-glide", 40.0, 70.0, (30.0, 60.0)),
        ("coast-descent", 50.0, 80.0, (35.0, 65.0)),
    ];
    for (name, lo, hi, exact_range) in ranges {
        let view = views.iter().find(|v| v.name == name).expect("view");
        let picks: Vec<usize> = if view.kind == ViewKind::Flight { vec![0, 120, 239] } else { vec![0] };
        let mut cache = SelectionCache::new();
        let mut water_cache = WaterTileCache::new();
        let mut exact_cache = WaterTileCache::with_book(world.water_book());
        for k in picks {
            let camera = view.cameras[k];
            let req = TerrainRequest {
                camera,
                size: SIZE,
                view: TerrainView::Level,
                lod: lod.clone(),
                face_mapping: "tangent-v1".into(),
                seed: field.seed(),
            };
            let plan = plan_terrain_cached(&req, field.as_ref(), &mut cache).expect("terrain plan");
            let make = |cut: Option<&dyn HeightField>, wc: &mut WaterTileCache| {
                world
                    .with_water(|f| {
                        plan_water(
                            &WaterRequest {
                                source: f,
                                map: &TangentWarp,
                                liquids: &table,
                                look: &look.water,
                                lod: &lod,
                                time_s: 2.0 + k as f64 / 60.0,
                                view: WaterView::Lit,
                                cut,
                            },
                            wc,
                            &plan,
                        )
                    })
                    .unwrap()
                    .unwrap()
                    .unwrap()
            };
            let full = make(None, &mut water_cache);
            let cut = make(Some(field.as_ref()), &mut water_cache);
            // The tile jobs of the water tiles (ADR 0056): each leaves the lowest own vertex beside its answer (ATMO-020).
            let new: Vec<TileId> = cut.nodes.iter().map(|n| plan.nodes[n.node as usize].tile).filter(|t| jobs.insert(*t)).collect();
            std::thread::scope(|s| {
                for chunk in new.chunks(new.len().div_ceil(8).max(1)) {
                    let (world, field, lod) = (&world, &field, &lod);
                    s.spawn(move || {
                        for tile in chunk {
                            let built =
                                planet_generators::compact::tile_data_field(field.as_ref(), &TangentWarp, *tile, o.cells, R, lod.max_level);
                            world.fill_water(*tile, &built);
                        }
                    });
                }
            });
            let exact = make(Some(field.as_ref()), &mut exact_cache);
            let (a, b, c) = (
                covered(&camera, field.as_ref(), &plan, &full),
                covered(&camera, field.as_ref(), &plan, &cut),
                covered(&camera, field.as_ref(), &plan, &exact),
            );
            let exact_share = 100.0 * exact.nodes.len() as f64 / full.nodes.len().max(1) as f64;
            let share = 100.0 * cut.nodes.len() as f64 / full.nodes.len().max(1) as f64;
            println!(
                "{name} frame {k}: {} terrain nodes, {} water nodes, {} after the cut ({share:.0} %: {} horizon, {} height), {a} / {b} / {c} sea pixels; the exact minimum keeps {} ({exact_share:.0} %, {} cut by it)",
                plan.nodes.len(),
                full.nodes.len(),
                cut.nodes.len(),
                water_cache.cut_horizon,
                water_cache.cut_height,
                exact.nodes.len(),
                exact_cache.cut_exact
            );
            assert_eq!(a, b, "{name} frame {k}: the cut changes the sea pixels the CPU rays find in water nodes ({a} full, {b} cut)");
            assert_eq!(
                a, c,
                "{name} frame {k}: the exact minimum changes the sea pixels the CPU rays find in water nodes ({a} full, {c} exact)"
            );
            assert!(exact.nodes.len() <= cut.nodes.len() && exact_cache.cut_exact == (cut.nodes.len() - exact.nodes.len()) as u64);
            assert!(a > 0, "{name} frame {k}: the view looks at no sea");
            let (elo, ehi) = exact_range;
            assert!(
                (elo..=ehi).contains(&exact_share),
                "{name} frame {k}: the exact minimum keeps {exact_share:.1} % of the water nodes, expected {elo} to {ehi}"
            );
            assert!((lo..=hi).contains(&share), "{name} frame {k}: the cut keeps {share:.1} % of the water nodes, expected {lo} to {hi}");
        }
    }
}
