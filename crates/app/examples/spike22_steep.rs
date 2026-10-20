//! SPIKE-22 (spike branch only): the real eroded world's steepest level-12 tiles by their height bounds; the window's base LOD from
//! poses over them.
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::lod::tile_extent;
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
const R: f64 = 6_371_000.0;
fn main() {
    let level: u8 = std::env::var("SP22_LEVEL").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    let def = planet_world_def::WorldDefinition::single_planet(R, 1);
    let world = match std::env::var("SP22_STORE") {
        Ok(s) => WorldTerrain::open_store(def, std::path::Path::new(&s), 16384).unwrap(),
        Err(_) => WorldTerrain::open(def, 256).unwrap(),
    };
    let field = world.field().unwrap();
    let size = (1280u32, 720u32);
    let lod = LodParams { cells: 32, viewport_h_px: 720.0, tau_px: 3.0, radius_m: R, max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()), max_level: 22, ..LodParams::earth_1080p() };
    let mut s = 0x2545f4914f6cdd1du64;
    let mut next = move || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; (s >> 11) as f64 / (1u64 << 53) as f64 };
    let mut tiles: Vec<(f64, TileId, f64, f64)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..40_000 {
        let (z, a) = (2.0 * next() - 1.0, std::f64::consts::TAU * next());
        let r = libm::sqrt(1.0 - z * z);
        let d = Vec3::new(r * libm::cos(a), r * libm::sin(a), z);
        let t = TileId::from_direction(&TangentWarp, d, level);
        if !seen.insert(t.raw()) { continue; }
        let e = tile_extent(&TangentWarp, t, &lod);
        let (lo, hi) = field.height_bounds(e.dir, e.angular_radius);
        tiles.push((hi - lo, t, lo, hi));
    }
    tiles.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (sd, nrb) = (lod.split_distance(level - 1), lod.node_radius_bound(level));
    let rho_max = 0.225 * sd;
    let h_max = if rho_max > nrb { libm::sqrt(rho_max * rho_max - nrb * nrb) } else { 0.0 };
    println!("theory: the plan refuses a level-{level} tile whose height half-range exceeds {h_max:.1} m (extent {:.1} m)", 2.0 * h_max);
    println!("{} level-{level} tiles sampled; extent of the steepest {:.0} m ([{:.0}, {:.0}]), median {:.0} m", tiles.len(), tiles[0].0, tiles[0].2, tiles[0].3, tiles[tiles.len() / 2].0);
    let (mut poses, mut refused) = (0, 0);
    let mut first = String::new();
    for (ext, t, _, _) in tiles.iter().take(40) {
        let dir = t.center_direction(&TangentWarp);
        let sd = lod.split_distance(level - 1);
        for alt in [0.2 * sd, 0.5 * sd, 0.9 * sd, 1.5 * sd, 3.0 * sd].map(|a| a.max(2.0)) {
            for k in 0..8 {
                let a = f64::from(k) * std::f64::consts::TAU / 8.0;
                let helper = Vec3::new(0.0, 0.0, 1.0);
                let east = dir.cross(helper).normalized();
                let north = dir.cross(east).normalized();
                let heading = east * libm::cos(a) + north * libm::sin(a);
                let cam = Camera::at_surface_point(PlanetFixed(dir * (R + alt)), heading, 0.4, 60f64.to_radians(), 1280.0 / 720.0, 0.1);
                poses += 1;
                if let Err(e) = plan_terrain_field(&TerrainRequest { camera: cam, size, view: TerrainView::Level, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 1 }, field.as_ref()) {
                    refused += 1;
                    if first.is_empty() { first = format!("tile {t:?} extent {ext:.0} m alt {alt}: {e}"); }
                }
            }
        }
    }
    println!("steepest 40 tiles: {poses} poses, {refused} refused {first}");
}
