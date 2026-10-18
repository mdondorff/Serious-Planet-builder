//! CPU half of the water cost attribution (windows 6 to 8): for the coast glide and descent camera scripts of the lit perf scenes, per
//! frame, the water nodes of the water plan, their triangles and vertices, and the screen pixels whose CPU ray meets the sea sphere
//! where the terrain lies below the level inside a water node's tile. Measurement only: no GPU, no product change.
//!
//! `cargo run --release -p planet --example water_attrib -- <store dir> [--frame-step N] [--pixel-step N] [--frames N]`
//! Prints one line per frame (`flight frame nodes water_nodes triangles vertices pixels`) and the medians.

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::litscenes::lit_views;
use planet_frame::look::file::LookParams;
use planet_frame::water::{plan_water, WaterRequest, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_cached, Camera, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_world_def::{BodyId, WorldDefinition};
use std::collections::{HashMap, HashSet};

/// The point of the sphere of radius `r` (the sea level) that a pixel's ray meets.
fn sea_point(cam: &Camera, size: (u32, u32), x: u32, y: u32, r: f64) -> Option<Vec3> {
    let (right, up, fwd) = cam.ray_frame();
    let (u, v) = (2.0 * (f64::from(x) + 0.5) / f64::from(size.0) - 1.0, 1.0 - 2.0 * (f64::from(y) + 0.5) / f64::from(size.1));
    let dir = (fwd + right * u + up * v).normalized();
    let from = cam.position.0;
    let (b, l) = (from.dot(dir), from.length());
    let c = (l - r) * (l + r);
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = if c >= 0.0 { -b - disc.sqrt() } else { -b + disc.sqrt() };
    (t > 0.0).then(|| (from + dir * t).normalized())
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let store = args.first().expect("usage: water_attrib <store dir> [--frame-step N] [--pixel-step N] [--frames N]").clone();
    let opt = |name: &str, default: usize| -> usize {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map_or(default, |v| v.parse().expect("a number"))
    };
    let (frame_step, pixel_step, frames) = (opt("--frame-step", 4), opt("--pixel-step", 8) as u32, opt("--frames", 240));
    let o = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).expect("options");
    let world = WorldTerrain::open_store(
        WorldDefinition::single_planet(6_371_000.0, o.seed),
        std::path::Path::new(&store),
        planet::streamperf::PRODUCTION_BAKE_RES,
    )
    .expect("the store opens")
    .bounding_water();
    let field = world.field().expect("field");
    let radius_m = world.radius_m();
    let level_m = world.definition().sea(BodyId::PLANET).expect("sea").level_m;
    let size = (2560u32, 1440u32);
    let aspect = f64::from(size.0) / f64::from(size.1);
    let cells = o.cells;
    let lod = LodParams {
        cells,
        viewport_h_px: f64::from(size.1),
        tau_px: o.tau,
        max_level: o.finest_level_for(cells),
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    let is_sea = |d: Vec3| planet::litperf::carries_sea(&world, d);
    let views = lit_views(radius_m, aspect, field.as_ref(), level_m, &is_sea, frames).expect("views");
    let look = LookParams::default();
    let table = world.definition().liquids().expect("liquids");
    let map = TangentWarp;
    for view in views.iter().filter(|v| v.name == "coast-glide" || v.name == "coast-descent") {
        let mut cache = SelectionCache::new();
        let (mut nodes, mut wn, mut tris, mut verts, mut pixels) = (vec![], vec![], vec![], vec![], vec![]);
        for (k, camera) in view.cameras.iter().enumerate().step_by(frame_step) {
            let req = TerrainRequest {
                camera: *camera,
                size,
                view: TerrainView::Level,
                lod: lod.clone(),
                face_mapping: "tangent-v1".into(),
                seed: field.seed(),
            };
            let plan = plan_terrain_cached(&req, field.as_ref(), &mut cache).expect("terrain plan");
            let wp = world
                .with_water(|f| {
                    plan_water(
                        &WaterRequest {
                            source: f,
                            map: &map,
                            liquids: &table,
                            look: &look.water,
                            lod: &lod,
                            time_s: 2.0 + k as f64 / 60.0,
                            view: WaterView::Lit,
                        },
                        &mut WaterTileCache::new(),
                        &plan,
                    )
                })
                .expect("water")
                .expect("this world has water")
                .expect("water plan");
            let mut by_level: HashMap<u8, HashSet<TileId>> = HashMap::new();
            for n in &wp.nodes {
                let t = plan.nodes[n.node as usize].tile;
                by_level.entry(t.level()).or_default().insert(t);
            }
            let mut covered = 0u64;
            for y in (0..size.1).step_by(pixel_step as usize) {
                for x in (0..size.0).step_by(pixel_step as usize) {
                    let Some(p) = sea_point(camera, size, x, y, radius_m + level_m) else { continue };
                    if field.height(p) >= level_m {
                        continue;
                    }
                    if by_level.iter().any(|(l, set)| set.contains(&TileId::from_direction(&map, p, *l))) {
                        covered += 1;
                    }
                }
            }
            let px = covered as f64 * f64::from(pixel_step * pixel_step);
            let wnodes = wp.nodes.len() as f64;
            let c = f64::from(cells);
            println!(
                "{} {k} {} {} {} {} {}",
                view.name,
                plan.nodes.len(),
                wp.nodes.len(),
                wnodes * c * c * 2.0,
                wnodes * (c + 1.0) * (c + 1.0),
                px
            );
            nodes.push(plan.nodes.len() as f64);
            wn.push(wnodes);
            tris.push(wnodes * c * c * 2.0);
            verts.push(wnodes * (c + 1.0) * (c + 1.0));
            pixels.push(px);
        }
        println!(
            "MEDIAN {} frames {} terrain_nodes {} water_nodes {} triangles {} vertices {} pixels {}",
            view.name,
            wn.len(),
            median(&mut nodes),
            median(&mut wn),
            median(&mut tris),
            median(&mut verts),
            median(&mut pixels)
        );
    }
}
