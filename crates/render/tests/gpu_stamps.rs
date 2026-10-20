//! Tier C, GEN-026 (Stamped scene): a stamped scene drawn in the debug relief view is the recorded frame of each adapter family, and its
//! images are written to `target/review/stamps/` for inspection. A hill, a crater and a plateau stand on the ground in front of a camera
//! 200 m above it; the unstamped scene is drawn beside them so the images show what the stamps add. No PNG golden (the owner's `/bless`
//! would be needed): the hash is the record.

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::{StampStage, STAMP_STAGE_VERSION};
use planet_render::{execute_terrain_tiles, AdapterPolicy, GpuContext};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (320, 240);
const CELLS: u32 = 16;
const SEED: u64 = 1;

fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}
const FNV0: u64 = 0xcbf2_9ce4_8422_2325;

const WARP: &str = "Microsoft Basic Render Driver | Dx12 | Cpu | driver '10.0.26100.9711' ''";
const RTX: &str = "NVIDIA GeForce RTX 3080 Ti Laptop GPU | Vulkan | DiscreteGpu | driver 'NVIDIA' '591.86'";

fn item(id: u128, at: Vec3, kind: StampKind, amount: f64, reach_m: f64, bound: f64) -> ItemSpec {
    ItemSpec {
        id,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: id as i32,
        dir: at,
        content_hash: 0,
        reach_m,
        max_delta_m: bound,
        stamp: None,
    }
    .with_stamp(Stamp::new(kind, [amount, 0.0, 0.0]))
}

/// The point `forward_m` ahead of the camera and `right_m` to its right, on the reference sphere, as a unit direction.
fn ground(camera: &planet_frame::Camera, forward_m: f64, right_m: f64) -> Vec3 {
    let up = camera.position.0.normalized();
    let ahead = (camera.forward - up * camera.forward.dot(up)).normalized();
    let right = ahead.cross(up);
    (up * R + ahead * forward_m + right * right_m).normalized()
}

fn plan(field: &dyn HeightField, camera: planet_frame::Camera) -> TerrainPlan {
    let lod = LodParams {
        cells: CELLS,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 2.5,
        max_level: planet_frame::production_finest_level(CELLS),
        ..LodParams::earth_1080p()
    };
    plan_terrain_field(
        &TerrainRequest { camera, size: SIZE, view: TerrainView::Relief, lod, face_mapping: "tangent-v1".into(), seed: SEED },
        field,
    )
    .unwrap()
}

fn draw(ctx: &GpuContext, field: &dyn HeightField, plan: &TerrainPlan) -> Vec<u8> {
    let tiles: Vec<CompactTile> = plan.nodes.iter().map(|n| compact_tile_field(field, &TangentWarp, n.tile, CELLS, R)).collect();
    let refs: Vec<&CompactTile> = tiles.iter().collect();
    execute_terrain_tiles(ctx, plan, &refs).unwrap().0.rgba
}

fn write_ppm(name: &str, rgba: &[u8]) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("target").join("review").join("stamps");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let mut out = format!("P6\n{} {}\n255\n", SIZE.0, SIZE.1).into_bytes();
    out.extend(rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]));
    let _ = std::fs::write(dir.join(format!("{name}.ppm")), out);
}

// spec: GEN-026
#[test]
fn a_stamped_scene_in_the_relief_view_is_the_recorded_frame_of_the_adapter_family() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (_, camera) = scripted_views_field(R, f64::from(SIZE.0) / f64::from(SIZE.1), &Reference(SEED))
        .into_iter()
        .find(|(n, _)| *n == "ground-200m")
        .expect("the 200 m view");
    let mut def = WorldDefinition::single_planet(R, SEED);
    def.put_item_spec(BodyId::PLANET, item(1, ground(&camera, 500.0, 0.0), StampKind::Hill, 35.0, 600.0, 60.0)).unwrap();
    def.put_item_spec(BodyId::PLANET, item(2, ground(&camera, 800.0, 350.0), StampKind::Crater, 30.0, 500.0, 60.0)).unwrap();
    def.put_item_spec(BodyId::PLANET, item(3, ground(&camera, 650.0, -450.0), StampKind::Plateau, 40.0, 500.0, 80.0)).unwrap();
    let base: Arc<dyn HeightField> = Arc::new(Reference(SEED));
    let stamped = StampStage::new(base, Arc::new(def), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, STAMP_STAGE_VERSION).unwrap();
    let plain = Reference(SEED);

    let (plain_plan, stamped_plan) = (plan(&plain, camera), plan(&stamped, camera));
    let plain_frame = draw(&ctx, &plain, &plain_plan);
    let stamped_frame = draw(&ctx, &stamped, &stamped_plan);
    let changed = plain_frame.chunks(4).zip(stamped_frame.chunks(4)).filter(|(a, b)| a != b).count();
    write_ppm("plain-relief", &plain_frame);
    write_ppm("stamped-relief", &stamped_frame);
    assert!(changed > 200, "the stamps show in the frame: {changed} pixels differ");
    assert!(changed < plain_frame.len() / 4 * 9 / 10, "the stamps are local: {changed} pixels differ");
    // The same scene drawn twice is the same frame.
    assert_eq!(draw(&ctx, &stamped, &stamped_plan), stamped_frame, "a second draw of the scene differs");

    let hash = fnv(fnv(FNV0, &plain_frame), &stamped_frame);
    println!("stamped scene: {changed} pixels differ, hash {hash:016x} on {}", ctx.driver_string());
    let driver = ctx.driver_string();
    let known: &[(&str, u64)] = &[(WARP, 0xe32d_6690_7031_8913), (RTX, 0x5335_ae65_a642_b13c)];
    match known.iter().find(|(d, _)| *d == driver) {
        Some((_, want)) => assert_eq!(hash, *want, "the stamped scene changed on {driver}"),
        None => println!("KNOWN-ANSWER stamped scene: no hash recorded for '{driver}' (WARP {WARP}, RTX {RTX}); got {hash:016x}"),
    }
}
