//! SPIKE-18 (spike branch only): the EXACT plan from a camera approaching a point; does any probe's level ever drop while it nears?
use planet::worldfield::WorldTerrain;
use planet_core::Vec3;
use planet_frame::litscenes::{coast_camera, Coast};
use planet_frame::{plan_terrain_field, LodParams, TerrainRequest, TerrainView};
const R: f64 = 6_371_000.0;
fn main() {
    let rate: f64 = std::env::var("SP18_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(0.01);
    let pitch: f64 = std::env::var("SP18_PITCH").ok().and_then(|v| v.parse().ok()).unwrap_or(75.0);
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, 1), 256).unwrap().bounding_water();
    let field = world.field().unwrap();
    let size = (1920u32, 1080u32);
    let aspect = f64::from(size.0) / f64::from(size.1);
    let lod = LodParams { cells: 32, viewport_h_px: f64::from(size.1), tau_px: 4.0, max_level: 22, max_height_m: 40_000.0, ..LodParams::earth_1080p() };
    let e = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let dir = Vec3::new(e("SP18_X", 0.3), e("SP18_Y", 0.5), e("SP18_Z", 0.8)).normalized();
    let helper = Vec3::new(0.0, 0.0, 1.0);
    let heading = dir.cross(helper).cross(dir).normalized();
    let coast = Coast { dir, heading };
    let mut det = planet::coarsen::Detector::open(R).expect("COARSEN_LOG");
    det.densify(dir, 0.35, 3000);
    let mut alt = 20_000_000.0f64;
    let mut k = 0u64;
    while alt > 3_000.0 {
        let cam = coast_camera(&coast, R, 0.0, alt, 0.0, 0.0, pitch, aspect);
        let plan = plan_terrain_field(&TerrainRequest { camera: cam, size, view: TerrainView::Level, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 1 }, field.as_ref());
        match plan {
            Ok(p) => det.frame(k, "Exact", 0, 0, cam.position.0, &p),
            Err(e) => eprintln!("step {k} alt {alt:.0}: plan refused: {e}"),
        }
        alt *= 1.0 - rate;
        k += 1;
    }
    eprintln!("steps {k}: violations {} frames with {}; pairs {} finer {} same {}", det.violations, det.frames_with, det.pairs, det.finer, det.same);
}
