//! SPIKE-17 (spike branch only): tiles inland of the coasts (2 to 130 km) whose terrain is under the sea level and that lie within the
//! oracle's reach of an ocean vertex, with a dry water answer.
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::water::{WaterSource, WaterTileCache};
use planet_generators::height::HeightField;
const R: f64 = 6_371_000.0;
fn main() {
    let seed: u64 = std::env::var("SP17_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let res: u32 = std::env::var("SP17_RES").ok().and_then(|v| v.parse().ok()).unwrap_or(256);
    let world = (if let Ok(s) = std::env::var("SP17_STORE") { WorldTerrain::open_store(planet_world_def::WorldDefinition::single_planet(R, seed), std::path::Path::new(&s), 16384) } else { WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, seed), res) }).unwrap().bounding_water();
    let field = world.field().unwrap();
    let map = TangentWarp;
    let is_sea = |d: Vec3| world.with_water(|f| WaterSource::water_at(f, d).ok().flatten().is_some()).unwrap().unwrap();
    let connected = |c: Vec3| -> bool {
        let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let east = c.cross(helper).normalized();
        let north = c.cross(east).normalized();
        for j in -20..=20 { for i in -20..=20 {
            let (dx, dy) = (f64::from(i) * 5_000.0, f64::from(j) * 5_000.0);
            if dx * dx + dy * dy > 1.0e10 { continue; }
            let d = (c + east * (dx / R) + north * (dy / R)).normalized();
            if world.bake_probe(d).is_some_and(|b| b.ocean_vertex) { return true; }
        } }
        false
    };
    let mut coasts: Vec<Vec3> = Vec::new();
    for lat_deg in (-17..=17).map(|i| f64::from(i) * 5.0) {
        let lat = lat_deg.to_radians();
        let at = |s: f64| { let lon = s / (R * libm::cos(lat)); Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat)) };
        let steps = (std::f64::consts::TAU * R * libm::cos(lat) / 20_000.0) as usize;
        let mut prev = is_sea(at(0.0)); let mut found = 0;
        for k in 1..steps {
            let cur = is_sea(at(k as f64 * 20_000.0));
            if prev != cur && found < 4 {
                let (mut a, mut b) = ((k - 1) as f64 * 20_000.0, k as f64 * 20_000.0);
                for _ in 0..20 { let m = 0.5 * (a + b); if is_sea(at(m)) == prev { a = m } else { b = m } }
                coasts.push(at(0.5 * (a + b))); found += 1;
            }
            prev = cur;
        }
    }
    eprintln!("{} coasts", coasts.len());
    let dists = [1_000.0, 3_000.0, 6_000.0, 12_000.0, 25_000.0, 45_000.0, 70_000.0, 100_000.0, 130_000.0];
    for lvl in [10u8, 12, 14, 16] {
        let (mut n, mut bad) = (0, 0);
        let mut ex: Vec<String> = Vec::new();
        let mut by_dist = vec![0usize; dists.len()];
        for c in &coasts {
            let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
            let east = c.cross(helper).normalized();
            let north = c.cross(east).normalized();
            for dir in [north, east, north * -1.0, east * -1.0] {
                for (di, d) in dists.iter().enumerate() {
                    let p = (*c + dir * (d / R)).normalized();
                    if is_sea(p) { continue; }
                    let t = TileId::from_direction(&map, p, lvl);
                    let mut m = f64::INFINITY;
                    for l in 0..=8 { for k in 0..=8 { m = m.min(field.height(t.sample_direction(&map, 8, k, l))); } }
                    if m >= -2.0 { continue; }
                    let tc = t.center_direction(&map);
                    if !connected(tc) { continue; }
                    n += 1;
                    let ans = world.with_water(|f| WaterTileCache::new().tile_water(f, &map, t, R)).unwrap().unwrap().unwrap();
                    if ans.is_none() { bad += 1; by_dist[di] += 1; if ex.len() < 4 { ex.push(format!("{:?} min {m:.1} at {:.0} km", t, d / 1000.0)); } }
                }
            }
        }
        eprintln!("L{lvl}: {n} tiles inland under the sea and connected, {bad} dry; by distance {:?}; {:?}", by_dist, ex);
    }
}
