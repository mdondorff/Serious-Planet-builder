//! Tier C, REND-016 / REND-018: the compact tiles in storage-buffer blocks (ADR 0054) draw known-answer pixels. The hashes are those of
//! the frames the atlas textures drew (recorded by the image differential of change `compact-storage-buffers` before the atlas path
//! was removed: byte-identical on WARP and on the RTX), so a change of the buffer path that moves one pixel fails here.

use planet_core::cube::TangentWarp;
use planet_frame::look::atmosphere::{Atmosphere, AtmosphereView};
use planet_frame::look::file::LookParams;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::height::Reference;
use planet_render::{execute_terrain_tiles, AdapterPolicy, GpuContext, TerrainRenderer, UploadMode};
use planet_streaming::PoolConfig;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const SEED: u64 = 1;

fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}
const FNV0: u64 = 0xcbf2_9ce4_8422_2325;

fn lod() -> LodParams {
    LodParams {
        cells: CELLS,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 2.5,
        max_level: planet_frame::production_finest_level(CELLS),
        ..LodParams::earth_1080p()
    }
}

fn plan_for(index: usize, view: TerrainView) -> TerrainPlan {
    let (_, camera) = scripted_views(R, f64::from(SIZE.0) / f64::from(SIZE.1), SEED)[index];
    plan_terrain(&TerrainRequest { camera, size: SIZE, view, lod: lod(), face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

fn tiles(plan: &TerrainPlan) -> Vec<CompactTile> {
    plan.nodes.iter().map(|n| compact_tile_field(&Reference(SEED), &TangentWarp, n.tile, CELLS, R)).collect()
}

fn compact_pool() -> PoolConfig {
    let block_slots = 24 * 24;
    compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap()
}

fn one_shot(ctx: &GpuContext, plan: &TerrainPlan) -> Vec<u8> {
    let t = tiles(plan);
    let refs: Vec<&CompactTile> = t.iter().collect();
    let (frame, _) = execute_terrain_tiles(ctx, plan, &refs).unwrap();
    frame.rgba
}

/// The hash recorded for this driver, if any: pixels are exact only for one driver build, so a driver not in the table prints its
/// hash (to record it) and passes on the structural checks of the other tests (the goldens, the pool read-back tests).
fn expect(ctx: &GpuContext, what: &str, got: u64, known: &[(&str, u64)]) {
    let driver = ctx.driver_string();
    match known.iter().find(|(d, _)| *d == driver) {
        Some((_, want)) => assert_eq!(got, *want, "{what}: the frames of the buffer path changed on {driver}"),
        None => println!("KNOWN-ANSWER {what}: no hash recorded for '{driver}'; got {got:016x}"),
    }
}

const WARP: &str = "Microsoft Basic Render Driver | Dx12 | Cpu | driver '10.0.26100.9711' ''";
// The RTX hashes below were recorded on 2026-11-01 under MESH_VERSION 10 (`test gpu --real`) on this adapter and driver; an adapter or
// driver not in a table skips the comparison with a printed hash and never compares against the RTX hash.
const RTX: &str = "NVIDIA GeForce RTX 3080 Ti Laptop GPU | Vulkan | DiscreteGpu | driver 'NVIDIA' '621.01'";

// spec: REND-016
#[test]
fn one_shot_frames_in_every_debug_view_are_the_recorded_pixels() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{}", ctx.driver_string());
    let (mut frames, mut compact_nodes, mut all) = (0, 0, FNV0);
    for index in 0..10 {
        for view in TerrainView::ALL {
            let plan = plan_for(index, view);
            compact_nodes += plan.nodes.iter().filter(|n| n.level >= 3).count();
            all = fnv(all, &one_shot(&ctx, &plan));
            frames += 1;
        }
    }
    // 18,180 since render-floor-3 (LOD-011, declared): the orbit views draw the 181 level-3 tiles instead of 52 level-2 tiles (16,542).
    assert_eq!((frames, compact_nodes), (90, 18_180), "the scripted views changed");
    println!("one_shot: {frames} frames, {compact_nodes} compact nodes drawn, hash {all:016x}");
    expect(&ctx, "one_shot", all, &[(WARP, 0xba29_e870_5bd4_05a3), (RTX, 0x60a4_c811_a037_d009)]);
}

fn persistent(ctx: &GpuContext, plan: &TerrainPlan, mode: UploadMode, lit: Option<LitPlan>) -> Vec<u8> {
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, compact_pool());
    r.set_upload_mode(mode);
    if lit.is_some() {
        r.enable_lit(ctx);
        r.enable_atmosphere(ctx, &Atmosphere::earth(R));
    }
    for t in tiles(plan) {
        r.upload_tile(ctx, &t).unwrap();
    }
    let mut p = plan.clone();
    p.lit = lit;
    r.render(ctx, &p, false).unwrap();
    r.read_color(ctx).rgba
}

// spec: REND-016, REND-018
#[test]
fn persistent_renderers_per_slot_and_batched_and_the_lit_atmosphere_frame_are_the_recorded_pixels() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (mut frames, mut all) = (0, FNV0);
    for index in [3usize, 5, 6, 9] {
        let plan = plan_for(index, TerrainView::Level);
        let batched = persistent(&ctx, &plan, UploadMode::Batched, None);
        let per_slot = persistent(&ctx, &plan, UploadMode::PerSlot, None);
        assert!(batched == per_slot, "view {index}: the upload modes draw other pixels");
        all = fnv(fnv(all, &batched), &per_slot);
        let mut l = LitPlan::adaptive(11.0, 0.016).with_atmosphere(&LookParams::default(), R).unwrap();
        l.atmosphere.as_mut().unwrap().view = AtmosphereView::Composite;
        all = fnv(all, &persistent(&ctx, &plan, UploadMode::Batched, Some(l)));
        frames += 3;
    }
    println!("persistent: {frames} frames, hash {all:016x}");
    expect(&ctx, "persistent", all, &[(WARP, 0x50c8_4abd_21f3_804b), (RTX, 0x213e_4a0a_bbc7_3f53)]);
}
