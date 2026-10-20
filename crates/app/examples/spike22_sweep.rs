//! SPIKE-22 (spike branch only): the window's BASE LOD over the real eroded world: orbital-to-ground poses, how many does the plan refuse?
use planet::worldfield::WorldTerrain;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
const R: f64 = 6_371_000.0;
fn main() {
    let seed: u64 = std::env::var("SP22_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let n: usize = std::env::var("SP22_N").ok().and_then(|v| v.parse().ok()).unwrap_or(3000);
    let size: (u32, u32) = (std::env::var("SP22_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280), std::env::var("SP22_H").ok().and_then(|v| v.parse().ok()).unwrap_or(720));
    let coarse = std::env::var("SP22_COARSE").is_ok();
    let def = planet_world_def::WorldDefinition::single_planet(R, seed);
    let world = match std::env::var("SP22_STORE") {
        Ok(s) => WorldTerrain::open_store(def, std::path::Path::new(&s), 16384).unwrap(),
        Err(_) => WorldTerrain::open(def, 256).unwrap(),
    };
    let real = world.field().unwrap();
    let stub = Bound9735;
    let field: &dyn HeightField = if std::env::var("SP22_STUB").is_ok() { &stub } else { real.as_ref() };
    // the window's LodParams (window.rs run_window): tau scales with the window height
    let tau = 6.0 * f64::from(size.1) / 1440.0;
    let mut base = LodParams { cells: 32, viewport_h_px: f64::from(size.1), tau_px: tau, radius_m: R, max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()), ..LodParams::earth_1080p() };
    base.max_level = 22;
    base.validate().unwrap();
    let lod = if coarse { base.coarse() } else { base };
    println!("max_abs_height {:.0} m, viewport/tau {:.1}, max_level {}", field.max_abs_height_m(), lod.viewport_h_px / lod.tau_px, lod.max_level);
    let mut s = 0x9e3779b97f4a7c15u64;
    let mut next = move || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; (s >> 11) as f64 / (1u64 << 53) as f64 };
    let bands = [(0.0, 100.0), (100.0, 1_000.0), (1_000.0, 10_000.0), (10_000.0, 31_000.0), (31_000.0, 100_000.0), (100_000.0, 1.0e6), (1.0e6, 1.0e9)];
    let mut tally = vec![(0usize, 0usize); bands.len()];
    let mut first: Vec<String> = Vec::new();
    let mut levels = std::collections::BTreeMap::<String, usize>::new();
    for _ in 0..n {
        let alt = 2.0 * libm::pow(10.0, next() * 7.2);
        let (z, a) = (2.0 * next() - 1.0, std::f64::consts::TAU * next());
        let r = libm::sqrt(1.0 - z * z);
        let dir = Vec3::new(r * libm::cos(a), r * libm::sin(a), z);
        let heading = dir.cross(Vec3::new(libm::cos(a * 3.0), libm::sin(a * 3.0), 0.3)).normalized();
        let pitch = if next() < 0.5 { 0.05 + next() * 1.3 } else { -(0.05 + next() * 1.3) };
        let cam = Camera::at_surface_point(PlanetFixed(dir * (R + alt)), heading, pitch, 60f64.to_radians(), f64::from(size.0) / f64::from(size.1), 0.1);
        let bi = bands.iter().position(|b| alt >= b.0 && alt < b.1).unwrap();
        tally[bi].0 += 1;
        match plan_terrain_field(&TerrainRequest { camera: cam, size, view: TerrainView::Level, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed }, field) {
            Ok(_) => {}
            Err(e) => {
                tally[bi].1 += 1;
                let m = e.to_string();
                let key = m.split("(level ").nth(1).and_then(|x| x.split(')').next()).map_or("other".to_string(), |l| format!("level {l}"));
                *levels.entry(key).or_default() += 1;
                if first.len() < 3 { first.push(format!("alt {alt:.0} m: {m}")); }
            }
        }
    }
    for (b, t) in bands.iter().zip(&tally) { println!("altitude {:>10.0} to {:>10.0} m: {:>4} poses, {:>3} refused", b.0, b.1, t.0, t.1); }
    println!("refused by level: {levels:?}");
    for f in first { println!("{f}"); }
}

/// A field with only the global bound (what the eroded field's tile bounds fall back to for a field without tile bounds).
struct Bound9735;
impl HeightField for Bound9735 {
    fn height(&self, _: Vec3) -> f64 { 0.0 }
    fn seed(&self) -> u64 { 1 }
    fn implementation_id(&self) -> &'static str { "bound-9735" }
    fn version(&self) -> u32 { 1 }
    fn max_abs_height_m(&self) -> f64 { 9735.0 }
}
