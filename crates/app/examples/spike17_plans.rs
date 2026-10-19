//! SPIKE-17 (spike branch only): exact plans from poses over the sea near the coasts of the default world, the water plan's verdict per
//! node: a node whose terrain lies under the sea (9x9 samples) and that has no water node is a gap.
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::litscenes::{coast_camera, Coast};
use planet_frame::look::file::LookParams;
use planet_frame::water::{plan_water, WaterRequest, WaterSource, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;

const R: f64 = 6_371_000.0;

fn main() {
    let seed: u64 = std::env::var("SP17_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let t0 = std::time::Instant::now();
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, seed), 256).expect("world").bounding_water();
    let field = world.field().unwrap();
    let level = 0.0;
    let is_sea = |d: Vec3| world.with_water(|f| WaterSource::water_at(f, d).ok().flatten().is_some()).unwrap().unwrap();
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
            if prev != cur && found < 3 {
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
    eprintln!("{} coasts {:?}", coasts.len(), t0.elapsed());
    let size = (1280u32, 720u32);
    let aspect = f64::from(size.0) / f64::from(size.1);
    let lod = LodParams { cells: 16, viewport_h_px: 2.0 * f64::from(size.1), tau_px: 4.0, max_level: 26, max_height_m: 40_000.0, ..LodParams::earth_1080p() };
    let look = LookParams::default();
    let table = world.definition().liquids().unwrap();
    let map = TangentWarp;
    let bounded = planet_generators::water::WaterBounded::borrowed(field.as_ref(), level);
    let mut mins: std::collections::HashMap<TileId, f64> = Default::default();
    for alt in [60.0f64, 600.0, 6_000.0, 60_000.0] {
        let (mut poses, mut with_gap, mut gap_nodes) = (0, 0, 0);
        let mut shown = 0;
        for c in &coasts {
            let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
            let east = c.cross(helper).normalized();
            let north = c.cross(east).normalized();
            for heading in [north, east, north * -1.0, east * -1.0] {
                // camera `back` metres out to sea on the side opposite to the heading
                let coast = Coast { dir: *c, heading };
                let back = 4.0 * alt + 500.0;
                let cam = coast_camera(&coast, R, level, alt, back, 0.0, 8.0, aspect);
                let cdir = cam.position.0.normalized();
                if !is_sea(cdir) { continue; }
                let Ok(plan) = plan_terrain_field(
                    &TerrainRequest { camera: cam, size, view: TerrainView::Level, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed },
                    &bounded,
                ) else { continue };
                let mut cache = WaterTileCache::new();
                cache.diag_on = true;
                let _ = world.with_water(|f| {
                    plan_water(&WaterRequest { source: f, map: &map, liquids: &table, look: &look.water, lod: &lod, time_s: 2.0, view: WaterView::Lit, cut: Some(field.as_ref()) }, &mut cache, &plan)
                });
                poses += 1;
                let mut gaps = Vec::new();
                for d in &cache.diag {
                    if d.reason == "kept" || d.reason == "cut-horizon" { continue; }
                    let node = &plan.nodes[d.node as usize];
                    let dist = (node.origin.0 - cam.position.0).length();
                    if dist > 10.0 * alt + 2_000.0 { continue; }
                    let m = *mins.entry(d.tile).or_insert_with(|| {
                        let mut m = f64::INFINITY;
                        for l in 0..=8 { for k in 0..=8 { m = m.min(field.height(d.tile.sample_direction(&map, 8, k, l))); } }
                        m
                    });
                    if m < level - 2.0 { gaps.push((d.tile, d.reason, m, dist)); }
                }
                if !gaps.is_empty() {
                    with_gap += 1;
                    gap_nodes += gaps.len();
                    if shown < 4 {
                        shown += 1;
                        eprintln!("  alt {alt}: pose at {:?}: {} gap nodes, e.g. {:?}", cdir, gaps.len(), gaps.iter().take(3).map(|g| format!("L{} {} min {:.1} dist {:.0}", g.0.level(), g.1, g.2, g.3)).collect::<Vec<_>>());
                    }
                }
            }
        }
        eprintln!("alt {alt}: {poses} poses, {with_gap} with gap nodes ({gap_nodes} nodes) {:?}", t0.elapsed());
    }
}
