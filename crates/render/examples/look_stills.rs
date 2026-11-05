//! Review stills of the ground look (M7 GT, REND-034, REND-035, VIEW-050): the lit path with the atmosphere over a site with steep ground (a hill, a
//! ridge and a crater made of authored stamps, so rock appears on its own on the flanks) and the scale references dropped in front of the camera,
//! at an eye height of 1.8 m, 0.5 m and 30 m, once with the natural materials at the window's strength and once per material forced everywhere, at
//! full strength. Written to `target/review/ground-detail/gt_eye<h>m_<variant>.png`. A dev tool; the owner looks, nothing here is a golden.
//!
//! `cargo run --release -p planet-render --example look_stills` (`PLANET_ADAPTER=hardware` for the discrete GPU).

use std::sync::{Arc, RwLock};

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::look::atmosphere::Atmosphere;
use planet_frame::look::file::LookParams;
use planet_frame::look::materials::{DetailCoefficients, Input, MaterialParams, INPUTS, MATERIALS, MATERIAL_NAMES, NATURAL_STRENGTH};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::scalerefs::drop_references;
use planet_frame::{plan_terrain_cached, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, slot_bytes};
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::StampStage;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (1280, 720);
const CELLS: u32 = 32;
const FINEST: u8 = 26;
const EV: f32 = 14.5;

fn forced(m: usize) -> MaterialParams {
    let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
    curves[m][Input::SoilDepth as usize] = [-1.0, 0.0];
    MaterialParams { curves }
}

/// The Earth coefficients with the 64 m and 8 m octaves of REND-057 on (`which`: bit 0 the 64 m one, bit 1 the 8 m one): the 64 m one as broad albedo
/// patches of the ground, the 8 m one as mounds and the faces of the rock (starting values for the owner's look sheet, not tuned).
fn octaves_on(which: u32) -> DetailCoefficients {
    let mut c = DetailCoefficients::EARTH;
    if which & 1 != 0 {
        let o = &mut c.octaves[4];
        o.albedo = [0.12, 0.10, 0.08];
        o.grass = [0.14, 0.14, 0.12];
        o.scree = [0.12, 0.0];
        o.wet = [0.08, 0.0];
        o.tilt = [0.02, 0.01, 0.02, 0.02];
    }
    if which & 2 != 0 {
        let o = &mut c.octaves[5];
        o.albedo = [0.10, 0.04, 0.06];
        o.grass = [0.10, 0.12, 0.08];
        o.scree = [0.10, 0.12];
        o.wet = [0.05, 0.04];
        o.tilt = [0.18, 0.05, 0.06, 0.08];
    }
    c
}

fn item(id: u128, at: Vec3, reach_m: f64, kind: StampKind, amount: f64) -> ItemSpec {
    ItemSpec {
        id,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: at,
        content_hash: 0,
        reach_m,
        max_delta_m: amount.abs() + 1.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(kind, [amount, 0.0, 0.0]))
}

