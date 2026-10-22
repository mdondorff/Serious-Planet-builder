//! Indicative GPU timing of the ground detail (M7 G2c): the lit terrain pass of the ground at an eye height of 1.8 m and 0.5 m at 2560x1440,
//! with the detail off and on, from GPU timestamps of waited frames in one process. Not a measurement (no readiness gate, other programs may run):
//! it compares builds of the shader on the same machine within minutes. Terrain only: no atmosphere, no water, the reference world.
//!
//! `PLANET_ADAPTER=hardware cargo run --release -p planet-render --example ground_bench [strength]` (default 0.6; without the variable it runs on the software adapter, whose times mean nothing).

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, slot_bytes};
use planet_generators::height::{HeightField, Reference};
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);
const CELLS: u32 = 32;
const FINEST: u8 = 22;
const EV: f32 = 14.5;
const WARM: usize = 30;
const FRAMES: usize = 200;

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

fn main() {
    let strength: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0.6);
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let (lat, lon) = (20f64.to_radians(), 30f64.to_radians());
    let up = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
    let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
    let north = up.cross(east).normalized();
    let sun = (up * libm::sin(30f64.to_radians()) + north * libm::cos(30f64.to_radians())).normalized();
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
        let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 6.0, max_level: FINEST, ..LodParams::earth_1080p() };
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
        let mut line = format!("eye {eye} m, {} nodes:", plan.nodes.len());
        for s in [0.0, strength] {
            let mut lit = LitPlan::fixed(EV);
            lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
            lit.ground_detail = s;
            let mut p = plan.clone();
            p.lit = Some(lit);
            for _ in 0..WARM {
                r.render(&ctx, &p, false).expect("render");
            }
            let mut gpu = Vec::new();
            for _ in 0..FRAMES {
                let (t, _) = r.render(&ctx, &p, true).expect("render");
                gpu.push(t.gpu_ms.unwrap_or(f64::NAN));
            }
            line += &format!("  strength {s}: gpu median {:.3} ms", median(&mut gpu));
        }
        println!("{line}");
    }
}
