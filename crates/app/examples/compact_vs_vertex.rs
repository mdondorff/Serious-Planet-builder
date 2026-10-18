//! Ad hoc comparison of the product's two tile formats (ADR 0033, REND-016): plans a standard view over the eroded field, builds every
//! tile of the draw set in the compact and in the vertex format, draws both through the production `TerrainRenderer` and reports
//! bytes, image differences per debug view, and GPU time (timestamp queries) and CPU encode time, interleaved in one process.
//! INDICATIVE ONLY, not a perf-window measurement (ADR 0020, ADR 0034: GPU milliseconds depend on the clock state).
//!
//! `cargo run --release -p planet --example compact_vs_vertex -- [--adapter hardware] [--view ground-1m] [--cells 32]
//! [--max-level 26] [--global-res 1024] [--frames 150] [--warm 4]`.

#![allow(clippy::disallowed_methods)]

use planet::worldfield::WorldTerrain;
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::TileId;
use planet_frame::{plan_terrain_cached, scripted_views_field, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::mesh::tile_mesh_field;
use planet_render::{AdapterPolicy, DrawMode, GpuContext, RgbaFrame, TerrainRenderer};
use planet_world_def::WorldDefinition;
use std::sync::Mutex;
use std::time::Instant;

const RADIUS_M: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn par_map<T: Send>(tiles: &[TileId], f: &(dyn Fn(TileId) -> T + Sync)) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: Mutex<Vec<Option<T>>> = Mutex::new((0..tiles.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= tiles.len() {
                    break;
                }
                let v = f(tiles[k]);
                out.lock().expect("lock")[k] = Some(v);
            });
        }
    });
    out.into_inner().expect("lock").into_iter().map(|v| v.expect("filled")).collect()
}

fn median_p99(v: &mut [f64]) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
    (q(0.5), q(0.99))
}

fn diff(a: &RgbaFrame, b: &RgbaFrame) -> (u8, usize, usize) {
    let (mut max, mut over12, mut over32) = (0u8, 0usize, 0usize);
    for (p, q) in a.rgba.chunks(4).zip(b.rgba.chunks(4)) {
        let d = (0..3).map(|c| p[c].abs_diff(q[c])).max().expect("3");
        max = max.max(d);
        over12 += usize::from(d > 12);
        over32 += usize::from(d > 32);
    }
    (max, over12, over32)
}

