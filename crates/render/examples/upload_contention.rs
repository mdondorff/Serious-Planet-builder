//! M4 fix F8 micro-benchmark (docs/archive/M4/learnings.md): what a frame's tile upload costs on the frame thread while N worker threads run a
//! CPU-bound tile build loop beside it. The real path (`TerrainRenderer::upload_tiles_at` with compact tiles in adjacent slots that
//! move every frame, then a frame submit as the loop does) is timed whole and, with the renderer's own timers, split into the queue
//! calls and the staging (conversion, sorting, packing); beside it a heap allocation of the staging size, which is what the batched
//! path did once per batch before its buffers were kept. Frames are paced like the pipelined loop (16 ms by default). Indicative:
//! the GPU clock state and the other processes are not controlled, never a measurement for a budget. Not production code.
//!
//! `cargo run --release -p planet-render --example upload_contention -- [--adapter software|hardware] [--frames 300] [--tiles 6]
//!  [--workers 0,4,8,16,20] [--sleep-ms 16]`

#![allow(clippy::disallowed_methods)]

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_streaming::{PoolConfig, PoolKind, SlotRef};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 32;
const COLS: u32 = 16;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn stats(v: &[f64]) -> String {
    let mut v = v.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
    format!("{:.3} / {:.3} / {:.3}", at(0.5), at(0.99), at(1.0))
}

fn pool() -> PoolConfig {
    let block_slots = COLS * COLS;
    let one = compact_format_pool_with(
        (128 << 20) + 40 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    PoolConfig { vertex_slots: 4096, fallback_slots: 3968, ..one }
}

/// Busy workers: each builds compact tiles in a loop (the generation workers' kind of work: heap-heavy, no GPU) until stopped.
fn spawn_workers(n: usize, stop: &Arc<AtomicBool>) -> Vec<std::thread::JoinHandle<u64>> {
    (0..n)
        .map(|w| {
            let stop = Arc::clone(stop);
            std::thread::spawn(move || {
                let field = Reference(1);
                let mut built = 0u64;
                let mut k = w as u32 * 977;
                while !stop.load(Ordering::Relaxed) {
                    let id = TileId::new(Face(0), 12, k % 3000, (k / 3000) % 3000).unwrap();
                    std::hint::black_box(compact_tile_field(&field, &TangentWarp, id, CELLS, R));
                    k = k.wrapping_add(1);
                    built += 1;
                }
                built
            })
        })
        .collect()
}

fn main() {
    let hardware = arg("--adapter").is_some_and(|a| a == "hardware");
    let frames: usize = arg("--frames").and_then(|v| v.parse().ok()).unwrap_or(300);
    let tiles_per_frame: usize = arg("--tiles").and_then(|v| v.parse().ok()).unwrap_or(6);
    let sleep = Duration::from_micros((arg("--sleep-ms").and_then(|v| v.parse::<f64>().ok()).unwrap_or(16.0) * 1000.0) as u64);
    let counts: Vec<usize> =
        arg("--workers").map_or(vec![0, 4, 8, 16, 20], |s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    let ctx = GpuContext::new(if hardware { AdapterPolicy::HighPerformance } else { AdapterPolicy::Software }).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    println!(
        "{} hardware threads; {frames} frames of {tiles_per_frame} compact tiles (cells {CELLS}) paced at {:.1} ms; ms on the frame thread, median / p99 / max",
        std::thread::available_parallelism().map_or(0, |n| n.get()),
        sleep.as_secs_f64() * 1000.0
    );

    let size = (128, 96);
    let mut renderer = TerrainRenderer::with_compact_pools(&ctx, size, CELLS, pool());
    // A frame that draws nothing: its submit flushes the frame's pending queue writes, as the loop's frame does.
    let lod = LodParams { cells: CELLS, max_level: 26, ..LodParams::earth_1080p() };
    let camera = scripted_views(R, 4.0 / 3.0, 1)[4].1;
    let req = TerrainRequest { camera, size, view: TerrainView::Relief, lod, face_mapping: "tangent-v1".into(), seed: 1 };
    let mut plan = plan_terrain(&req).unwrap();
    plan.nodes.clear();
    let field = Reference(1);
    let tiles: Vec<CompactTile> = (0..tiles_per_frame as u32)
        .map(|k| compact_tile_field(&field, &TangentWarp, TileId::new(Face(0), 12, k, 5).unwrap(), CELLS, R))
        .collect();
    let staging_bytes = tiles_per_frame * ((CELLS + 1) * (CELLS + 1) * 8) as usize;

    println!(
        "workers | upload_tiles_at | of it in queue calls | staging (the rest) | heap alloc + fill of {staging_bytes} B | tiles/s built by the workers"
    );
    let mut submitted = 0u64;
    for &n in &counts {
        let stop = Arc::new(AtomicBool::new(false));
        let handles = spawn_workers(n, &stop);
        std::thread::sleep(Duration::from_millis(300));
        let (mut whole, mut queue, mut stage, mut alloc) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let started = Instant::now();
        for f in 0..frames {
            let base = ((f * tiles_per_frame) % 200) as u32;
            let batch: Vec<(&CompactTile, SlotRef)> =
                tiles.iter().enumerate().map(|(i, t)| (t, SlotRef { pool: PoolKind::Main, index: base + i as u32 })).collect();
            let before = renderer.upload_times();
            let t = Instant::now();
            renderer.upload_tiles_at(&ctx, &batch).unwrap();
            let wall = t.elapsed().as_secs_f64() * 1000.0;
            let after = renderer.upload_times();
            let q = (after.queue - before.queue).as_secs_f64() * 1000.0;
            whole.push(wall);
            queue.push(q);
            stage.push(((after.total - before.total).as_secs_f64() * 1000.0 - q).max(0.0));

            renderer.submit(&ctx, &plan).unwrap();
            submitted += 1;
            while submitted - renderer.retired_frames(&ctx) >= 2 {
                renderer.wait_oldest(&ctx);
            }

            let t = Instant::now();
            let mut v = vec![0u8; staging_bytes];
            v[f % staging_bytes] = f as u8;
            std::hint::black_box(&v);
            drop(v);
            alloc.push(t.elapsed().as_secs_f64() * 1000.0);

            std::thread::sleep(sleep);
        }
        let elapsed = started.elapsed().as_secs_f64();
        stop.store(true, Ordering::Relaxed);
        let built: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        println!("{n:>7} | {} | {} | {} | {} | {:.0}", stats(&whole), stats(&queue), stats(&stage), stats(&alloc), built as f64 / elapsed);
    }
    println!("staging buffers of the renderer after the run: {} elements", renderer.batch_scratch_capacity());
}
