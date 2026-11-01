//! REND-050, REND-051 (the frame path part), tier C: the shadow pass runs after the lit terrain pass on its depth. A lit frame with a shadow
//! frame set resolves a visibility target of the frame's size and leaves the lit picture byte-identical (nothing reads the visibility for
//! shading before the apply step); a frame without a shadow frame runs no shadow pass.

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::clipmap::{ClipmapParams, ClipmapPlanner};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::shadowview::ShadowView;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::Reference;
use planet_render::shadow::ClipmapPreview;
use planet_render::shadow_path::ShadowFrame;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const TEXELS: u32 = 256;
const LEVELS: usize = 11;
const EV: f32 = 15.0;

fn plan() -> TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[3];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let mut p =
        plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 })
            .unwrap();
    p.lit = Some(LitPlan::fixed(EV));
    p
}

fn renderer(ctx: &GpuContext, p: &TerrainPlan) -> (TerrainRenderer, Vec<TileId>) {
    let block_slots = 24 * 24;
    let pool = compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, pool);
    r.enable_lit(ctx);
    let mut tiles = Vec::new();
    for n in &p.nodes {
        if r.slot_of(n.tile).is_none() {
            r.upload_tile(ctx, &compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R)).unwrap();
            tiles.push(n.tile);
        }
    }
    (r, tiles)
}

fn shadow_frame(p: &TerrainPlan, tiles: &[TileId], planner: &mut ClipmapPlanner) -> ShadowFrame {
    let cam = p.camera.position;
    let clip = planner.plan(cam, tiles, &[], &[], &TangentWarp);
    let anchor = clip.anchor;
    let up = cam.0.normalized();
    let sun = (up * 0.5 + Vec3::new(0.0, 0.0, 1.0)).normalized();
    ShadowFrame {
        view: ShadowView::new(anchor, &p.camera, sun, R),
        plan: clip,
        preview: ClipmapPreview::default(),
        floor_tiles: Vec::new(),
        cut: false,
    }
}

fn frame_bytes(r: &TerrainRenderer, ctx: &GpuContext) -> Vec<u8> {
    r.read_color(ctx).rgba
}

// spec: REND-050
#[test]
fn a_lit_frame_with_a_shadow_frame_resolves_a_visibility_and_draws_the_same_picture() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan();
    let (mut with, tiles) = renderer(&ctx, &p);
    let (mut without, _) = renderer(&ctx, &p);
    with.enable_shadow(&ctx, TEXELS, LEVELS, R);
    let mut planner = ClipmapPlanner::new(ClipmapParams { texels: TEXELS, ..ClipmapParams::new(R, CELLS) });
    with.set_shadow_frame(shadow_frame(&p, &tiles, &mut planner));
    with.render(&ctx, &p, false).unwrap();
    without.render(&ctx, &p, false).unwrap();
    assert_eq!(frame_bytes(&with, &ctx), frame_bytes(&without, &ctx), "the lit picture is the picture without the shadow pass");
    let visibility = with.read_shadow(&ctx).expect("the shadow pass ran");
    assert_eq!(visibility.len(), (SIZE.0 * SIZE.1) as usize);
    assert!(visibility.iter().all(|v| (0.0..=1.0).contains(v)), "visibility lies in [0, 1]");
    assert!(without.read_shadow(&ctx).is_none(), "a renderer without the shadow pass has none");
}
