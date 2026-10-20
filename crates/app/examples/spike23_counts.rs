//! SPIKE-23 (spike branch only): the drawn tiles of the edit-latency cases, and how many of them the stamp changes by more than a
//! fraction of the screen-space tolerance at their distance (the tiles whose refresh the eye could see).
use planet::editperf::{camera_for, case_stamp};
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::{plan_terrain_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
use planet_world_def::BodyId;
const R: f64 = 6_371_000.0;
fn main() {
    let def = planet_world_def::WorldDefinition::single_planet(R, 1);
    let world = match std::env::var("SP23_STORE") {
        Ok(s) => WorldTerrain::open_store(def, std::path::Path::new(&s), 16384).unwrap(),
        Err(_) => WorldTerrain::open(def, 256).unwrap(),
    };
    let size = (2560u32, 1440u32);
    let aspect = f64::from(size.0) / f64::from(size.1);
    let lod = LodParams { cells: 32, viewport_h_px: 1440.0, tau_px: 6.0, radius_m: R, max_level: 22, max_height_m: 9735.0f64.max(LodParams::earth_1080p().max_height_m), ..LodParams::earth_1080p() };
    let pixel = 2.0 * libm::tan(lod.fov_y_rad / 2.0) / lod.viewport_h_px;
    println!("tau {} px, pixel angle {pixel:.3e} rad", lod.tau_px);
    for altitude in [1.0f64, 200.0, 2000.0] {
        for r in [50.0f64, 150.0, 500.0] {
            let stamp = case_stamp(r);
            let base_field = world.field().unwrap();
            let ground = |d: Vec3| base_field.height(d);
            let rho = (1.5 * r).max(altitude);
            let cam = camera_for(stamp.dir, &ground, R, rho, altitude, aspect);
            let req = TerrainRequest { camera: cam, size, view: TerrainView::Level, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 1 };
            let plan = plan_terrain_field(&req, base_field.as_ref()).unwrap();
            // the tiles the footprint find names: the drawn ones whose cap the stamp reaches (plan nodes, ancestors not needed for drawn)
            let affected = [planet_editor::commands::Affected { body: BodyId::PLANET, dir: stamp.dir, reach_m: stamp.reach_m }];
            let nodes: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
            let found = planet_editor::feed::edited_tiles(&affected, BodyId::PLANET, &nodes, R, &TangentWarp);
            // edited field
            let before = world.definition();
            let mut edited = (*before).clone();
            edited.put_item_spec(BodyId::PLANET, stamp.clone()).unwrap();
            world.set_definition(std::sync::Arc::new(edited)).unwrap();
            let edit_field = world.field().unwrap();
            let mut rows: Vec<(f64, f64)> = Vec::new(); // (max delta, tolerance m)
            for t in &found {
                let n = plan.nodes.iter().find(|n| n.tile == *t).unwrap();
                let (mut md, mut dist) = (0.0f64, f64::INFINITY);
                for l in 0..=4 { for k in 0..=4 {
                    let d = t.sample_direction(&TangentWarp, 4, k, l);
                    md = md.max((edit_field.height(d) - base_field.height(d)).abs());
                    dist = dist.min((cam.position.0 - d * (R + base_field.height(d))).length());
                } }
                let _ = n;
                rows.push((md, dist * lod.tau_px * pixel));
            }
            world.set_definition(before).unwrap();
            let count = |f: f64| rows.iter().filter(|(d, tol)| *d >= f * *tol).count();
            let mut levels = std::collections::BTreeMap::<u8, usize>::new();
            for t in &found { *levels.entry(t.level()).or_default() += 1; }
            println!("altitude {altitude:>6} m, stamp {r:>3} m: {} nodes in the plan, {} drawn tiles named; changed by more than 1.0 / 0.5 / 0.25 / 0.1 of the tolerance: {} / {} / {} / {}; any change at all: {}",
                plan.nodes.len(), found.len(), count(1.0), count(0.5), count(0.25), count(0.1), rows.iter().filter(|(d, _)| *d > 1e-9).count());
        }
    }
}
