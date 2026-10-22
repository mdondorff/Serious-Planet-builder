//! Probe for P2-7 (a colour step between 95 and 100 km): the same view at every altitude of a climb, the mean colour of the sky part and the
//! ground part of the frame printed with its step from the previous altitude. `PLANET_ADAPTER=hardware` for the discrete GPU.
//!
//! `cargo run --release -p planet-render --example atmo_climb [from_km to_km step_km pitch_deg]`

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::look::file::LookParams;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain_cached, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, slot_bytes};
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (640, 360);
const CELLS: u32 = 32;

fn main() {
    let a: Vec<f64> = std::env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    let (from, to, step, pitch) =
        (*a.first().unwrap_or(&90.0), *a.get(1).unwrap_or(&105.0), *a.get(2).unwrap_or(&1.0), *a.get(3).unwrap_or(&20.0));
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let (lat, lon) = (20f64.to_radians(), 30f64.to_radians());
    let up = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
    let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
    let north = up.cross(east).normalized();
    let sun = (up * libm::sin(40f64.to_radians()) + (east * 0.7 - north * 0.7).normalized() * libm::cos(40f64.to_radians())).normalized();
    let look = LookParams::default();
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let mut last: Option<[f64; 6]> = None;
    let mut prev: Option<Vec<u8>> = None;
    let mut km = from;
    while km <= to + 1e-9 {
        let camera = Camera::at_surface_point(
            PlanetFixed(up * (R + km * 1000.0)),
            Vec3::new(0.0, 0.0, 1.0),
            f64::to_radians(pitch),
            60f64.to_radians(),
            aspect,
            100.0,
        );
        let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 2.5, ..LodParams::earth_1080p() };
        let plan = plan_terrain_cached(
            &TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 },
            &field,
            &mut planet_frame::SelectionCache::new(),
        )
        .expect("plan");
        let block_slots = (plan.nodes.len() as u32 + 64).max(1024);
        let pool =
            compact_format_pool_with((256 << 20) + 4 * u64::from(block_slots) * slot_bytes(CELLS), CELLS, block_slots, 1).expect("pool");
        let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, pool);
        r.enable_lit(&ctx);
        r.enable_atmosphere(&ctx, &planet_frame::look::atmosphere::Atmosphere::earth(R));
        for n in &plan.nodes {
            if r.slot_of(n.tile).is_none() {
                r.upload_tile(&ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).expect("upload");
            }
        }
        let mut lit = LitPlan::fixed(14.5).with_atmosphere(&look, R).expect("atmosphere plan");
        lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
        let mut p = plan.clone();
        p.lit = Some(lit);
        for _ in 0..2 {
            r.render(&ctx, &p, false).expect("render");
        }
        let img = r.read_color(&ctx);
        let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
        let mut sums = [[0.0f64; 3]; 2];
        let mut n = [0usize; 2];
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 4;
                let px = [img.rgba[o], img.rgba[o + 1], img.rgba[o + 2]];
                let k = usize::from(y >= h / 2);
                for c in 0..3 {
                    sums[k][c] += f64::from(px[c]) / 255.0;
                }
                n[k] += 1;
            }
        }
        let m = [
            sums[0][0] / n[0] as f64,
            sums[0][1] / n[0] as f64,
            sums[0][2] / n[0] as f64,
            sums[1][0] / n[1] as f64,
            sums[1][1] / n[1] as f64,
            sums[1][2] / n[1] as f64,
        ];
        let d = last.map(|l| (0..6).map(|i| m[i] - l[i]).fold(0.0f64, |a, b| if b.abs() > a.abs() { b } else { a })).unwrap_or(0.0);
        println!(
            "{km:7.1} km  upper rgb {:.4} {:.4} {:.4}  lower rgb {:.4} {:.4} {:.4}  max step {d:+.4}",
            m[0], m[1], m[2], m[3], m[4], m[5]
        );
        if let Some(pv) = &prev {
            let (mut worst, mut at) = (0i32, (0usize, 0usize));
            let mut big = 0usize;
            let mut skyworst = 0i32;
            let mut rows = [0usize; 12];
            for (i, (a, b)) in img.rgba.iter().zip(pv).enumerate() {
                if i % 4 == 3 {
                    continue;
                }
                let dd = (i32::from(*a) - i32::from(*b)).abs();
                if dd > 3 {
                    big += 1;
                    rows[((i / 4) / w) * 12 / h] += 1;
                }
                if (i / 4) / w < h * 4 / 12 + 8 && dd > skyworst {
                    skyworst = dd;
                }
                if dd > worst {
                    worst = dd;
                    at = ((i / 4) % w, (i / 4) / w);
                }
            }
            println!("         pixel step vs previous: max {worst}/255 at {at:?}, channels over 3/255: {big}, sky rows max {skyworst}, by row twelfth {rows:?}");
        }
        prev = Some(img.rgba.clone());
        last = Some(m);
        km += step;
    }
}
