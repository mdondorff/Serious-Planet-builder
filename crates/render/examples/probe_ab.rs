//! Exploration (compact tiles in a storage buffer instead of an atlas texture): interleaved A/B of the GPU time of one view and the
//! upload cost of the tile writes. Both renderers live on one device and draw the same plan; frames alternate A B, B A, A B ...
//! (docs/learnings.md, 2026-10-15). Indicative: the GPU clock state is not controlled; never a measurement for a budget.
//!
//! `cargo run --release -p planet-render --example probe_ab -- [--adapter software|hardware] [--view 6] [--frames 300] [--part gpu|upload|both]`

#![allow(clippy::disallowed_methods)]

use planet_core::cube::TangentWarp;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{tile_data_field, RenderTile};
use planet_generators::height::Reference;
use planet_render::{set_buffer_atlas, AdapterPolicy, GpuContext, TerrainRenderer, UploadMode};
use planet_streaming::{PoolKind, SlotRef};
use std::time::{Duration, Instant};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 32;
const SEED: u64 = 1;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 - 1.0) * q).round() as usize]
}

fn summary(name: &str, v: &mut Vec<f64>) {
    println!("  {name:<8} median {:.3}  p10 {:.3}  p90 {:.3}  (n {})", pct(v, 0.5), pct(v, 0.1), pct(v, 0.9), v.len());
}

fn tiles_of(plan: &TerrainPlan, finest: u8) -> Vec<RenderTile> {
    let nodes: Vec<_> = plan.nodes.iter().map(|n| n.tile).collect();
    let threads = 16usize;
    let chunk = nodes.len().div_ceil(threads);
    let mut out: Vec<Vec<RenderTile>> = Vec::new();
    std::thread::scope(|s| {
        let hs: Vec<_> = nodes
            .chunks(chunk.max(1))
            .map(|c| s.spawn(move || c.iter().map(|t| tile_data_field(&Reference(SEED), &TangentWarp, *t, CELLS, R, finest)).collect::<Vec<_>>()))
            .collect();
        for h in hs {
            out.push(h.join().unwrap());
        }
    });
    out.into_iter().flatten().collect()
}

fn renderer(ctx: &GpuContext, size: (u32, u32), buffer: bool) -> TerrainRenderer {
    set_buffer_atlas(buffer);
    let pool = compact_format_pool_with(1 << 30, CELLS, 4096, 1).unwrap();
    let r = TerrainRenderer::with_compact_pools(ctx, size, CELLS, pool);
    set_buffer_atlas(false);
    r
}

