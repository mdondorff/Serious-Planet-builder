//! SPIKE-06 measurement helper (docs/spikes.md): the compact tile format against today's 44-byte vertices. Plans a standard view
//! with the production planner, builds both formats for every tile of the draw set, and reports (1) bytes, (2) the CPU mirror of
//! the compact vertex shader against the product mesh (position, coarse position, normal, heights, per level) and the seam
//! mismatch of shared border vertices in both formats, (3) the images of both renderers per debug view, and (4) GPU time
//! (timestamp queries) of both, interleaved. GPU times here are indicative, not a perf-window measurement (ADR 0020).
//!
//! Not production code: `cargo run --release -p planet --features spike06 --example compact_tile -- [--adapter hardware]
//! [--field ref|eroded] [--cells 32|64] [--view ground-1m] [--max-level 26] [--global-res 1024] [--frames 150]`.

#![allow(clippy::type_complexity, clippy::disallowed_methods)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use planet::worldfield::{legacy_world, WorldTerrain};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{TileId, Vec3};
use planet_frame::{plan_terrain_cached, scripted_views_field, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::mesh::{tile_mesh_field, vertex_count, MeshVertex, TileMesh};
use planet_render::spike_compact::{compact_tile, CompactRenderer, CompactTile, TILE_RECORD_BYTES};
use planet_render::{AdapterPolicy, GpuContext, RgbaFrame, TerrainRenderer};
use planet_world_def::WorldDefinition;

const RADIUS_M: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (2560, 1440);

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn has(name: &str) -> bool {
    std::env::args().any(|x| x == name)
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

fn median_p99(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
    (q(0.5), q(0.99), v[0])
}

fn world(local: [f32; 3], origin: Vec3) -> Vec3 {
    origin + Vec3::new(f64::from(local[0]), f64::from(local[1]), f64::from(local[2]))
}

fn diff(a: &RgbaFrame, b: &RgbaFrame) -> (u8, f64, usize, usize) {
    let (mut max, mut sum, mut over8, mut over32) = (0u8, 0u64, 0usize, 0usize);
    for (p, q) in a.rgba.chunks(4).zip(b.rgba.chunks(4)) {
        let d = (0..3).map(|c| p[c].abs_diff(q[c])).max().expect("3");
        max = max.max(d);
        sum += u64::from(d);
        over8 += usize::from(d > 8);
        over32 += usize::from(d > 32);
    }
    (max, sum as f64 / (a.rgba.len() / 4) as f64, over8, over32)
}

fn main() -> Result<(), String> {
    let cells: u32 = arg("--cells").map_or(32, |s| s.parse().expect("cells"));
    let max_level: u8 = arg("--max-level").map_or(26, |s| s.parse().expect("max-level"));
    let view_name = arg("--view").unwrap_or_else(|| "ground-1m".into());
    let frames: usize = arg("--frames").map_or(150, |s| s.parse().expect("frames"));
    let global_res: u32 = arg("--global-res").map_or(1024, |s| s.parse().expect("global-res"));
    let field_kind = arg("--field").unwrap_or_else(|| "eroded".into());
    let policy = if arg("--adapter").as_deref() == Some("hardware") { AdapterPolicy::HighPerformance } else { AdapterPolicy::Software };

    let def = if field_kind == "ref" { legacy_world(1, RADIUS_M) } else { WorldDefinition::single_planet(RADIUS_M, 1) };
    let t = Instant::now();
    let terrain = WorldTerrain::open(def, global_res)?;
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
    let tiles: Vec<TileId> = {
        let mut v: Vec<_> = plan.nodes.iter().map(|n| n.tile).collect();
        v.sort_by_key(|t| t.raw());
        v.dedup();
        v
    };
    println!("view {view_name}, cells {cells}, finest level {max_level}: {} nodes, {} distinct tiles", plan.nodes.len(), tiles.len());

    let t = Instant::now();
    let meshes: Vec<TileMesh> = par_map(&tiles, &|id| tile_mesh_field(field.as_ref(), &TangentWarp, id, cells, RADIUS_M));
    let mesh_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let compacts: Vec<CompactTile> = par_map(&tiles, &|id| compact_tile(field.as_ref(), &TangentWarp, id, cells, RADIUS_M));
    println!("generation on all threads: product mesh {mesh_s:.1} s, compact tile {:.1} s", t.elapsed().as_secs_f64());

    // ---- bytes ----
    let mesh_bytes = vertex_count(cells) as u64 * MeshVertex::STRIDE;
    let c = &compacts[0];
    println!(
        "BYTES per tile at cells {cells}: product vertices {} ({} x 44 B); compact texels {} ({} x 4 B) + record {} = {}; ratio {:.1}x; draw set {:.1} MB -> {:.1} MB",
        mesh_bytes,
        vertex_count(cells),
        c.texel_bytes(),
        c.texels.len(),
        TILE_RECORD_BYTES,
        c.resident_bytes(),
        mesh_bytes as f64 / c.resident_bytes() as f64,
        tiles.len() as f64 * mesh_bytes as f64 / 1e6,
        tiles.len() as f64 * c.resident_bytes() as f64 / 1e6
    );

    // ---- CPU mirror of the shader against the product mesh ----
    #[derive(Default, Clone, Copy)]
    struct Errs {
        pos: f64,
        coarse: f64,
        height: f64,
        coarse_height: f64,
        normal_deg: f64,
        step: f64,
        range: f64,
        tiles: usize,
    }
    let mut by_level: std::collections::BTreeMap<u8, Errs> = Default::default();
    let mut border: HashMap<[u64; 3], Vec<(u8, TileId, Vec3, Vec3)>> = HashMap::new();
    let w = (cells + 1) as usize;
    for (mesh, ct) in meshes.iter().zip(&compacts) {
        let sh = ct.shade_all();
        let e = by_level.entry(mesh.id.level()).or_default();
        e.tiles += 1;
        e.step = e.step.max(f64::from(ct.hr_step));
        e.range = e.range.max(f64::from(ct.hr_step) * 65535.0);
        for (k, (a, b)) in mesh.vertices.iter().zip(&sh).enumerate() {
            let d3 = |p: [f32; 3], q: [f32; 3]| {
                ((f64::from(p[0]) - f64::from(q[0])).powi(2)
                    + (f64::from(p[1]) - f64::from(q[1])).powi(2)
                    + (f64::from(p[2]) - f64::from(q[2])).powi(2))
                .sqrt()
            };
            e.pos = e.pos.max(d3(a.pos, b.pos));
            e.coarse = e.coarse.max(d3(a.coarse, b.coarse));
            e.height = e.height.max(f64::from((a.height - b.height).abs()));
            e.coarse_height = e.coarse_height.max(f64::from((a.coarse_height - b.coarse_height).abs()));
            let dot: f64 = (0..3).map(|c| f64::from(a.normal[c]) * f64::from(b.normal[c])).sum();
            e.normal_deg = e.normal_deg.max(dot.clamp(-1.0, 1.0).acos().to_degrees());
            if k < w * w {
                let (i, j) = ((k % w) as u32, (k / w) as u32);
                if i == 0 || j == 0 || i == cells || j == cells {
                    let dir = mesh.id.sample_direction(&TangentWarp, cells, i, j);
                    border.entry([dir.x.to_bits(), dir.y.to_bits(), dir.z.to_bits()]).or_default().push((
                        mesh.id.level(),
                        mesh.id,
                        world(a.pos, mesh.origin.0),
                        world(b.pos, ct.origin.0),
                    ));
                }
            }
        }
    }
    println!(
        "MODEL ERROR compact (CPU mirror of the vertex shader, f32) against the product mesh, per level (max over tiles and vertices):"
    );
    println!("  level tiles  pos_m       coarse_m    height_m    coarse_h_m  normal_deg  hr_step_m");
    for (l, e) in &by_level {
        println!(
            "  {l:>5} {:>5}  {:.3e}  {:.3e}  {:.3e}  {:.3e}  {:.3}       {:.3e}",
            e.tiles, e.pos, e.coarse, e.height, e.coarse_height, e.normal_deg, e.step
        );
    }
    // Seams: shared border directions seen by two or more tiles.
    let (mut same, mut cross) = ([0f64; 2], [0f64; 2]);
    let (mut n_same, mut n_cross) = (0usize, 0usize);
    for v in border.values() {
        for a in 0..v.len() {
            for b in a + 1..v.len() {
                if v[a].1 == v[b].1 {
                    continue;
                }
                let dm = v[a].2.distance(v[b].2);
                let dc = v[a].3.distance(v[b].3);
                let (acc, n) = if v[a].0 == v[b].0 { (&mut same, &mut n_same) } else { (&mut cross, &mut n_cross) };
                acc[0] = acc[0].max(dm);
                acc[1] = acc[1].max(dc);
                *n += 1;
            }
        }
    }
    println!(
        "SEAMS (world positions of one border direction seen by two tiles, max mismatch in metres): same level {} pairs: product {:.3e} compact {:.3e}; across levels {} pairs: product {:.3e} compact {:.3e}",
        n_same, same[0], same[1], n_cross, cross[0], cross[1]
    );

    if has("--cpu-only") {
        return Ok(());
    }
    // ---- GPU ----
    let ctx = GpuContext::new(policy).map_err(|e| e.to_string())?;
    println!("{}", ctx.adapter_log_line());
    let mut a = TerrainRenderer::new(&ctx, SIZE, cells);
    let mut b = CompactRenderer::new(&ctx, SIZE, cells, tiles.len() as u32);
    for m in &meshes {
        a.upload(&ctx, m).map_err(|e| e.to_string())?;
    }
    for c in &compacts {
        b.upload(&ctx, c).map_err(|e| e.to_string())?;
    }
    println!("uploaded: product {:.1} MB, compact {:.1} MB", a.uploaded_bytes as f64 / 1e6, b.uploaded_bytes as f64 / 1e6);
    for view in
        [TerrainView::Level, TerrainView::Morph, TerrainView::Height, TerrainView::Normals, TerrainView::Relief, TerrainView::MorphHeight]
    {
        let mut p = plan.clone();
        p.view = view;
        a.render(&ctx, &p, false).map_err(|e| e.to_string())?;
        let fa = a.read_color(&ctx);
        b.render(&ctx, &p, false).map_err(|e| e.to_string())?;
        let fb = b.read_color(&ctx);
        let (max, mean, o8, o32) = diff(&fa, &fb);
        println!(
            "IMAGE {:>12}: max channel diff {max}, mean {mean:.4}, pixels over 8: {o8}, over 32: {o32} (of {})",
            view.name(),
            fa.rgba.len() / 4
        );
        if let Some(dir) = arg("--dump") {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            fa.write_png(std::path::Path::new(&format!("{dir}/{}-product.png", view.name()))).map_err(|e| e.to_string())?;
            fb.write_png(std::path::Path::new(&format!("{dir}/{}-compact.png", view.name()))).map_err(|e| e.to_string())?;
        }
    }
    for view in [TerrainView::Level, TerrainView::Relief] {
        let mut p = plan.clone();
        p.view = view;
        let (mut ga, mut gb, mut ca, mut cb) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        // Warm-up for 4 s of alternating frames so both start from the same (boosted) clocks.
        let warm = Instant::now();
        while warm.elapsed().as_secs_f64() < arg("--warm").map_or(4.0, |s| s.parse().expect("warm seconds")) {
            a.render(&ctx, &p, true).map_err(|e| e.to_string())?;
            b.render(&ctx, &p, true).map_err(|e| e.to_string())?;
        }
        // `--blocks`: all frames of A, then all of B (the product perf scene's pattern); default: frame by frame AB, AB, BA, BA ...
        // (a slow drift of the GPU clock hits both formats alike).
        if has("--blocks") {
            for _ in 0..frames * 8 {
                let (t, _) = a.render(&ctx, &p, true).map_err(|e| e.to_string())?;
                ga.extend(t.gpu_ms);
                ca.push(t.cpu_ms);
            }
            for _ in 0..frames * 8 {
                let (t, _) = b.render(&ctx, &p, true).map_err(|e| e.to_string())?;
                gb.extend(t.gpu_ms);
                cb.push(t.cpu_ms);
            }
        }
        for k in 0..if has("--blocks") { 0 } else { frames * 8 } {
            let a_first = (k / 2) % 2 == 0;
            for first in [a_first, !a_first] {
                if first {
                    let (t, _) = a.render(&ctx, &p, true).map_err(|e| e.to_string())?;
                    ga.extend(t.gpu_ms);
                    ca.push(t.cpu_ms);
                } else {
                    let (t, _) = b.render(&ctx, &p, true).map_err(|e| e.to_string())?;
                    gb.extend(t.gpu_ms);
                    cb.push(t.cpu_ms);
                }
            }
        }
        let (gam, gap, gamin) = median_p99(&mut ga);
        let (gbm, gbp, gbmin) = median_p99(&mut gb);
        let (cam_, _, _) = median_p99(&mut ca);
        let (cbm, _, _) = median_p99(&mut cb);
        println!(
            "GPU {:>8} (indicative, {} frames each): product gpu median {gam:.2} ms p99 {gap:.2} min {gamin:.2}, cpu encode {cam_:.2} ms | compact gpu median {gbm:.2} ms p99 {gbp:.2} min {gbmin:.2}, cpu encode {cbm:.2} ms",
            view.name(),
            frames * 8
        );
    }
    Ok(())
}
