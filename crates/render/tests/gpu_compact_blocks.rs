//! Tier C, REND-016 / REND-018: the compact tiles in storage-buffer blocks (ADR 0054) draw exactly the pixels the atlas textures drew.
//! TRANSITIONAL: this differential exists while both paths exist; it records the atlas path's frame hashes.

use planet_core::cube::TangentWarp;
use planet_frame::look::atmosphere::{Atmosphere, AtmosphereView};
use planet_frame::look::file::LookParams;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{tile_data_field, RenderTile};
use planet_generators::height::Reference;
use planet_render::{execute_terrain_tiles, set_buffer_blocks, AdapterPolicy, GpuContext, TerrainRenderer, UploadMode};
use planet_streaming::PoolConfig;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const SEED: u64 = 1;

fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}
/// The switch is process-global while both paths exist: the two tests take turns.
static SWITCH: std::sync::Mutex<()> = std::sync::Mutex::new(());
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

fn tiles(plan: &TerrainPlan) -> Vec<RenderTile> {
    plan.nodes.iter().map(|n| tile_data_field(&Reference(SEED), &TangentWarp, n.tile, CELLS, R, lod().max_level)).collect()
}

fn compact_pool() -> PoolConfig {
    let side = 24 * (CELLS + 1);
    let one = compact_format_pool_with((128 << 20) + 4 * u64::from(side) * u64::from(side) * 8, CELLS, side, 1).unwrap();
    PoolConfig { vertex_slots: 256, fallback_slots: 192, ..one }
}

fn one_shot(ctx: &GpuContext, plan: &TerrainPlan, buffer: bool) -> Vec<u8> {
    set_buffer_blocks(buffer);
    assert_eq!(planet_render::compact_terrain_shader(CELLS).contains("array<vec2<u32>>"), buffer, "the switch changes the shader");
    let t = tiles(plan);
    let refs: Vec<&RenderTile> = t.iter().collect();
    let (frame, _) = execute_terrain_tiles(ctx, plan, &refs).unwrap();
    set_buffer_blocks(false);
    frame.rgba
}

// spec: REND-016
#[test]
fn one_shot_frames_are_identical_in_every_debug_view() {
    let _turn = SWITCH.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let (mut frames, mut compact_nodes, mut all) = (0, 0, FNV0);
    for index in 0..10 {
        for view in TerrainView::ALL {
            let plan = plan_for(index, view);
            compact_nodes += plan.nodes.iter().filter(|n| n.level >= 3).count();
            let atlas = one_shot(&ctx, &plan, false);
            let buffer = one_shot(&ctx, &plan, true);
            assert!(
                atlas == buffer,
                "view {index} {}: the buffer path draws other pixels ({} differ)",
                view.name(),
                atlas.iter().zip(&buffer).filter(|(a, b)| a != b).count()
            );
            all = fnv(all, &atlas);
            frames += 1;
        }
    }
    println!("DIFFERENTIAL one_shot identical: {frames} frames, {compact_nodes} compact nodes drawn, atlas-path hash {all:016x}");
}

fn persistent(ctx: &GpuContext, plan: &TerrainPlan, buffer: bool, mode: UploadMode, lit: Option<LitPlan>) -> Vec<u8> {
    set_buffer_blocks(buffer);
    let mut r = TerrainRenderer::with_compact_pools(ctx, SIZE, CELLS, compact_pool());
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
    let rgba = r.read_color(ctx).rgba;
    set_buffer_blocks(false);
    rgba
}

// spec: REND-016, REND-018
#[test]
fn persistent_renderers_and_the_lit_atmosphere_frame_are_identical() {
    let _turn = SWITCH.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (mut frames, mut all) = (0, FNV0);
    for index in [3usize, 5, 6, 9] {
        let plan = plan_for(index, TerrainView::Level);
        for mode in [UploadMode::Batched, UploadMode::PerSlot] {
            let a = persistent(&ctx, &plan, false, mode, None);
            let b = persistent(&ctx, &plan, true, mode, None);
            assert!(a == b, "view {index} {mode:?}: debug frame differs");
            all = fnv(all, &a);
            frames += 1;
        }
        let mut l = LitPlan::adaptive(11.0, 0.016).with_atmosphere(&LookParams::default(), R).unwrap();
        l.atmosphere.as_mut().unwrap().view = AtmosphereView::Composite;
        let a = persistent(&ctx, &plan, false, UploadMode::Batched, Some(l.clone()));
        let b = persistent(&ctx, &plan, true, UploadMode::Batched, Some(l));
        assert!(a == b, "view {index}: lit atmosphere frame differs");
        all = fnv(all, &a);
        frames += 1;
    }
    println!("DIFFERENTIAL persistent identical: {frames} frames, atlas-path hash {all:016x}");
}
