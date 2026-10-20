//! SPIKE-19 (spike branch only): the window's LodParams (coarse and base) over the eroded field of earth.toml (seed 5, 128-cell bake):
//! which orbital poses does the plan refuse?
use planet::worldfield::WorldTerrain;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
const R: f64 = 6_371_000.0;
fn main() {
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, 5), 128).unwrap();
    let field = world.field().unwrap();
    let size = (320u32, 200u32);
    let tau = 6.0 * f64::from(size.1) / 1440.0;
    let base = LodParams { cells: 32, viewport_h_px: f64::from(size.1), tau_px: tau, max_level: 22, radius_m: R, max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()), ..LodParams::earth_1080p() };
    base.validate().unwrap();
    let coarse = base.coarse();
    println!("base ratio viewport/tau {:.2}; coarse factor {:.5}, ratio {:.3}; max_abs_height {:.0} m", base.viewport_h_px / base.tau_px, base.coarse_factor(), coarse.viewport_h_px / coarse.tau_px, field.max_abs_height_m());
    let (s4, e4) = coarse.morph_interval(4);
    println!("coarse own morph_interval(4) [{s4:.1}, {e4:.1}] ratio {:.6} (the plan refuses start > 0.95 end)", s4 / e4);
    for (name, lod) in [("base", &base), ("coarse", &coarse)] {
        let (mut ok, mut refused) = (0, 0);
        let mut first = String::new();
        for pitch in [-1.2f64, 1.2] {
        for i in 0..400 {
            let dir = Vec3::new(libm::cos(i as f64 * 0.37) * libm::cos(i as f64 * 0.11), libm::sin(i as f64 * 0.37) * libm::cos(i as f64 * 0.11), libm::sin(i as f64 * 0.11)).normalized();
            let alt = 3.0e5 * libm::pow(1.012, f64::from(i));
            let cam = Camera::at_surface_point(PlanetFixed(dir * (R + alt)), dir.cross(Vec3::new(0.0, 0.0, 1.0)).normalized(), pitch, 60f64.to_radians(), f64::from(size.0) / f64::from(size.1), 0.1);
            match plan_terrain_field(&TerrainRequest { camera: cam, size, view: TerrainView::Level, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 5 }, field.as_ref()) {
                Ok(_) => ok += 1,
                Err(e) => { refused += 1; if first.is_empty() { first = format!("alt {alt:.0} m dir {dir:?}: {e}"); } }
            }
        }
        }
        println!("{name}: {ok} poses planned, {refused} refused {first}");
    }
}
