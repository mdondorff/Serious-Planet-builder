//! REND-015: the indirect multi-draw path draws exactly the image of the per-draw loop (the reference), on every adapter that
//! supports it, with the draw records built on the CPU from the plan (CON-03). Where the adapter lacks the feature the mode
//! falls back to the per-draw loop and says so.

//!
//! On the compact path (converted from the vertex-format renderer by `remove-vertex-render-path`, task 2.2).

use planet_core::cube::TangentWarp;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, DrawMode, GpuContext, TerrainRenderer};
use planet_streaming::budget::FIXED_RESERVE_BYTES;
use planet_streaming::{PoolConfig, PoolKind, SlotRef};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const FINEST: u8 = 26;

fn plan(index: usize, view: TerrainView) -> TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: SIZE, view, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

fn tiles(p: &TerrainPlan) -> Vec<CompactTile> {
    p.nodes.iter().map(|n| compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R)).collect()
}

/// 3 slots per block, 2,000 blocks: the nodes of a plan span several buffers, so runs split.
fn small_pool() -> PoolConfig {
    compact_format_pool_with(FIXED_RESERVE_BYTES + 2000 * 3 * planet_generators::compact::slot_bytes(CELLS), CELLS, 3, 1).unwrap()
}

fn renderer(ctx: &GpuContext, ts: &[CompactTile], scattered: bool) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, small_pool());
    let mut main = 5999u32;
    for t in ts {
        if scattered {
            main -= 2;
            r.upload_tile_at(ctx, t, SlotRef { pool: PoolKind::Main, index: main }).unwrap();
        } else {
            r.upload_tile(ctx, t).unwrap();
        }
    }
    r
}

// spec: REND-015
#[test]
fn the_indirect_path_draws_the_image_of_the_per_draw_loop() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{} | indirect draw supported: {}", ctx.adapter_log_line(), ctx.indirect_draw_supported());
    let mut compared = 0;
    for view in [TerrainView::Level, TerrainView::Relief] {
        for index in 0..10 {
            let p = plan(index, view);
            let ms = tiles(&p);
            for scattered in [false, true] {
                let mut r = renderer(&ctx, &ms, scattered);
                assert_eq!(r.set_draw_mode(&ctx, DrawMode::PerDraw), DrawMode::PerDraw);
                let (_, per_draw) = r.render(&ctx, &p, false).unwrap();
                let reference = r.read_color(&ctx);
                assert_eq!((per_draw.draw_calls as usize, per_draw.encoded_draws as usize), (p.nodes.len(), p.nodes.len()));
                assert_eq!(per_draw.argument_bytes, 0, "the reference path writes no argument records");

                let effective = r.set_draw_mode(&ctx, DrawMode::Indirect);
                if !ctx.indirect_draw_supported() {
                    assert_eq!(effective, DrawMode::PerDraw, "an adapter without the feature keeps the per-draw loop");
                    continue;
                }
                assert_eq!(effective, DrawMode::Indirect);
                let (_, indirect) = r.render(&ctx, &p, false).unwrap();
                assert_eq!(r.read_color(&ctx), reference, "{view:?} view {index} scattered {scattered}: indirect differs from per-draw");
                assert_eq!(indirect.draw_calls, per_draw.draw_calls);
                assert_eq!(indirect.triangles, per_draw.triangles);
                assert_eq!(indirect.node_record_bytes, per_draw.node_record_bytes);
                assert_eq!(
                    indirect.node_record_bytes,
                    p.nodes.len() as u64 * planet_frame::draw::NODE_RECORD_BYTES,
                    "one 208 B node record per node (192 B and the up vector)"
                );
                assert_eq!(indirect.argument_bytes, p.nodes.len() as u64 * 20, "one 20 B record per node");
                assert!(indirect.encoded_draws >= 1 && indirect.encoded_draws <= indirect.draw_calls, "{indirect:?}");
                if scattered && p.nodes.iter().filter(|n| n.tile.level() > 2).count() > 3 {
                    assert!(indirect.encoded_draws > 1, "nodes in several buffers need several multi-draws: {indirect:?}");
                }
                compared += 1;
            }
        }
    }
    if ctx.indirect_draw_supported() {
        assert_eq!(compared, 40);
    }
}

// spec: REND-015
#[test]
fn frames_in_flight_keep_their_own_records() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (a, b) = (plan(4, TerrainView::Level), plan(5, TerrainView::Level));
    let ms: Vec<CompactTile> = tiles(&a).into_iter().chain(tiles(&b)).collect();
    let mut r = renderer(&ctx, &ms, false);
    // The reference of each plan on its own.
    let mut image = |p: &TerrainPlan| {
        r.render(&ctx, p, false).unwrap();
        r.read_color(&ctx)
    };
    let (ref_a, ref_b) = (image(&a), image(&b));
    assert_ne!(ref_a, ref_b);
    // Alternating plans submitted without waiting, more of them than the ring is deep, and the draws of the last one are its own.
    for (last, reference) in [(&a, &ref_a), (&b, &ref_b)] {
        for k in 0..7 {
            r.submit(&ctx, if k % 2 == 0 { &b } else { &a }).unwrap();
        }
        r.submit(&ctx, last).unwrap();
        assert_eq!(&r.read_color(&ctx), reference);
    }
}