fn main() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let out = std::path::Path::new("target/review/ground-detail");
    std::fs::create_dir_all(out).expect("output directory");
    // The scripted ground site (20 N, 30 E), facing north, the sun 40 degrees up behind the camera's right shoulder (so the slopes ahead are lit).
    let (lat, lon) = (20f64.to_radians(), 30f64.to_radians());
    let up = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
    let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
    let north = up.cross(east).normalized();
    let sun = (up * libm::sin(40f64.to_radians()) + (east * 0.7 - north * 0.7).normalized() * libm::cos(40f64.to_radians())).normalized();
    let at = |ahead_m: f64, right_m: f64| (up * R + north * ahead_m + east * right_m).normalized();
    // The steep ground: a hill to the right, a bigger one beyond to the left, a crater between, all with flanks over 40 degrees at their steepest.
    let mut def = WorldDefinition::single_planet(R, 1);
    for spec in [
        item(1, at(220.0, 90.0), 90.0, StampKind::Hill, 120.0),
        item(2, at(450.0, -200.0), 220.0, StampKind::Hill, 280.0),
        item(3, at(90.0, -45.0), 70.0, StampKind::Crater, 25.0),
    ] {
        def.put_item_spec(BodyId::PLANET, spec).expect("an item");
    }
    let base: Arc<dyn HeightField> = Arc::new(Reference(1));
    let field = StampStage::new(base, Arc::new(RwLock::new(def)), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1).expect("a stage");
    let ground = field.height(up);
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let look = LookParams::default();
    for (eye, pitch_deg) in [(1.8, 5.0), (0.5, 10.0), (30.0, 14.0)] {
        let camera = Camera::at_surface_point(
            PlanetFixed(up * (R + ground + eye)),
            Vec3::new(0.0, 0.0, 1.0),
            f64::to_radians(pitch_deg),
            60f64.to_radians(),
            aspect,
            0.05,
        );
        let lod = LodParams {
            cells: CELLS,
            viewport_h_px: f64::from(SIZE.1),
            tau_px: 2.5,
            max_level: FINEST,
            max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m() + 10.0),
            ..LodParams::earth_1080p()
        };
        let plan = plan_terrain_cached(
            &TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 },
            &field,
            &mut SelectionCache::new(),
        )
        .expect("plan");
        let block_slots = (plan.nodes.len() as u32 + 64).max(1024);
        let pool =
            compact_format_pool_with((256 << 20) + 4 * u64::from(block_slots) * slot_bytes(CELLS), CELLS, block_slots, 1).expect("pool");
        let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, pool);
        r.enable_lit(&ctx);
        r.enable_atmosphere(&ctx, &Atmosphere::earth(R));
        for n in &plan.nodes {
            if r.slot_of(n.tile).is_none() {
                r.upload_tile(&ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).expect("upload");
            }
        }
        r.set_scale_refs(&drop_references(camera.position, camera.forward, R, &|d| field.height(d)));
        let e = DetailCoefficients::EARTH;
        let mut variants: Vec<(String, MaterialParams, f64, DetailCoefficients)> = vec![
            ("natural".into(), MaterialParams::EARTH, NATURAL_STRENGTH, e),
            ("natural_oct64".into(), MaterialParams::EARTH, NATURAL_STRENGTH, octaves_on(1)),
            ("natural_oct8".into(), MaterialParams::EARTH, NATURAL_STRENGTH, octaves_on(2)),
            ("natural_oct64_8".into(), MaterialParams::EARTH, NATURAL_STRENGTH, octaves_on(3)),
            ("full".into(), MaterialParams::EARTH, 1.0, e),
        ];
        for (m, name) in MATERIAL_NAMES.iter().enumerate() {
            variants.push(((*name).to_string(), forced(m), 1.0, e));
        }
        // The cost of the two octaves, indicative: the same plan rendered 40 times each (after two warm-up renders), the colour read back to wait for the GPU, the wall time of
        // the octaves on over the wall time of the Earth default in this one run (a ratio, not a measurement of the budget).
        let time = |r: &mut TerrainRenderer, coef: DetailCoefficients| {
            let mut lit = LitPlan::fixed(EV).with_atmosphere(&look, R).expect("an atmosphere plan");
            lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
            lit.ground_detail = NATURAL_STRENGTH;
            lit.coef = coef;
            let mut p = plan.clone();
            p.lit = Some(lit);
            for _ in 0..2 {
                r.render(&ctx, &p, false).expect("render");
            }
            let _ = r.read_color(&ctx);
            let t = std::time::Instant::now();
            for _ in 0..40 {
                r.render(&ctx, &p, false).expect("render");
            }
            let _ = r.read_color(&ctx);
            t.elapsed().as_secs_f64() * 1000.0 / 40.0
        };
        let (off1, on, off2) = (time(&mut r, e), time(&mut r, octaves_on(3)), time(&mut r, e));
        println!("cost (indicative, same run) eye {eye} m: octaves off {off1:.2} / {off2:.2} ms a frame, both octaves on {on:.2} ms, ratio {:.2}", on / ((off1 + off2) / 2.0));
        for (name, materials, strength, coef) in variants {
            let mut lit = LitPlan::fixed(EV).with_atmosphere(&look, R).expect("an atmosphere plan");
            lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
            lit.materials = materials;
            lit.ground_detail = strength;
            lit.coef = coef;
            let mut p = plan.clone();
            p.lit = Some(lit);
            for _ in 0..2 {
                r.render(&ctx, &p, false).expect("render");
            }
            let file = out.join(format!("gt_eye{eye}m_{name}.png"));
            r.read_color(&ctx).write_png(&file).expect("png");
            println!("wrote {} ({} nodes)", file.display(), plan.nodes.len());
        }
    }
}
