//! SPIKE-23 (spike branch only): single-thread time of one tile job's stages on a drawn tile of the edit-latency case (200 m, stamp 150 m):
//! the compact tile of the base field, of the field with the stamp, and the heights alone. A rough timing: run it only on a quiet machine
//! (a perf window or the planner's go).
use planet::editperf::case_stamp;
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_generators::compact::compact_tile_field;
use planet_generators::height::HeightField;
use planet_world_def::BodyId;
use std::time::Instant;
const R: f64 = 6_371_000.0;
fn main() {
    let def = planet_world_def::WorldDefinition::single_planet(R, 1);
    let world = match std::env::var("SP23_STORE") {
        Ok(s) => WorldTerrain::open_store(def, std::path::Path::new(&s), 16384).unwrap(),
        Err(_) => WorldTerrain::open(def, 256).unwrap(),
    };
    let stamp = case_stamp(150.0);
    let level: u8 = std::env::var("SP23_LEVEL").ok().and_then(|v| v.parse().ok()).unwrap_or(19);
    let base_field = world.field().unwrap();
    let tiles: Vec<planet_core::TileId> = (0..24).map(|i| {
        let d = (stamp.dir + planet_core::Vec3::new(1.0, 0.3, 0.2).normalized() * (f64::from(i) * 6.0 / R)).normalized();
        planet_core::TileId::from_direction(&TangentWarp, d, level)
    }).collect();
    let time = |label: &str, f: &dyn Fn(planet_core::TileId)| {
        for t in tiles.iter().take(2) { f(*t); } // warm: blocks of the store
        let started = Instant::now();
        for t in &tiles { f(*t); }
        println!("{label}: {:.2} ms per tile ({} tiles, one thread)", started.elapsed().as_secs_f64() * 1000.0 / tiles.len() as f64, tiles.len());
    };
    time("compact tile, base field (no stamp)", &|t| { let _ = compact_tile_field(base_field.as_ref(), &TangentWarp, t, 32, R); });
    let before = world.definition();
    let mut edited = (*before).clone();
    edited.put_item_spec(BodyId::PLANET, stamp).unwrap();
    world.set_definition(std::sync::Arc::new(edited)).unwrap();
    let edit_field = world.field().unwrap();
    time("compact tile, field with the stamp", &|t| { let _ = compact_tile_field(edit_field.as_ref(), &TangentWarp, t, 32, R); });
    // the heights alone (5,446 samples of one tile)
    time("5,446 heights of the base field", &|t| {
        let c = t.center_direction(&TangentWarp);
        let dirs: Vec<planet_core::Vec3> = (0..5446).map(|k| (c + planet_core::Vec3::new(1.0, 0.0, 0.0) * (f64::from(k as u32 % 74) * 1.0e-8)).normalized()).collect();
        let _ = base_field.heights(&dirs);
    });
}
