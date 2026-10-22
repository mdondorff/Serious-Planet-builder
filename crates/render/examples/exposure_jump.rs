//! Probe for P2-6 (a faint colour shift for under a second after a fast surface-to-orbit move): a cut from the ground to 400 km with the
//! adaptive exposure, the adapted EV100, its target and the frame's mean colour per frame, against the frame with the exposure snapped.
//!
//! `PLANET_ADAPTER=hardware cargo run --release -p planet-render --example exposure_jump`

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::look::file::LookParams;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::viewer::viewer_pre_ev100;
use planet_frame::{plan_terrain_cached, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, slot_bytes};
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (640, 360);
const CELLS: u32 = 32;

fn main() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let (lat, lon) = (20f64.to_radians(), 30f64.to_radians());
    let up = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
    let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
    let north = up.cross(east).normalized();
    let sun = (up * libm::sin(40f64.to_radians()) + (east * 0.7 - north * 0.7).normalized() * libm::cos(40f64.to_radians())).normalized();
    let look = LookParams::default();
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let poses = [(1.8, 5.0), (400_000.0, 35.0)];
    let mut plans = Vec::new();
    let mut r: Option<TerrainRenderer> = None;
    for (alt, pitch) in poses {
        let camera = Camera::at_surface_point(
            PlanetFixed(up * (R + alt)),
            Vec3::new(0.0, 0.0, 1.0),
            f64::to_radians(pitch),
            60f64.to_radians(),
            aspect,
            0.05,
        );
        let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 2.5, ..LodParams::earth_1080p() };
        let plan = plan_terrain_cached(
            &TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 },
            &field,
            &mut planet_frame::SelectionCache::new(),
        )
        .expect("plan");
        if r.is_none() {
            let block_slots = 8192;
            let pool = compact_format_pool_with((256 << 20) + 4 * u64::from(block_slots) * slot_bytes(CELLS), CELLS, block_slots, 1)
                .expect("pool");
            let mut rr = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, pool);
            rr.enable_lit(&ctx);
            rr.enable_atmosphere(&ctx, &planet_frame::look::atmosphere::Atmosphere::earth(R));
            r = Some(rr);
        }
        let rr = r.as_mut().unwrap();
        for n in &plan.nodes {
            if rr.slot_of(n.tile).is_none() {
                rr.upload_tile(&ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).expect("upload");
            }
        }
        plans.push(plan);
    }
    let r = r.as_mut().unwrap();
    let pre = viewer_pre_ev100(&look);
    let make = |plan: &planet_frame::TerrainPlan, reset: bool| {
        let mut lit = LitPlan::adaptive_with(&look, pre, 1.0 / 60.0).with_atmosphere(&look, R).expect("atmosphere plan");
        lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
        lit.reset_exposure = reset;
        let mut p = plan.clone();
        p.lit = Some(lit);
        p
    };
    let mean = |img: &planet_render::RgbaFrame| {
        let n = (img.width * img.height) as f64;
        let mut s = [0.0f64; 3];
        for px in img.rgba.chunks(4) {
            for c in 0..3 {
                s[c] += f64::from(px[c]) / 255.0;
            }
        }
        s.map(|v| v / n)
    };
    // The ground, adapted (snapped).
    for _ in 0..3 {
        r.render(&ctx, &make(&plans[0], true), false).expect("render");
    }
    let e0 = r.lit_path().expect("lit").read_exposure(&ctx);
    println!("ground, snapped: ev100 {:.3} target {:.3}", e0.ev100, e0.target_ev100);
    // The orbit frame with the exposure snapped: the converged picture.
    for _ in 0..3 {
        r.render(&ctx, &make(&plans[1], true), false).expect("render");
    }
    let snapped = mean(&r.read_color(&ctx));
    let es = r.lit_path().expect("lit").read_exposure(&ctx);
    println!(
        "orbit, snapped:  ev100 {:.3} target {:.3}  mean rgb {:.4} {:.4} {:.4}",
        es.ev100, es.target_ev100, snapped[0], snapped[1], snapped[2]
    );
    // The cut: back to the ground, then orbit with the adaptation running.
    for _ in 0..3 {
        r.render(&ctx, &make(&plans[0], true), false).expect("render");
    }
    for f in 0..120 {
        r.render(&ctx, &make(&plans[1], false), false).expect("render");
        let m = mean(&r.read_color(&ctx));
        let e = r.lit_path().expect("lit").read_exposure(&ctx);
        if f < 12 || f % 10 == 0 {
            println!(
                "frame {f:3} ({:.2} s): ev100 {:.3} target {:.3}  mean rgb {:.4} {:.4} {:.4}  b/r {:.3} (converged {:.3})",
                (f + 1) as f64 / 60.0,
                e.ev100,
                e.target_ev100,
                m[0],
                m[1],
                m[2],
                m[2] / m[0],
                snapped[2] / snapped[0]
            );
        }
    }
}
