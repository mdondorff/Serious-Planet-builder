//! SPIKE-17 (spike branch only): near the coasts of the editor's default world (seed 1, 256-cell bake), tiles of levels 8 to 15 whose
//! terrain lies under the sea level but whose water answer is dry (a whole tile with no water node).
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::water::{WaterSource, WaterTileCache};
use planet_generators::height::HeightField;

const R: f64 = 6_371_000.0;

fn main() {
    let seed: u64 = std::env::var("SP17_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let res: u32 = std::env::var("SP17_RES").ok().and_then(|v| v.parse().ok()).unwrap_or(256);
    let t0 = std::time::Instant::now();
    let world = (if let Ok(s) = std::env::var("SP17_STORE") { WorldTerrain::open_store(planet_world_def::WorldDefinition::single_planet(R, seed), std::path::Path::new(&s), 16384) } else { WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, seed), res) }).expect("world").bounding_water();
    let field = world.field().unwrap();
    eprintln!("world open {:?}", t0.elapsed());
    let level = world.definition().sea(planet_world_def::BodyId::PLANET).unwrap().level_m;
    let is_sea = |d: Vec3| world.with_water(|f| WaterSource::water_at(f, d).ok().flatten().is_some()).unwrap().unwrap();
    // coast points along parallels
    let mut coasts: Vec<Vec3> = Vec::new();
    for lat_deg in (-17..=17).map(|i| f64::from(i) * 5.0) {
        let lat = lat_deg.to_radians();
        let at = |s: f64| {
            let lon = s / (R * libm::cos(lat));
            Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
        };
        let steps = (std::f64::consts::TAU * R * libm::cos(lat) / 20_000.0) as usize;
        let mut prev = is_sea(at(0.0));
        let mut found = 0;
        for k in 1..steps {
            let cur = is_sea(at(k as f64 * 20_000.0));
            if prev != cur && found < 8 {
                let (mut a, mut b) = ((k - 1) as f64 * 20_000.0, k as f64 * 20_000.0);
                for _ in 0..20 {
                    let m = 0.5 * (a + b);
                    if is_sea(at(m)) == prev { a = m } else { b = m }
                }
                coasts.push(at(0.5 * (a + b)));
                found += 1;
            }
            prev = cur;
        }
    }
    eprintln!("{} coast points {:?}", coasts.len(), t0.elapsed());
    let map = TangentWarp;
    for lvl in [6u8, 7, 8, 9, 10] {
        let edge = std::f64::consts::FRAC_PI_2 * R / f64::from(1u32 << lvl);
        let (mut total, mut bad, mut bad_deep, mut differ) = (0usize, 0usize, 0usize, 0usize);
        let mut samples: Vec<String> = Vec::new();
        for c in &coasts {
            let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
            let east = c.cross(helper).normalized();
            let north = c.cross(east).normalized();
            let mut seen = std::collections::BTreeSet::new();
            let n = 4i32;
            for j in -n..=n {
                for i in -n..=n {
                    let d = (*c + east * (f64::from(i) * edge / R) + north * (f64::from(j) * edge / R)).normalized();
                    let t = TileId::from_direction(&map, d, lvl);
                    if !seen.insert(t.raw()) { continue; }
                    let ans = world.with_water(|f| WaterTileCache::new().tile_water(f, &map, t, R)).unwrap().unwrap().unwrap();
                    let pt = world.with_water(|f| WaterTileCache::new().tile_water(&NoSummary(f), &map, t, R)).unwrap().unwrap().unwrap();
                    if ans.is_none() != pt.is_none() { differ += 1; }
                    let mut m = f64::INFINITY;
                    for l in 0..=8 { for k in 0..=8 { m = m.min(field.height(t.sample_direction(&map, 8, k, l))); } }
                    total += 1;
                    if ans.is_none() && m < level - 2.0 {
                        bad += 1;
                        if m < level - 10.0 { bad_deep += 1; }
                        if samples.len() < 6 { samples.push(format!("{:?} min {m:.1}", t)); }
                    }
                }
            }
        }
        eprintln!("L{lvl}: summary vs point-by-point answers differ on {differ} tiles");
        eprintln!("L{lvl}: {total} tiles near coasts, {bad} dry with terrain >2 m under the sea, {bad_deep} of them >10 m under; {:?}  {:?}", samples, t0.elapsed());
    }
}

/// The same field without the coarse summary: every tile is answered point by point.
pub struct NoSummary<'a>(pub &'a dyn WaterSource);
impl WaterSource for NoSummary<'_> {
    fn water_at(&self, d: Vec3) -> Result<Option<planet_generators::water::WaterSample>, planet_generators::global::ReadError> { self.0.water_at(d) }
    fn band_at(&self, d: Vec3) -> Result<Option<planet_generators::water::WaterSample>, planet_generators::global::ReadError> { self.0.band_at(d) }
    fn ocean_bit_at(&self, d: Vec3) -> Result<bool, planet_generators::global::ReadError> { self.0.ocean_bit_at(d) }
}
