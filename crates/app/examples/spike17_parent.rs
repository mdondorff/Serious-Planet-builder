//! SPIKE-17 (spike branch only): a tile that answers dry while a descendant (two levels down) answers wet.
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::water::{WaterSource, WaterTileCache};
const R: f64 = 6_371_000.0;
fn main() {
    let world = (if let Ok(s) = std::env::var("SP17_STORE") { WorldTerrain::open_store(planet_world_def::WorldDefinition::single_planet(R, 1), std::path::Path::new(&s), 16384) } else { WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, 1), 256) }).unwrap().bounding_water();
    let map = TangentWarp;
    let is_sea = |d: Vec3| world.with_water(|f| WaterSource::water_at(f, d).ok().flatten().is_some()).unwrap().unwrap();
    let wet = |t: TileId| world.with_water(|f| WaterTileCache::new().tile_water(f, &map, t, R)).unwrap().unwrap().unwrap().is_some();
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
    for lvl in 4u8..=14 {
        let edge = std::f64::consts::FRAC_PI_2 * R / f64::from(1u32 << lvl);
        let (mut total, mut viol) = (0, 0);
        let mut ex = Vec::new();
        for c in &coasts {
            let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
            let east = c.cross(helper).normalized();
            let north = c.cross(east).normalized();
            let mut seen = std::collections::BTreeSet::new();
            for j in -3i32..=3 { for i in -3i32..=3 {
                let d = (*c + east * (f64::from(i) * edge / R) + north * (f64::from(j) * edge / R)).normalized();
                let t = TileId::from_direction(&map, d, lvl);
                if !seen.insert(t.raw()) { continue; }
                total += 1;
                if wet(t) { continue; }
                // descendants two levels down
                let mut any = false;
                for k in t.children().unwrap() { for g in k.children().unwrap() { if wet(g) { any = true; } } }
                if any { viol += 1; if ex.len() < 3 { ex.push(format!("{:?}", t)); } }
            } }
        }
        eprintln!("L{lvl}: {total} tiles near coasts, {viol} dry while a grandchild is wet {:?}", ex);
    }
}
