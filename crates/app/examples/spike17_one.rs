use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::water::{WaterSource, WaterTileCache};
use planet_generators::height::HeightField;
const R: f64 = 6_371_000.0;
fn main() {
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, 1), 256).unwrap().bounding_water();
    let field = world.field().unwrap();
    let map = TangentWarp;
    for raw in [12105675798371920865u64, 12105675798371943206, 11961560610296040463, 12033618204333977678] {
        let t = TileId::from_raw(raw).unwrap();
        let ans = world.with_water(|f| WaterTileCache::new().tile_water(f, &map, t, R)).unwrap().unwrap().unwrap();
        let mut best = (f64::INFINITY, Vec3::new(0.0, 0.0, 1.0));
        for l in 0..=32 { for k in 0..=32 { let d = t.sample_direction(&map, 32, k, l); let h = field.height(d); if h < best.0 { best = (h, d); } } }
        let wet = world.with_water(|f| WaterSource::water_at(f, best.1).ok().flatten().is_some()).unwrap().unwrap();
        let band = world.with_water(|f| WaterSource::band_at(f, best.1).ok().flatten().is_some()).unwrap().unwrap();
        let p = world.bake_probe(best.1).unwrap();
        // children
        let mut kids = Vec::new();
        for c in t.children().unwrap() { kids.push(world.with_water(|f| WaterTileCache::new().tile_water(f, &map, c, R)).unwrap().unwrap().unwrap().is_some()); }
        println!("L{} {:?}: answer wet {} | lowest {:.1} m at the point: water_at {} band_at {} bake continent {:.3} elevation {:.1} ocean_vertex {} | children wet {:?}", t.level(), t, ans.is_some(), best.0, wet, band, p.continent, p.elevation_m, p.ocean_vertex, kids);
    }
}