fn main() -> Result<(), String> {
    let cells: u32 = arg("--cells").map_or(32, |s| s.parse().expect("cells"));
    let max_level: u8 = arg("--max-level").map_or(26, |s| s.parse().expect("max-level"));
    let view_name = arg("--view").unwrap_or_else(|| "ground-1m".into());
    let frames: usize = arg("--frames").map_or(150, |s| s.parse().expect("frames"));
    let global_res: u32 = arg("--global-res").map_or(1024, |s| s.parse().expect("global-res"));
    let warm: f64 = arg("--warm").map_or(4.0, |s| s.parse().expect("warm seconds"));
    let policy = if arg("--adapter").as_deref() == Some("hardware") { AdapterPolicy::HighPerformance } else { AdapterPolicy::Software };

    let t = Instant::now();
    let terrain = WorldTerrain::open(WorldDefinition::single_planet(RADIUS_M, 1), global_res)?;
    let field = terrain.field()?;
    println!("{} ({:.1} s to open)", terrain.describe(), t.elapsed().as_secs_f64());
    let views = scripted_views_field(RADIUS_M, f64::from(SIZE.0) / f64::from(SIZE.1), field.as_ref());
    let cam = views.iter().find(|(n, _)| *n == view_name).ok_or("unknown --view")?.1;
    let lod = LodParams {
        radius_m: RADIUS_M,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        cells,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 6.0,
        max_level,
        ..LodParams::earth_1080p()
    };
    let req = TerrainRequest {
        camera: cam,
        size: SIZE,
        view: TerrainView::Level,
        lod,
        face_mapping: TangentWarp.id().to_string(),
        seed: field.seed(),
    };
    let mut cache = SelectionCache::new();
    let plan = plan_terrain_cached(&req, field.as_ref(), &mut cache).map_err(|e| e.to_string())?;
    let mut tiles: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
    tiles.sort_by_key(|t| t.raw());
    tiles.dedup();
    println!("view {view_name}, cells {cells}, finest level {max_level}: {} nodes, {} distinct tiles", plan.nodes.len(), tiles.len());

    let t = Instant::now();
    let vertex: Vec<CompactTile> =
        par_map(&tiles, &|id| RenderTile::Vertex(tile_mesh_field(field.as_ref(), &TangentWarp, id, cells, RADIUS_M)));
    let vertex_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let compact: Vec<CompactTile> = par_map(&tiles, &|id| compact_tile_field(field.as_ref(), &TangentWarp, id, cells, RADIUS_M));
    println!("generation on all threads: vertex format {vertex_s:.1} s, compact format {:.1} s", t.elapsed().as_secs_f64());
    let fallbacks = compact.iter().filter(|t| matches!(t, RenderTile::Vertex(_))).count();
    let compact_tiles = compact.len() - fallbacks;
    println!("compact tiles {compact_tiles}, vertex-format tiles (levels 0-2 and fallbacks) {fallbacks}");

    let ctx = GpuContext::new(policy).map_err(|e| e.to_string())?;
    println!("{}", ctx.adapter_log_line());
    let mut a = TerrainRenderer::new(&ctx, SIZE, cells);
    let mut b = TerrainRenderer::new_compact(&ctx, SIZE, cells);
    for t in &vertex {
        a.upload_tile(&ctx, t).map_err(|e| e.to_string())?;
    }
    for t in &compact {
        b.upload_tile(&ctx, t).map_err(|e| e.to_string())?;
    }
    println!(
        "uploaded: vertex {:.1} MB, compact {:.1} MB; allocated by the pools: vertex {:.1} MB, compact {:.1} MB",
        a.uploaded_bytes as f64 / 1e6,
        b.uploaded_bytes as f64 / 1e6,
        a.pool_allocated_bytes() as f64 / 1e6,
        b.pool_allocated_bytes() as f64 / 1e6
    );
    for view in [TerrainView::Level, TerrainView::Morph, TerrainView::Normals, TerrainView::Relief, TerrainView::MorphHeight] {
        let mut p = plan.clone();
        p.view = view;
        a.render(&ctx, &p, false).map_err(|e| e.to_string())?;
        let fa = a.read_color(&ctx);
        b.render(&ctx, &p, false).map_err(|e| e.to_string())?;
        let fb = b.read_color(&ctx);
        let (max, o12, o32) = diff(&fa, &fb);
        println!("IMAGE {:>12}: max channel diff {max}, pixels over 12: {o12}, over 32: {o32} (of {})", view.name(), fa.rgba.len() / 4);
    }
    for mode in [DrawMode::Indirect, DrawMode::PerDraw] {
        a.set_draw_mode(&ctx, mode);
        b.set_draw_mode(&ctx, mode);
        for view in [TerrainView::Level, TerrainView::Relief] {
            let mut p = plan.clone();
            p.view = view;
            let (mut ga, mut gb, mut ca, mut cb) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            let started = Instant::now();
            while started.elapsed().as_secs_f64() < warm {
                a.render(&ctx, &p, true).map_err(|e| e.to_string())?;
                b.render(&ctx, &p, true).map_err(|e| e.to_string())?;
            }
            // Interleaved AB, BA pairs so a drifting clock hits both formats alike.
            for k in 0..frames * 4 {
                let a_first = (k / 2) % 2 == 0;
                for first in [a_first, !a_first] {
                    let (t, _) = if first { a.render(&ctx, &p, true) } else { b.render(&ctx, &p, true) }.map_err(|e| e.to_string())?;
                    if first {
                        ga.extend(t.gpu_ms);
                        ca.push(t.cpu_ms);
                    } else {
                        gb.extend(t.gpu_ms);
                        cb.push(t.cpu_ms);
                    }
                }
            }
            let ((gam, gap), (gbm, gbp)) = (median_p99(&mut ga), median_p99(&mut gb));
            let ((cam, _), (cbm, _)) = (median_p99(&mut ca), median_p99(&mut cb));
            println!(
                "{} {:>8} (indicative, {} frames each): vertex gpu median {gam:.2} ms p99 {gap:.2}, cpu encode {cam:.2} ms | compact gpu median {gbm:.2} ms p99 {gbp:.2}, cpu encode {cbm:.2} ms | ratio {:.2}",
                mode.name(),
                view.name(),
                frames * 4,
                gbm / gam
            );
        }
    }
    Ok(())
}
