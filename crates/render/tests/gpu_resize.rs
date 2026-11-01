//! VIEW-006, tier C: a resize of the 3D target (owner, 2026-10-30: the target kept its size after a window resize). The renderer makes its
//! colour and depth targets, the lit path's targets and the atmosphere's products again at the new size, keeps its tiles, pipelines and pools,
//! and then draws the frame of a renderer built at that size from the start, byte for byte.

use planet_core::cube::TangentWarp;
use planet_frame::look::atmosphere::Atmosphere;
use planet_frame::look::file::LookParams;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, ExecError, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
const EV: f32 = 15.0;
const SMALL: (u32, u32) = (96, 64);
const WIDE: (u32, u32) = (160, 72);

/// The plan of scripted view 3 at `size`, its camera's aspect the size's.
fn plan_at(size: (u32, u32), atmosphere: bool) -> TerrainPlan {
    let aspect = f64::from(size.0) / f64::from(size.1);
    let (_, camera) = scripted_views(R, aspect, 1)[3];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let mut p =
        plan_terrain(&TerrainRequest { camera, size, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap();
    p.lit = Some(if atmosphere { LitPlan::fixed(EV).with_atmosphere(&LookParams::default(), R).unwrap() } else { LitPlan::fixed(EV) });
    p
}

fn renderer(ctx: &GpuContext, size: (u32, u32), plans: &[&TerrainPlan], atmosphere: bool) -> TerrainRenderer {
    renderer_with_water(ctx, size, plans, atmosphere, false)
}

fn renderer_with_water(ctx: &GpuContext, size: (u32, u32), plans: &[&TerrainPlan], atmosphere: bool, water: bool) -> TerrainRenderer {
    let block_slots = 24 * 24;
    let pool = compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    let mut r = TerrainRenderer::with_pools(ctx, size, CELLS, pool);
    r.enable_lit(ctx);
    if atmosphere {
        r.enable_atmosphere(ctx, &Atmosphere::earth(R));
    }
    if water {
        r.enable_water(ctx);
    }
    for p in plans {
        for n in &p.nodes {
            if r.slot_of(n.tile).is_none() {
                r.upload_tile(ctx, &compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R)).unwrap();
            }
        }
    }
    r
}

fn resize_draws_what_a_fresh_renderer_draws(atmosphere: bool, water: bool) {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (small, wide) = (plan_at(SMALL, atmosphere), plan_at(WIDE, atmosphere));
    let mut resized = renderer_with_water(&ctx, SMALL, &[&small, &wide], atmosphere, water);
    resized.render(&ctx, &small, false).unwrap();
    let slots: Vec<_> = small.nodes.iter().map(|n| resized.slot_of(n.tile)).collect();
    resized.resize(&ctx, WIDE);
    assert!(small.nodes.iter().zip(&slots).all(|(n, s)| resized.slot_of(n.tile) == *s), "the tiles stay where they were");
    assert_eq!(resized.target_size(), WIDE);
    // The old size is refused, the new one is drawn.
    let e = resized.render(&ctx, &small, false).unwrap_err();
    assert!(matches!(e, ExecError::SizeMismatch { .. }), "{e}");
    resized.render(&ctx, &wide, false).unwrap();
    let got = resized.read_color(&ctx);
    assert_eq!((got.width, got.height), WIDE);
    let mut fresh = renderer_with_water(&ctx, WIDE, &[&wide], atmosphere, water);
    fresh.render(&ctx, &wide, false).unwrap();
    assert!(got.rgba == fresh.read_color(&ctx).rgba, "the resized renderer draws the frame of a fresh one (atmosphere: {atmosphere})");
    // And back again.
    resized.resize(&ctx, SMALL);
    resized.render(&ctx, &small, false).unwrap();
    let mut back = renderer_with_water(&ctx, SMALL, &[&small], atmosphere, water);
    back.render(&ctx, &small, false).unwrap();
    assert!(resized.read_color(&ctx).rgba == back.read_color(&ctx).rgba, "and back to the first size (atmosphere: {atmosphere})");
}

// spec: VIEW-006
#[test]
fn a_resized_lit_renderer_draws_the_frame_of_a_fresh_one() {
    resize_draws_what_a_fresh_renderer_draws(false, false);
}

// spec: VIEW-006
#[test]
fn a_resized_renderer_with_the_atmosphere_draws_the_frame_of_a_fresh_one() {
    resize_draws_what_a_fresh_renderer_draws(true, false);
}

// spec: VIEW-006
#[test]
fn a_resized_renderer_with_the_water_pass_and_the_atmosphere_draws_the_frame_of_a_fresh_one() {
    resize_draws_what_a_fresh_renderer_draws(true, true);
}
