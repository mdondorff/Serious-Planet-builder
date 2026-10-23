//! Tier C: the lit look of the ground at eye height (M7 GT, REND-034, REND-035, REND-037): the steep stamped site of the review stills
//! (`examples/look_stills.rs`) on the lit path with the atmosphere, the natural materials at the window's strength, at an eye height of
//! 1.8 m and 0.5 m. The owner approved this look as the floor (2026-10-23); a change of it is the owner's to bless.

use std::sync::{Arc, RwLock};

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::look::atmosphere::Atmosphere;
use planet_frame::look::file::LookParams;
use planet_frame::look::materials::{MaterialParams, NATURAL_STRENGTH};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain_cached, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, slot_bytes};
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::StampStage;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_testkit::{GoldenChecks, Image, Tolerance};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (640, 360);
const CELLS: u32 = 32;
const FINEST: u8 = 26;
const EV: f32 = 14.5;

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

// spec: REND-034, REND-035, REND-037
#[test]
fn the_lit_ground_at_eye_height_matches_the_adapters_goldens() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut goldens = GoldenChecks::new(&ctx.adapter_key(), &ctx.driver_string());
    // The ground site (20 N, 30 E) facing north, the sun 40 degrees up behind the camera's right shoulder, so the slopes ahead are lit.
    let (lat, lon) = (20f64.to_radians(), 30f64.to_radians());
    let up = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
    let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
    let north = up.cross(east).normalized();
    let sun = (up * libm::sin(40f64.to_radians()) + (east * 0.7 - north * 0.7).normalized() * libm::cos(40f64.to_radians())).normalized();
    let at = |ahead_m: f64, right_m: f64| (up * R + north * ahead_m + east * right_m).normalized();
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
    for (eye, pitch_deg, golden) in [(1.8, 5.0, "lit_ground_eye1p8m_natural_ev14p5"), (0.5, 10.0, "lit_ground_eye0p5m_natural_ev14p5")] {
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
        let mut plan = plan_terrain_cached(
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
        let mut lit = LitPlan::fixed(EV).with_atmosphere(&look, R).expect("an atmosphere plan");
        lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
        lit.materials = MaterialParams::EARTH;
        lit.ground_detail = NATURAL_STRENGTH;
        plan.lit = Some(lit);
        // Two renders: the second sees the settled exposure and caches.
        for _ in 0..2 {
            r.render(&ctx, &plan, false).expect("render");
        }
        let f = r.read_color(&ctx);
        let img = Image::new(f.width, f.height, f.rgba);
        let covered = r.lit_path().expect("lit").read_hdr(&ctx).iter().filter(|px| px[3] > 0.0).count();
        assert!(covered > 10_000, "{golden}: only {covered} terrain pixels");
        let colours: std::collections::BTreeSet<[u8; 3]> = img.pixels().map(|px| [px[0], px[1], px[2]]).collect();
        assert!(colours.len() > 1000, "{golden}: {} colours", colours.len());
        goldens.check(golden, &img, Tolerance::Exact);
    }
    goldens.finish();
}
