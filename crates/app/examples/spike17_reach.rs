//! SPIKE-17 (spike branch only): water nodes kept and not kept near the camera of an owner bundle, against the distance of their centre to
//! the nearest ocean vertex of the bake (the reach of the coastal band, COAST_REACH_M).
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::look::file::LookParams;
use planet_frame::water::{plan_water, WaterRequest, WaterTileCache, WaterView};
use planet_frame::TerrainBundle;
const R: f64 = 6_371_000.0;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let b = TerrainBundle::from_text(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, b.request.seed), 256).unwrap().bounding_water();
    let field = world.field().unwrap();
    let look = LookParams::default();
    let table = world.definition().liquids().unwrap();
    let mut cache = WaterTileCache::new();
    cache.diag_on = true;
    let _ = world.with_water(|f| plan_water(&WaterRequest { source: f, map: &TangentWarp, liquids: &table, look: &look.water, lod: &b.request.lod, time_s: 2.0, view: WaterView::Lit, cut: Some(field.as_ref()) }, &mut cache, &b.plan));
    let cam = b.request.camera.position.0;
    let nearest = |c: Vec3| -> f64 {
        let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let east = c.cross(helper).normalized();
        let north = c.cross(east).normalized();
        let mut best = f64::INFINITY;
        for ring in 1..=60 {
            let d = f64::from(ring) * 2_500.0;
            for k in 0..48 {
                let ang = f64::from(k) * std::f64::consts::TAU / 48.0;
                let q = (c + (east * libm::cos(ang) + north * libm::sin(ang)) * (d / R)).normalized();
                if world.bake_probe(q).is_some_and(|p| p.ocean_vertex) { return d.min(best); }
            }
            best = best.min(f64::INFINITY);
        }
        best
    };
    let mut rows: Vec<(f64, &str, u8, f64)> = Vec::new();
    for d in &cache.diag {
        if d.reason != "kept" && d.reason != "no-water-answer" { continue; }
        let n = &b.plan.nodes[d.node as usize];
        let dist = (n.origin.0 - cam).length();
        if dist > 3_000.0 || n.level < 17 { continue; }
        rows.push((nearest(d.tile.center_direction(&TangentWarp)), d.reason, n.level, dist));
    }
    rows.sort_by(|x, y| x.0.total_cmp(&y.0));
    let (kept, dry): (Vec<_>, Vec<_>) = rows.iter().partition(|r| r.1 == "kept");
    let rng = |v: &Vec<&(f64, &str, u8, f64)>| (v.iter().map(|r| r.0).fold(f64::INFINITY, f64::min), v.iter().map(|r| r.0).fold(0.0, f64::max));
    println!("{} kept nodes within 3 km: nearest-ocean-vertex distance of their centres {:?} m; {} dry nodes: {:?} m", kept.len(), rng(&kept), dry.len(), rng(&dry));
}
