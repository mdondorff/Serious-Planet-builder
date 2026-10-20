//! SPIKE-17 (spike branch only): one tile of an owner bundle in detail.
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::water::{WaterSource, WaterTileCache};
use planet_generators::height::HeightField;
const R: f64 = 6_371_000.0;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, 1), 256).unwrap().bounding_water();
    let field = world.field().unwrap();
    let t = TileId::from_raw(a[1].parse().unwrap()).unwrap();
    let c = t.center_direction(&TangentWarp);
    println!("tile L{} centre lat {:.4} lon {:.4}", t.level(), c.z.asin().to_degrees(), c.y.atan2(c.x).to_degrees());
    let p = world.bake_probe(c).unwrap();
    let wet = world.with_water(|f| WaterSource::water_at(f, c).ok().flatten().is_some()).unwrap().unwrap();
    let band = world.with_water(|f| WaterSource::band_at(f, c).ok().flatten().is_some()).unwrap().unwrap();
    println!("centre: height {:.1} m, bake continent {:.3} elevation {:.1} ocean_vertex {}, water_at {wet}, band_at {band}", field.height(c), p.continent, p.elevation_m, p.ocean_vertex);
    let ans = world.with_water(|f| WaterTileCache::new().tile_water(f, &TangentWarp, t, R)).unwrap().unwrap().unwrap();
    println!("tile answer wet: {}", ans.is_some());
    // distance to the nearest ocean vertex / sea point along 16 bearings
    let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
    let east = c.cross(helper).normalized();
    let north = c.cross(east).normalized();
    let mut nearest_ocean = f64::INFINITY;
    let mut nearest_sea = f64::INFINITY;
    for k in 0..24 {
        let ang = f64::from(k) * std::f64::consts::TAU / 24.0;
        let dir = east * libm::cos(ang) + north * libm::sin(ang);
        for step in 1..=200 {
            let d = f64::from(step) * 500.0;
            let q = (c + dir * (d / R)).normalized();
            if nearest_ocean > d && world.bake_probe(q).is_some_and(|b| b.ocean_vertex) { nearest_ocean = d; }
            if nearest_sea > d && world.with_water(|f| WaterSource::water_at(f, q).ok().flatten().is_some()).unwrap().unwrap() { nearest_sea = d; }
        }
    }
    println!("nearest ocean vertex {nearest_ocean:.0} m, nearest sea point {nearest_sea:.0} m (within 100 km, 24 bearings)");
}