fn main() {
    let policy = match arg("--adapter").as_deref() {
        Some("hardware") => AdapterPolicy::HighPerformance,
        _ => AdapterPolicy::Software,
    };
    let view: usize = arg("--view").map(|v| v.parse().unwrap()).unwrap_or(6);
    let frames: usize = arg("--frames").map(|v| v.parse().unwrap()).unwrap_or(300);
    let part = arg("--part").unwrap_or_else(|| "both".into());
    let size = (2560u32, 1440u32);
    let ctx = GpuContext::new(policy).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let finest = planet_frame::production_finest_level(CELLS);
    let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(size.1), tau_px: 6.0, max_level: finest, ..LodParams::earth_1080p() };
    let (name, camera) = scripted_views(R, f64::from(size.0) / f64::from(size.1), SEED)[view];
    let plan = plan_terrain(&TerrainRequest { camera, size, view: TerrainView::Relief, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap();
    let t0 = Instant::now();
    let tiles = tiles_of(&plan, finest);
    let compact = tiles.iter().filter(|t| matches!(t, RenderTile::Compact(_))).count();
    println!("view {view} {name}: {} nodes, {compact} compact, tiles built in {:.1} s", plan.nodes.len(), t0.elapsed().as_secs_f64());

    if part == "gpu" || part == "both" {
        let (mut a, mut b) = (renderer(&ctx, size, false), renderer(&ctx, size, true));
        for r in [&mut a, &mut b] {
            r.warm_up(&ctx);
            for t in &tiles {
                r.upload_tile(&ctx, t).unwrap();
            }
        }
        for _ in 0..30 {
            a.render(&ctx, &plan, true).unwrap();
            b.render(&ctx, &plan, true).unwrap();
        }
        let (mut ga, mut gb, mut ca, mut cb) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for i in 0..frames {
            let order = if i % 2 == 0 { [0, 1] } else { [1, 0] };
            for which in order {
                let (r, g, c) = if which == 0 { (&mut a, &mut ga, &mut ca) } else { (&mut b, &mut gb, &mut cb) };
                let (t, _) = r.render(&ctx, &plan, true).unwrap();
                g.push(t.gpu_ms.unwrap_or(f64::NAN));
                c.push(t.cpu_ms);
            }
        }
        println!("GPU time of the render pass, ms, interleaved A B / B A over {frames} frames (A = atlas texture, B = storage buffer):");
        summary("atlas", &mut ga);
        summary("buffer", &mut gb);
        println!("ratio buffer / atlas (medians): {:.3}", pct(&mut gb, 0.5) / pct(&mut ga, 0.5));
        println!("CPU time to record and submit, ms:");
        summary("atlas", &mut ca);
        summary("buffer", &mut cb);
        // The same in blocks of 60 frames (A block, B block, ...), as the 2026-10-15 learning also did.
        let (mut ba, mut bb) = (Vec::new(), Vec::new());
        for blk in 0..(frames / 60).max(2) {
            for which in if blk % 2 == 0 { [0, 1] } else { [1, 0] } {
                let r = if which == 0 { &mut a } else { &mut b };
                let v = if which == 0 { &mut ba } else { &mut bb };
                for _ in 0..60 {
                    v.push(r.render(&ctx, &plan, true).unwrap().0.gpu_ms.unwrap_or(f64::NAN));
                }
            }
        }
        println!("GPU time in alternating blocks of 60 frames:");
        summary("atlas", &mut ba);
        summary("buffer", &mut bb);
        println!("ratio buffer / atlas (medians): {:.3}", pct(&mut bb, 0.5) / pct(&mut ba, 0.5));
    }

    if part == "upload" || part == "both" {
        println!("
Upload of {} tiles ({} compact), median of 5 fresh renderers. first = the first upload into fresh blocks (includes the driver's / wgpu's", tiles.len(), compact);
        println!("first-touch initialisation of the block), warm = the same tiles written again into the same slots. calls = frame-thread time in the");
        println!("upload calls, wall = calls plus an empty submission and waiting until the queue is idle:");
        for (label, mode, stride) in [("per slot, contiguous", UploadMode::PerSlot, 1u32), ("batched, contiguous", UploadMode::Batched, 1), ("batched, every 3rd slot", UploadMode::Batched, 3)] {
            for buffer in [false, true] {
                let (mut fc, mut fw, mut wc, mut ww) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                let (mut writes, mut bytes) = (0, 0);
                for _ in 0..5 {
                    let mut r = renderer(&ctx, size, buffer);
                    r.warm_up(&ctx);
                    r.set_upload_mode(mode);
                    ctx.probe_wait();
                    let placed: Vec<(&RenderTile, SlotRef)> = tiles
                        .iter()
                        .filter(|t| matches!(t, RenderTile::Compact(_)))
                        .enumerate()
                        .map(|(k, t)| (t, SlotRef { pool: PoolKind::Main, index: k as u32 * stride }))
                        .collect();
                    let t0 = Instant::now();
                    r.upload_tiles_at(&ctx, &placed).unwrap();
                    fc.push(t0.elapsed().as_secs_f64() * 1e3);
                    ctx.probe_wait();
                    fw.push(t0.elapsed().as_secs_f64() * 1e3);
                    let before = r.upload_counters();
                    let t1 = Instant::now();
                    r.upload_tiles_at(&ctx, &placed).unwrap();
                    wc.push(t1.elapsed().as_secs_f64() * 1e3);
                    ctx.probe_wait();
                    ww.push(t1.elapsed().as_secs_f64() * 1e3);
                    let c = r.upload_counters();
                    writes = c.write_calls - before.write_calls;
                    bytes = c.staging_bytes - before.staging_bytes;
                    let _ = Duration::ZERO;
                }
                println!(
                    "  {label:<24} {:<7} first: calls {:>8.2} wall {:>9.2} ms | warm: calls {:>7.2} wall {:>8.2} ms = {:.2} us/tile ({writes} write calls, {bytes} bytes)",
                    if buffer { "buffer" } else { "atlas" },
                    pct(&mut fc, 0.5),
                    pct(&mut fw, 0.5),
                    pct(&mut wc, 0.5),
                    pct(&mut ww, 0.5),
                    pct(&mut ww, 0.5) * 1e3 / compact as f64
                );
            }
        }
    }
}
