//! SPIKE-17 (spike branch only): the owner's F9 bundle: the water verdict of every node of the plan the frame drew, and of the exact plans
//! along the recorded path.
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_frame::look::file::LookParams;
use planet_frame::pathlog::PathLog;
use planet_frame::water::{plan_water, WaterRequest, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_field, TerrainBundle, TerrainPlan};
use planet_generators::height::HeightField;

const R: f64 = 6_371_000.0;

fn verdicts(label: &str, plan: &TerrainPlan, req_lod: &planet_frame::LodParams, world: &WorldTerrain, field: &dyn HeightField, mins: &mut std::collections::HashMap<TileId, f64>, alt: f64) -> usize {
    let look = LookParams::default();
    let table = world.definition().liquids().unwrap();
    let mut cache = WaterTileCache::new();
    cache.diag_on = true;
    let _ = world.with_water(|f| {
        plan_water(&WaterRequest { source: f, map: &TangentWarp, liquids: &table, look: &look.water, lod: req_lod, time_s: 2.0, view: WaterView::Lit, cut: Some(field) }, &mut cache, plan)
    });
    let cam = plan.camera.position.0;
    let mut reasons = std::collections::BTreeMap::<&str, usize>::new();
    let mut gaps = Vec::new();
    for d in &cache.diag {
        *reasons.entry(d.reason).or_default() += 1;
        if d.reason == "kept" { continue; }
        let node = &plan.nodes[d.node as usize];
        let dist = (node.origin.0 - cam).length();
        if dist > (4.0 * alt).max(20_000.0) { continue; }
        let m = *mins.entry(d.tile).or_insert_with(|| {
            let mut m = f64::INFINITY;
            for l in 0..=8 { for k in 0..=8 { m = m.min(field.height(d.tile.sample_direction(&TangentWarp, 8, k, l))); } }
            m
        });
        if m < -0.1 { gaps.push((d.tile, node.level, d.reason, m, d.lowest_bound, d.exact_min, dist)); }
    }
    let mut levels = std::collections::BTreeMap::<u8, usize>::new();
    for n in &plan.nodes { *levels.entry(n.level).or_default() += 1; }
    println!("{label}: {} nodes, levels {:?}, reasons {:?}, {} gap nodes", plan.nodes.len(), levels, reasons, gaps.len());
    gaps.sort_by(|a, b| a.6.total_cmp(&b.6));
    for g in gaps.iter().take(8) {
        println!("   gap L{} {:?} {} truemin {:.1} loose {:.1} exact {:?} dist {:.0}", g.1, g.0, g.2, g.3, g.4, g.5, g.6);
    }
    gaps.len()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let b = TerrainBundle::from_text(&text).unwrap();
    let w = b.world.as_ref().expect("a world bundle");
    println!("bundle: field {} global_res {} seed {} generator {} mesh {} def_hash {:016x}", w.height_field, w.global_res, b.request.seed, b.generator_version, b.mesh_version, b.definition_hash);
    let def = planet_world_def::WorldDefinition::single_planet(R, b.request.seed);
    println!("this build's single_planet hash {:016x}", def.hash());
    let world = WorldTerrain::open(def, w.global_res).unwrap().bounding_water();
    let field = world.field().unwrap();
    let alt = b.request.camera.position.0.length() - R;
    println!("camera altitude (above radius) {alt:.1} m");
    let mut mins = Default::default();
    verdicts("drawn plan of the bundle", &b.plan, &b.request.lod, &world, field.as_ref(), &mut mins, alt.max(1.0));
    let exact = plan_terrain_field(&b.request, field.as_ref()).unwrap();
    println!("exact plan of the bundle's request: hash {:016x} (bundle {:016x}), {} nodes", exact.plan_hash(), b.plan.plan_hash(), exact.nodes.len());
    verdicts("exact plan", &exact, &b.request.lod, &world, field.as_ref(), &mut mins, alt.max(1.0));
    if let Some(p) = args.get(2) {
        let log = PathLog::from_text(&std::fs::read_to_string(p).unwrap()).unwrap();
        let mut total = 0;
        for (i, s) in log.samples().enumerate() {
            if i % 25 != 0 && i + 1 != log.len() { continue; }
            let req = planet_frame::TerrainRequest { camera: s.camera(&b.request.camera), ..b.request.clone() };
            let Ok(plan) = plan_terrain_field(&req, field.as_ref()) else { println!("pose {i} refused"); continue };
            total += verdicts(&format!("path pose {i} t {} ms alt {:.1}", s.t_ms, s.altitude_m), &plan, &b.request.lod, &world, field.as_ref(), &mut mins, s.altitude_m.max(1.0));
        }
        println!("path: {total} gap nodes over the sampled poses");
    }
}
