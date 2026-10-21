//! Review stills of the procedural ground detail (M7 G2, REND-034): the ground at an eye height of 1.8 m and 0.5 m, once with the natural
//! materials and once per material forced everywhere (a table that gives the material full weight on the reserved soil-depth input), each
//! with the detail off and on, written to `target/review/ground-detail/` as PNG for the owner's tuning. A dev tool: it draws the reference
//! world without an atmosphere at a fixed exposure.
//!
//! `cargo run --release -p planet-render --example ground_stills [strength]` (default strength 1).

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::look::materials::{Input, MaterialParams, INPUTS, MATERIALS, MATERIAL_NAMES};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, slot_bytes};
use planet_generators::height::{HeightField, Reference};
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (960, 540);
const CELLS: u32 = 32;
const FINEST: u8 = 26;
const EV: f32 = 14.5;

fn forced(m: usize) -> MaterialParams {
    let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
    curves[m][Input::SoilDepth as usize] = [-1.0, 0.0];
    MaterialParams { curves }
}

fn main() {
    let strength: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let out = std::path::Path::new("target/review/ground-detail");
    std::fs::create_dir_all(out).expect("output directory");
    let field = Reference(1);
    // The scripted ground site (20 N, 30 E), the sun 40 degrees above the horizon towards the east.
    let (lat, lon) = (20f64.to_radians(), 30f64.to_radians());
    let up = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
    let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
    let sun = (up * libm::sin(40f64.to_radians()) + east * libm::cos(40f64.to_radians())).normalized();
    let ground = field.height(up);
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    for (eye, pitch_deg) in [(1.8, 6.0), (0.5, 25.0)] {
        let camera = Camera::at_surface_point(
            PlanetFixed(up * (R + ground + eye)),
            Vec3::new(0.0, 0.0, 1.0),
            f64::to_radians(pitch_deg),
            60f64.to_radians(),
            aspect,
            0.05,
        );
        let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
        let plan =
            plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 })
                .expect("plan");
        let block_slots = (plan.nodes.len() as u32 + 64).max(1024);
        let pool =
            compact_format_pool_with((256 << 20) + 4 * u64::from(block_slots) * slot_bytes(CELLS), CELLS, block_slots, 1).expect("pool");
        let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, pool);
        r.enable_lit(&ctx);
        for n in &plan.nodes {
            if r.slot_of(n.tile).is_none() {
                r.upload_tile(&ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).expect("upload");
            }
        }
        let mut variants: Vec<(String, MaterialParams, f64)> =
            vec![("natural-off".into(), MaterialParams::EARTH, 0.0), ("natural".into(), MaterialParams::EARTH, strength)];
        for (m, name) in MATERIAL_NAMES.iter().enumerate() {
            variants.push(((*name).to_string(), forced(m), strength));
        }
        for (name, materials, s) in variants {
            let mut lit = LitPlan::fixed(EV);
            lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
            lit.materials = materials;
            lit.ground_detail = s;
            let mut p = plan.clone();
            p.lit = Some(lit);
            for _ in 0..2 {
                r.render(&ctx, &p, false).expect("render");
            }
            let file = out.join(format!("eye{eye}m_{name}.png"));
            r.read_color(&ctx).write_png(&file).expect("png");
            println!("wrote {} ({} nodes)", file.display(), plan.nodes.len());
        }
    }
}
