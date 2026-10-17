//! SPIKE-12 (vertex retirement) experiments. Throwaway, spike branch only; nothing here is a product test.
//!
//! Run one at a time: `PLANET_ADAPTER=software|hardware cargo test --release -p planet-render --test spike12 <name> -- --nocapture --ignored --test-threads=1`.
//! The prototype switches are process-global statics, so the experiments serialise on `LOCK`.
//! Results go to `spikes/vertex-retirement/results/` (text) and `.../images/` (PNG).

#![allow(clippy::disallowed_methods, unused_imports, dead_code)]

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::TileId;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, plan_terrain_field, scripted_views, scripted_views_field, spike_set_floor, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{
    compact_tile_field, record_stride, spike_set, tile_data_field, vertex_tile_field, RenderTile, SPIKE_BASE5, SPIKE_BOTH, SPIKE_COARSE,
    SPIKE_V2_SHADER, SPIKE_WIDE,
};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::tile_mesh_field;
use planet_generators::stress::Steep;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_testkit::{write_png, Image};
use std::fmt::Write as _;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

const R: f64 = 6_371_000.0;
const CELLS: u32 = 32;
const SEED: u64 = 1;

fn out_dir() -> std::path::PathBuf {
    let d = planet_testkit::repo_root().join("spikes/vertex-retirement/results");
    std::fs::create_dir_all(d.join("images")).unwrap();
    d
}

fn set(flags: u32, floor: u8) {
    spike_set(flags);
    spike_set_floor(floor);
}

fn reset() {
    set(0, 0);
}

fn lod(size: (u32, u32), tau: f64) -> LodParams {
    LodParams { cells: CELLS, viewport_h_px: f64::from(size.1), tau_px: tau, max_level: planet_frame::production_finest_level(CELLS), ..LodParams::earth_1080p() }
}

fn plan_at(index: usize, size: (u32, u32), view: TerrainView, tau: f64) -> (&'static str, TerrainPlan) {
    let (name, camera) = scripted_views(R, f64::from(size.0) / f64::from(size.1), SEED)[index];
    (name, plan_terrain(&TerrainRequest { camera, size, view, lod: lod(size, tau), face_mapping: "tangent-v1".into(), seed: 1 }).unwrap())
}

/// Tiles of a plan built in parallel under the current switches.
fn tiles_of(plan: &TerrainPlan, field: &(dyn HeightField + Sync), finest: u8) -> Vec<RenderTile> {
    let nodes: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
    let threads = 16usize;
    let chunk = nodes.len().div_ceil(threads).max(1);
    let mut out: Vec<Vec<RenderTile>> = Vec::new();
    std::thread::scope(|s| {
        let hs: Vec<_> = nodes
            .chunks(chunk)
            .map(|c| s.spawn(move || c.iter().map(|t| tile_data_field(field, &TangentWarp, *t, CELLS, R, finest)).collect::<Vec<_>>()))
            .collect();
        for h in hs {
            out.push(h.join().unwrap());
        }
    });
    out.into_iter().flatten().collect()
}

fn pool() -> planet_streaming::PoolConfig {
    compact_format_pool_with(1 << 30, CELLS, 4096, 1).unwrap()
}

/// A persistent renderer built and filled under the current switches.
fn renderer(ctx: &GpuContext, size: (u32, u32), tiles: &[RenderTile]) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_compact_pools(ctx, size, CELLS, pool());
    r.warm_up(ctx);
    for t in tiles {
        r.upload_tile(ctx, t).unwrap();
    }
    r
}

fn draw_all(ctx: &GpuContext, plan: &TerrainPlan, tiles: &[RenderTile]) -> Vec<Vec<u8>> {
    let mut r = renderer(ctx, plan.size, tiles);
    TerrainView::ALL
        .iter()
        .map(|v| {
            let mut p = plan.clone();
            p.view = *v;
            r.render(ctx, &p, false).unwrap();
            r.read_color(ctx).rgba
        })
        .collect()
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 - 1.0) * q).round() as usize]
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T1 (CPU): position error of the base surface per level, 3 x 3 against 5 x 5, actual builder and mirror, in metres and in pixels.

fn dist3(a: [f32; 3], b: [f32; 3]) -> f64 {
    (0..3).map(|k| (f64::from(a[k]) - f64::from(b[k])).powi(2)).sum::<f64>().sqrt()
}

#[test]
#[ignore]
fn t1_base_error_by_level() {
    let _g = LOCK.lock().unwrap();
    let field = Reference(SEED);
    let mut rng = SplitMix64(12);
    let mut out = String::new();
    writeln!(out, "# T1: position error of the compact base surface against the vertex-format mesh, by level (CPU, Reference field, cells {CELLS})").unwrap();
    writeln!(out, "Tiles: 24 random per level (seed 12). Error = largest distance over all grid and skirt vertices of position and morph target between the CPU mirror (shade_all) and tile_mesh_field. px = metres / (tile edge / 190), SPIKE-06's convention (a 190-pixel tile).\n").unwrap();
    writeln!(out, "| level | tile edge m | 3x3 max m | 3x3 max px | 5x5 max m | 5x5 max px | 5x5 p50 px |").unwrap();
    writeln!(out, "|---|---|---|---|---|---|---|").unwrap();
    for level in 0..=8u8 {
        let n = 1u64 << level;
        let tiles: Vec<TileId> = (0..24)
            .map(|_| TileId::new(planet_core::Face((rng.next_u64() % 6) as u8), level, (rng.next_u64() % n) as u32, (rng.next_u64() % n) as u32).unwrap())
            .collect();
        let edge = R * core::f64::consts::FRAC_PI_2 / n as f64;
        let mut row = Vec::new();
        for flags in [SPIKE_COARSE, SPIKE_COARSE | SPIKE_BASE5] {
            set(flags, 0);
            let mut errs = Vec::new();
            for id in &tiles {
                let mesh = tile_mesh_field(&field, &TangentWarp, *id, CELLS, R);
                let ct = compact_tile_field(&field, &TangentWarp, *id, CELLS, R);
                let shaded = ct.shade_all();
                let e = mesh.vertices.iter().zip(&shaded).map(|(a, b)| dist3(a.pos, b.pos).max(dist3(a.coarse, b.coarse))).fold(0.0f64, f64::max);
                errs.push(e);
            }
            reset();
            let px = |m: f64| m / (edge / 190.0);
            let mut pxs: Vec<f64> = errs.iter().map(|e| px(*e)).collect();
            let max = errs.iter().copied().fold(0.0, f64::max);
            row.push((max, px(max), pct(&mut pxs, 0.5)));
        }
        writeln!(out, "| {level} | {edge:.3e} | {:.3e} | {:.3} | {:.3e} | {:.5} | {:.5} |", row[0].0, row[0].1, row[1].0, row[1].1, row[1].2).unwrap();
    }
    println!("{out}");
    std::fs::write(out_dir().join("t1-base-error-by-level.md"), out).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T2 (CPU): base-surface error in screen pixels for the nodes actually drawn at orbit when levels 0-2 are compact, and when the floor is 3.

#[test]
#[ignore]
fn t2_orbit_error_in_screen_pixels() {
    let _g = LOCK.lock().unwrap();
    let size = (2560u32, 1440u32);
    let field = Reference(SEED);
    let mut out = String::new();
    writeln!(out, "# T2: screen-space error of the compact base surface at the standard views (2560x1440, tau 6, fov 60, Reference field, cells {CELLS})").unwrap();
    writeln!(out, "Per drawn node: error = largest vertex distance between the compact mirror and the vertex mesh (position and morph target); pixel size at the node = distance to the node's near side * 2 tan(fov/2) / 1440. Only nodes of level < 3 are listed per view (the others are compact today).\n").unwrap();
    writeln!(out, "| view | base | level 0-2 nodes | worst error m | worst error px | mean error px |").unwrap();
    writeln!(out, "|---|---|---|---|---|---|").unwrap();
    for index in 0..10usize {
        reset();
        let (name, plan) = plan_at(index, size, TerrainView::Relief, 6.0);
        let coarse: Vec<_> = plan.nodes.iter().filter(|n| n.level < 3).collect();
        if coarse.is_empty() {
            continue;
        }
        let k = 2.0 * libm::tan(plan.camera.fov_y_rad / 2.0) / f64::from(size.1);
        for (label, flags) in [("3x3", SPIKE_COARSE), ("5x5", SPIKE_COARSE | SPIKE_BASE5)] {
            set(flags, 0);
            let mut pxs = Vec::new();
            let mut worst_m = 0.0f64;
            for n in &coarse {
                let mesh = tile_mesh_field(&field, &TangentWarp, n.tile, CELLS, R);
                let ct = compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R);
                let e = mesh.vertices.iter().zip(&ct.shade_all()).map(|(a, b)| dist3(a.pos, b.pos).max(dist3(a.coarse, b.coarse))).fold(0.0, f64::max);
                let rel = n.camera_relative_origin;
                let dist = (f64::from(rel[0]).powi(2) + f64::from(rel[1]).powi(2) + f64::from(rel[2]).powi(2)).sqrt();
                let edge = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << n.level);
                let near = (dist - 0.71 * edge).max(0.3 * dist);
                worst_m = worst_m.max(e);
                pxs.push(e / (near * k));
            }
            reset();
            let worst = pxs.iter().copied().fold(0.0, f64::max);
            let mean = pxs.iter().sum::<f64>() / pxs.len() as f64;
            writeln!(out, "| {name} | {label} | {} | {worst_m:.3e} | {worst:.4} | {mean:.4} |", coarse.len()).unwrap();
        }
    }
    println!("{out}");
    std::fs::write(out_dir().join("t2-orbit-error-px.md"), out).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T3 (CPU): the level-3 floor: draws and slots at the ten standard views.

#[test]
#[ignore]
fn t3_floor_counts() {
    let _g = LOCK.lock().unwrap();
    let size = (2560u32, 1440u32);
    let mut out = String::new();
    writeln!(out, "# T3: render floor at level 3 against none, ten standard views (2560x1440, tau 6, cells {CELLS}, finest level 22)").unwrap();
    writeln!(out, "nodes = draw records of the plan (one per tile); runs = draw runs of the indirect list (one bind-group change each); levels = histogram of drawn levels (level:count).\n").unwrap();
    writeln!(out, "| view | floor | nodes | levels 0-2 | runs | levels |").unwrap();
    writeln!(out, "|---|---|---|---|---|---|").unwrap();
    let mut totals = [(0usize, 0usize); 2];
    for index in 0..10usize {
        for (k, floor) in [0u8, 3].into_iter().enumerate() {
            set(0, floor);
            let (name, plan) = plan_at(index, size, TerrainView::Relief, 6.0);
            reset();
            let mut hist = std::collections::BTreeMap::new();
            for n in &plan.nodes {
                *hist.entry(n.level).or_insert(0usize) += 1;
            }
            let coarse = plan.nodes.iter().filter(|n| n.level < 3).count();
            totals[k].0 += plan.nodes.len();
            totals[k].1 += coarse;
            let levels = hist.iter().map(|(l, c)| format!("{l}:{c}")).collect::<Vec<_>>().join(" ");
            writeln!(out, "| {name} | {floor} | {} | {coarse} | - | {levels} |", plan.nodes.len()).unwrap();
        }
    }
    writeln!(out, "\nTotals over the ten views: floor 0: {} nodes ({} at levels 0-2); floor 3: {} nodes ({} at levels 0-2).", totals[0].0, totals[0].1, totals[1].0, totals[1].1).unwrap();
    println!("{out}");
    std::fs::write(out_dir().join("t3-floor-counts.md"), out).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T4 (GPU): image differences of the candidates against the current vertex path, orbit and aerial views, all nine debug views.

fn diff_stats(a: &[u8], b: &[u8]) -> (usize, u8, usize, f64) {
    let (mut n, mut max, mut over8, mut sum) = (0usize, 0u8, 0usize, 0u64);
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = (0..3).map(|k| p[k].abs_diff(q[k])).max().unwrap();
        if d > 0 {
            n += 1;
            max = max.max(d);
            sum += u64::from(d);
        }
        if d > 8 {
            over8 += 1;
        }
    }
    (n, max, over8, if n > 0 { sum as f64 / n as f64 } else { 0.0 })
}

fn diff_image(a: &[u8], b: &[u8], w: u32, h: u32, gain: u32) -> Image {
    let mut rgba = Vec::with_capacity(a.len());
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = (0..3).map(|k| u32::from(p[k].abs_diff(q[k]))).max().unwrap();
        let v = (d * gain).min(255) as u8;
        rgba.extend([v, v, v, 255]);
    }
    Image::new(w, h, rgba)
}

/// Panels side by side, each cropped to `crop` (x, y, w, h) of the source frames and scaled by `zoom` (nearest), with a 4-pixel gap.
fn sheet(panels: &[(&[u8], bool)], w: u32, crop: (u32, u32, u32, u32), zoom: u32) -> Image {
    let (cx, cy, cw, ch) = crop;
    let (pw, ph) = (cw * zoom, ch * zoom);
    let total_w = panels.len() as u32 * pw + (panels.len() as u32 - 1) * 4;
    let mut rgba = vec![40u8; (total_w * ph * 4) as usize];
    for (k, (frame, gray)) in panels.iter().enumerate() {
        for y in 0..ph {
            for x in 0..pw {
                let (sx, sy) = (cx + x / zoom, cy + y / zoom);
                let s = ((sy * w + sx) * 4) as usize;
                let d = (((y * total_w) + k as u32 * (pw + 4) + x) * 4) as usize;
                rgba[d..d + 4].copy_from_slice(&frame[s..s + 4]);
                if *gray {
                    rgba[d + 3] = 255;
                }
            }
        }
    }
    Image::new(total_w, ph, rgba)
}

#[test]
#[ignore]
fn t4_images_levels_0_to_2() {
    let _g = LOCK.lock().unwrap();
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let log = ctx.adapter_log_line();
    let tag = if log.contains("Cpu") { "warp" } else { "rtx" };
    let size = (960u32, 540u32);
    let field = Reference(SEED);
    let finest = planet_frame::production_finest_level(CELLS);
    let mut report = String::new();
    writeln!(report, "# T4: image differences of the candidates against the current vertex path ({log})").unwrap();
    writeln!(report, "Reference field, {}x{}, tau 6, cells {CELLS}. Candidates: A = 5x5 base surface, levels 0-2 compact (same plan as the vertex path); A0 = 3x3 base surface, levels 0-2 compact (why 5x5 is needed); B = render floor 3 (a different plan: level-3 tiles instead of levels 0-2, compared with the vertex path's own plan). diff = pixels with any channel different; max = largest channel difference (0-255); over8 = pixels differing by more than 8; mean = mean difference of the differing pixels.\n", size.0, size.1).unwrap();
    writeln!(report, "| view | debug view | candidate | frame nodes | diff px | of | max | over8 | mean |").unwrap();
    writeln!(report, "|---|---|---|---|---|---|---|---|---|").unwrap();
    for index in [0usize, 1, 2] {
        reset();
        let (name, base_plan) = plan_at(index, size, TerrainView::Relief, 6.0);
        let base_tiles = tiles_of(&base_plan, &field, finest);
        set(SPIKE_COARSE | SPIKE_BASE5, 0);
        let a_tiles = tiles_of(&base_plan, &field, finest);
        let a_frames = draw_all(&ctx, &base_plan, &a_tiles);
        set(SPIKE_COARSE, 0);
        let a0_tiles = tiles_of(&base_plan, &field, finest);
        let a0_frames = draw_all(&ctx, &base_plan, &a0_tiles);
        set(0, 3);
        let (_, floor_plan) = plan_at(index, size, TerrainView::Relief, 6.0);
        let b_tiles = tiles_of(&floor_plan, &field, finest);
        let b_frames = draw_all(&ctx, &floor_plan, &b_tiles);
        reset();
        let ref_frames = draw_all(&ctx, &base_plan, &base_tiles);
        for (vi, v) in TerrainView::ALL.iter().enumerate() {
            for (label, frame, nodes) in [("A (5x5)", &a_frames[vi], base_plan.nodes.len()), ("A0 (3x3)", &a0_frames[vi], base_plan.nodes.len()), ("B (floor 3)", &b_frames[vi], floor_plan.nodes.len())] {
                let (n, max, over8, mean) = diff_stats(&ref_frames[vi], frame);
                writeln!(report, "| {name} | {} | {label} | {nodes} | {n} | {} | {max} | {over8} | {mean:.2} |", v.name(), size.0 * size.1).unwrap();
            }
        }
        // Visuals for the Relief view (index 7) and the Normals view (index 5).
        for (vi, v) in TerrainView::ALL.iter().enumerate().filter(|(_, v)| matches!(v, TerrainView::Relief | TerrainView::Normals)).filter(|_| index != 1) {
            let dir = out_dir().join("images");
            let (full, zoom) = if index == 0 { ((355, 150, 240, 240), 2) } else { ((0, 0, size.0, size.1), 1) };
            // Crop to the planet: the orbit view shows a disc; use the whole frame at half size for the sheet.
            let diff_a = diff_image(&ref_frames[vi], &a_frames[vi], size.0, size.1, 16);
            let diff_b = diff_image(&ref_frames[vi], &b_frames[vi], size.0, size.1, 16);
            let s = sheet(&[(&ref_frames[vi], false), (&a_frames[vi], false), (&diff_a.rgba, true)], size.0, full, zoom);
            write_png(&dir.join(format!("{tag}-{name}-{}-vertex-vs-5x5-vs-diff16x.png", v.name())), &s).unwrap();
            let s = sheet(&[(&ref_frames[vi], false), (&b_frames[vi], false), (&diff_b.rgba, true)], size.0, full, zoom);
            write_png(&dir.join(format!("{tag}-{name}-{}-vertex-vs-floor3-vs-diff16x.png", v.name())), &s).unwrap();
            if index == 0 {
                let s = sheet(&[(&ref_frames[vi], false), (&a0_frames[vi], false), (&diff_image(&ref_frames[vi], &a0_frames[vi], size.0, size.1, 16).rgba, true)], size.0, full, zoom);
                write_png(&dir.join(format!("{tag}-{name}-{}-vertex-vs-3x3-vs-diff16x.png", v.name())), &s).unwrap();
            }
        }
    }
    println!("{report}");
    std::fs::write(out_dir().join(format!("t4-image-diff-{tag}.md")), report).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T5 (GPU): indicative GPU time of the candidates at orbit and aerial-200km. Interleaved A B C D, medians; the label is in the file.

fn timed_candidates(ctx: &GpuContext, index: usize, size: (u32, u32), frames: usize, candidates: &[(&str, u32, u8, bool)], field: &(dyn HeightField + Sync)) -> String {
    let finest = planet_frame::production_finest_level(CELLS);
    let mut rs: Vec<(String, TerrainRenderer, TerrainPlan)> = Vec::new();
    let mut out = String::new();
    for (label, flags, floor, _) in candidates {
        set(*flags, *floor);
        let (_, plan) = plan_at(index, size, TerrainView::Relief, 6.0);
        let tiles = tiles_of(&plan, field, finest);
        let r = renderer(ctx, size, &tiles);
        rs.push((format!("{label} ({} nodes, record {} B)", plan.nodes.len(), record_stride()), r, plan));
        reset();
    }
    for _ in 0..30 {
        for (_, r, p) in rs.iter_mut() {
            r.render(ctx, p, true).unwrap();
        }
    }
    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); rs.len()];
    for i in 0..frames {
        let n = rs.len();
        for k in 0..n {
            let which = if i % 2 == 0 { k } else { n - 1 - k };
            let (_, r, p) = &mut rs[which];
            let (t, _) = r.render(ctx, p, true).unwrap();
            samples[which].push(t.gpu_ms.unwrap_or(f64::NAN));
        }
    }
    let base = pct(&mut samples[0].clone(), 0.5);
    for (k, (label, _, _)) in rs.iter().enumerate() {
        let (m, lo, hi) = (pct(&mut samples[k].clone(), 0.5), pct(&mut samples[k].clone(), 0.1), pct(&mut samples[k].clone(), 0.9));
        writeln!(out, "| {label} | {m:.3} | {lo:.3} | {hi:.3} | {:.3} |", m / base).unwrap();
    }
    out
}

#[test]
#[ignore]
fn t5_gpu_time_orbit_and_aerial() {
    let _g = LOCK.lock().unwrap();
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let log = ctx.adapter_log_line();
    let size = (2560u32, 1440u32);
    let frames: usize = std::env::var("FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
    let field = Reference(SEED);
    let started = std::time::SystemTime::now();
    let mut out = String::new();
    writeln!(out, "# T5: indicative GPU time of the render pass, levels 0-2 candidates ({log})").unwrap();
    writeln!(out, "2560x1440, Relief debug view, Reference field, cells {CELLS}, tau 6, {frames} frames interleaved in rotating order, GPU timestamp of the render pass, median / p10 / p90 in ms, ratio to the first row. **Indicative, not a perf window.**\n").unwrap();
    let cands: Vec<(&str, u32, u8, bool)> = vec![
        ("current: levels 0-2 vertex format, 3x3 compact", 0, 0, false),
        ("A: levels 0-2 compact, 3x3 (not accurate)", SPIKE_COARSE, 0, false),
        ("A: levels 0-2 compact, 5x5 base (all tiles)", SPIKE_COARSE | SPIKE_BASE5, 0, false),
        ("B: render floor 3, 3x3 compact", 0, 3, false),
        ("B+5x5: floor 3, 5x5 base (all tiles)", SPIKE_BASE5, 3, false),
    ];
    for index in [0usize, 2] {
        let (name, _) = plan_at(index, size, TerrainView::Relief, 6.0);
        writeln!(out, "## {name}\n\n| candidate | median ms | p10 | p90 | ratio |\n|---|---|---|---|---|").unwrap();
        out += &timed_candidates(&ctx, index, size, frames, &cands, &field);
        out.push('\n');
    }
    writeln!(out, "Label: indicative, taken from {started:?} for about {:?}; Rasierklinge, adapter above; GPU clock state not controlled; no perf window; the 3x3 rows build the product's own shader through the flat-record generator only where a flag selects it.", started.elapsed().unwrap_or_default()).unwrap();
    println!("{out}");
    let tag = if log.contains("Cpu") { "warp" } else { "rtx" };
    std::fs::write(out_dir().join(format!("t5-gpu-time-orbit-aerial-{tag}.md")), out).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T6 (CPU): the wide texel on the steep stress field: CON-13 (1 mm) and CON-14 (0.01 degrees), against the vertex-format mesh.

fn angle_deg(a: [f32; 3], b: [f32; 3]) -> f64 {
    let (a, b) = (
        planet_core::Vec3::new(f64::from(a[0]), f64::from(a[1]), f64::from(a[2])),
        planet_core::Vec3::new(f64::from(b[0]), f64::from(b[1]), f64::from(b[2])),
    );
    libm::atan2(a.cross(b).length(), a.dot(b)).to_degrees()
}

#[test]
#[ignore]
fn t6_wide_texel_con13_con14() {
    let _g = LOCK.lock().unwrap();
    let steep = Steep(1);
    let reference = Reference(SEED);
    let mut rng = SplitMix64(77);
    let mut out = String::new();
    writeln!(out, "# T6: wide texel (32-bit height, 15-bit normals) against the 16-bit texel, CPU mirror against tile_mesh_field, cells {CELLS}").unwrap();
    writeln!(out, "Per level: 6 random tiles. relief = largest relief of the sampled tiles; step = largest height step (hr_step); h err = largest |height of the mirror - height of the mesh| (metres, raw: includes the f32 rounding of both); n err = largest angle between the decoded normal and the mesh normal (degrees, CON-14 bound 0.01); CON-13 = step <= 1 mm.\n").unwrap();
    writeln!(out, "| field | level | texel | relief m | step mm | h err mm | n err deg | CON-13 | CON-14 |").unwrap();
    writeln!(out, "|---|---|---|---|---|---|---|---|---|").unwrap();
    for (fname, field) in [("steep", &steep as &dyn HeightField), ("reference", &reference as &dyn HeightField)] {
        for level in [9u8, 12, 16, 22] {
            let n = 1u64 << level;
            let tiles: Vec<TileId> = (0..6)
                .map(|_| TileId::new(planet_core::Face((rng.next_u64() % 6) as u8), level, (rng.next_u64() % n) as u32, (rng.next_u64() % n) as u32).unwrap())
                .collect();
            for (label, flags) in [("16-bit", 0u32), ("wide", SPIKE_WIDE)] {
                set(flags, 0);
                let (mut relief, mut step, mut herr, mut nerr) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                for id in &tiles {
                    let mesh = tile_mesh_field(field, &TangentWarp, *id, CELLS, R);
                    let ct = compact_tile_field(field, &TangentWarp, *id, CELLS, R);
                    let shaded = ct.shade_all();
                    relief = relief.max(ct.relief_m);
                    step = step.max(f64::from(ct.hr_step));
                    for (a, b) in mesh.vertices.iter().zip(&shaded) {
                        herr = herr.max(f64::from((a.height - b.height).abs()));
                        nerr = nerr.max(angle_deg(a.normal, b.normal));
                    }
                }
                reset();
                writeln!(
                    out,
                    "| {fname} | {level} | {label} | {relief:.1} | {:.4} | {:.3} | {nerr:.5} | {} | {} |",
                    step * 1e3,
                    herr * 1e3,
                    if step <= 1.0e-3 * 1.000001 { "ok" } else { "FAIL (step)" },
                    if nerr <= 0.01 { "ok" } else { "FAIL" }
                )
                .unwrap();
            }
        }
    }
    println!("{out}");
    std::fs::write(out_dir().join("t6-wide-texel-con13-con14.md"), out).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T7 (GPU): cliff texel on the steep field against the vertex-format (CPU mesh) path.

fn half(i: &Image) -> Image {
    let (w, h) = (i.width / 2, i.height / 2);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let s = |dx: u32, dy: u32| u32::from(i.rgba[(((2 * y + dy) * i.width + 2 * x + dx) * 4 + c) as usize]);
                rgba.push(((s(0, 0) + s(1, 0) + s(0, 1) + s(1, 1)) / 4) as u8);
            }
        }
    }
    Image::new(w, h, rgba)
}

fn steep_tiles(plan: &TerrainPlan, cells: u32, finest: u8, vertex: bool) -> Vec<RenderTile> {
    let nodes: Vec<(TileId, u64)> = plan.nodes.iter().map(|n| (n.tile, n.mesh_key)).collect();
    let chunk = nodes.len().div_ceil(16).max(1);
    let mut parts: Vec<Vec<RenderTile>> = Vec::new();
    std::thread::scope(|s| {
        let hs: Vec<_> = nodes
            .chunks(chunk)
            .map(|c| {
                s.spawn(move || {
                    c.iter()
                        .map(|(t, key)| {
                            if vertex {
                                let mut m = vertex_tile_field(&Steep(1), &TangentWarp, *t, cells, R, finest);
                                m.key = *key;
                                RenderTile::Vertex(m)
                            } else {
                                let mut c = compact_tile_field(&Steep(1), &TangentWarp, *t, cells, R);
                                c.key = *key;
                                RenderTile::Compact(c)
                            }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in hs {
            parts.push(h.join().unwrap());
        }
    });
    parts.into_iter().flatten().collect()
}

fn one_shot(ctx: &GpuContext, plan: &TerrainPlan, flags: u32, tiles: &[RenderTile], view: TerrainView) -> Vec<u8> {
    set(flags, 0);
    let refs: Vec<&RenderTile> = tiles.iter().collect();
    let mut p = plan.clone();
    p.view = view;
    let (frame, _) = planet_render::execute_terrain_tiles(ctx, &p, &refs).unwrap();
    reset();
    frame.rgba
}

#[test]
#[ignore]
fn t7_steep_images() {
    let _g = LOCK.lock().unwrap();
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let log = ctx.adapter_log_line();
    let tag = if log.contains("Cpu") { "warp" } else { "rtx" };
    let steep = Steep(1);
    let size = (960u32, 540u32);
    let cells = 16u32;
    let finest = 12u8;
    let l = LodParams { cells, viewport_h_px: f64::from(size.1), tau_px: 4.0, max_level: finest, ..LodParams::earth_1080p() };
    let mut report = String::new();
    writeln!(report, "# T7: steep field ({log}): vertex path (CPU mesh as vertex buffers) against compact 16-bit texels (relief guard off, so cliff tiles are quantised in 16 bits) and against the wide texel").unwrap();
    writeln!(report, "Steep(1), cells {cells}, finest level {finest} (tiles of about 1.5 km with kilometres of relief: the CON-13 fallback case), {}x{}. Pixel counts as in T4.\n", size.0, size.1).unwrap();
    writeln!(report, "| view | nodes | debug view | candidate | diff px | max | over8 | mean |\n|---|---|---|---|---|---|---|---|").unwrap();
    for (index, (name, camera)) in scripted_views_field(R, f64::from(size.0) / f64::from(size.1), &steep).into_iter().enumerate() {
        if ![2usize, 4, 5].contains(&index) {
            continue;
        }
        let req = TerrainRequest { camera, size, view: TerrainView::Relief, lod: l.clone(), face_mapping: "tangent-v1".into(), seed: 1 };
        let plan = plan_terrain_field(&req, &steep).unwrap();
        set(0, 0);
        let vt = steep_tiles(&plan, cells, finest, true);
        let nt = steep_tiles(&plan, cells, finest, false);
        set(SPIKE_WIDE, 0);
        let wt = steep_tiles(&plan, cells, finest, false);
        reset();
        for v in [TerrainView::Normals, TerrainView::Relief, TerrainView::Height] {
            let r = one_shot(&ctx, &plan, 0, &vt, v);
            let n = one_shot(&ctx, &plan, 0, &nt, v);
            let w = one_shot(&ctx, &plan, SPIKE_WIDE, &wt, v);
            for (label, f) in [("compact 16-bit", &n), ("compact wide", &w)] {
                let (cnt, max, over8, mean) = diff_stats(&r, f);
                writeln!(report, "| {name} | {} | {} | {label} | {cnt} | {max} | {over8} | {mean:.2} |", plan.nodes.len(), v.name()).unwrap();
            }
            if (matches!(v, TerrainView::Normals | TerrainView::Relief) && index == 4) || (matches!(v, TerrainView::Height) && index == 5) {
                let gain = if matches!(v, TerrainView::Height) { 64 } else { 8 };
                let dn = diff_image(&r, &n, size.0, size.1, gain);
                let dw = diff_image(&r, &w, size.0, size.1, gain);
                let s = sheet(&[(&r, false), (&n, false), (&w, false), (&dn.rgba, true), (&dw.rgba, true)], size.0, (0, 0, size.0, size.1), 1);
                write_png(&out_dir().join("images").join(format!("{tag}-steep-{name}-{}-vertex-16bit-wide-diff16bit-diffwide.png", v.name())), &half(&s)).unwrap();
            }
        }
    }
    println!("{report}");
    std::fs::write(out_dir().join(format!("t7-steep-images-{tag}.md")), report).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T8 (GPU): indicative GPU time of the texel and record layouts at ground-1m (Reference) and on the steep field.

fn time_table(ctx: &GpuContext, mut rs: Vec<(String, TerrainRenderer, TerrainPlan)>, frames: usize) -> String {
    for _ in 0..30 {
        for (_, r, p) in rs.iter_mut() {
            r.render(ctx, p, true).unwrap();
        }
    }
    let n = rs.len();
    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); n];
    for i in 0..frames {
        for k in 0..n {
            let which = if i % 2 == 0 { k } else { n - 1 - k };
            let (_, r, p) = &mut rs[which];
            let (t, _) = r.render(ctx, p, true).unwrap();
            samples[which].push(t.gpu_ms.unwrap_or(f64::NAN));
        }
    }
    let base = pct(&mut samples[0].clone(), 0.5);
    let mut out = String::new();
    for (k, (label, _, _)) in rs.iter().enumerate() {
        let (m, lo, hi) = (pct(&mut samples[k].clone(), 0.5), pct(&mut samples[k].clone(), 0.1), pct(&mut samples[k].clone(), 0.9));
        writeln!(out, "| {label} | {m:.3} | {lo:.3} | {hi:.3} | {:.3} |", m / base).unwrap();
    }
    out
}

#[test]
#[ignore]
fn t8_gpu_time_texel_layouts() {
    let _g = LOCK.lock().unwrap();
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let log = ctx.adapter_log_line();
    let tag = if log.contains("Cpu") { "warp" } else { "rtx" };
    let frames: usize = std::env::var("FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
    let started = std::time::SystemTime::now();
    let mut out = String::new();
    writeln!(out, "# T8: indicative GPU time of the render pass, texel and record layouts ({log})").unwrap();
    writeln!(out, "Relief debug view, 2560x1440, {frames} frames interleaved in rotating order (first row = baseline), GPU timestamp of the render pass, median / p10 / p90 ms, ratio to the first row. **Indicative, not a perf window.** Every row uses storage-buffer blocks (ADR 0054).\n").unwrap();
    let cands: [(&str, u32); 6] = [
        ("baseline: product shader, 16-bit texel, 3x3 base, buffer blocks (record 128 B)", 0),
        ("flat-record generator only (16-bit texel, 3x3)", SPIKE_V2_SHADER),
        ("wide texel, one layout for all tiles (climate in the record)", SPIKE_WIDE),
        ("both decoders, per-slot flag (tiles stay 16-bit)", SPIKE_BOTH),
        ("5x5 base surface, 16-bit texel", SPIKE_BASE5),
        ("5x5 base + wide texel (the full v2 layout)", SPIKE_BASE5 | SPIKE_WIDE),
    ];
    let finest = planet_frame::production_finest_level(CELLS);
    let size = (2560u32, 1440u32);
    // Reference field, ground-1m (index 6) and aerial-200km (index 2).
    for index in [6usize, 2] {
        let (name, _) = plan_at(index, size, TerrainView::Relief, 6.0);
        let mut rs = Vec::new();
        for (label, flags) in cands {
            set(flags, 0);
            planet_render::set_buffer_atlas(true);
            let (_, plan) = plan_at(index, size, TerrainView::Relief, 6.0);
            let tiles = tiles_of(&plan, &Reference(SEED), finest);
            let r = renderer(&ctx, size, &tiles);
            planet_render::set_buffer_atlas(false);
            rs.push((format!("{label} [{} B/slot record]", record_stride()), r, plan));
            reset();
        }
        writeln!(out, "## Reference field, {name}\n\n| layout | median ms | p10 | p90 | ratio |\n|---|---|---|---|---|").unwrap();
        out += &time_table(&ctx, rs, frames);
        out.push('\n');
    }
    // Steep field: tiles forced compact (16-bit rows are not CON-13-correct there: timing only).
    let steep = Steep(1);
    let cells = 16u32;
    let sfinest = 12u8;
    let l = LodParams { cells, viewport_h_px: f64::from(size.1), tau_px: 4.0, max_level: sfinest, ..LodParams::earth_1080p() };
    let (sname, camera) = scripted_views_field(R, f64::from(size.0) / f64::from(size.1), &steep).into_iter().nth(5).unwrap();
    let req = TerrainRequest { camera, size, view: TerrainView::Relief, lod: l, face_mapping: "tangent-v1".into(), seed: 1 };
    let plan = plan_terrain_field(&req, &steep).unwrap();
    let mut rs = Vec::new();
    for (label, flags) in cands {
        set(flags, 0);
        planet_render::set_buffer_atlas(true);
        let tiles = steep_tiles(&plan, cells, sfinest, false);
        let mut r = TerrainRenderer::with_compact_pools(&ctx, size, cells, compact_format_pool_with(1 << 30, cells, 4096, 1).unwrap());
        r.warm_up(&ctx);
        for t in &tiles {
            r.upload_tile(&ctx, t).unwrap();
        }
        planet_render::set_buffer_atlas(false);
        rs.push((format!("{label} [{} B/slot record]", record_stride()), r, plan.clone()));
        reset();
    }
    writeln!(out, "## Steep field, {sname} ({} nodes of {cells} cells, finest level {sfinest}; 16-bit rows quantise cliff tiles coarsely: timing only)\n\n| layout | median ms | p10 | p90 | ratio |\n|---|---|---|---|---|", plan.nodes.len()).unwrap();
    out += &time_table(&ctx, rs, frames);
    writeln!(out, "\nLabel: indicative, taken from {started:?} for about {:?}; Rasierklinge, adapter above; GPU clock state not controlled; no perf window.", started.elapsed().unwrap_or_default()).unwrap();
    println!("{out}");
    std::fs::write(out_dir().join(format!("t8-gpu-time-layouts-{tag}.md")), out).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// T9 (CPU): slots at the two tier shares for each record layout (ADR 0033 point 3 arithmetic, the product's fit_share_with).

#[test]
#[ignore]
fn t9_share_arithmetic() {
    let _g = LOCK.lock().unwrap();
    let mut out = String::new();
    writeln!(out, "# T9: compact slots at the tier shares by layout (planet_frame::pool::tier_compact_pool, cells {CELLS}; texels 8,712 B per slot)").unwrap();
    writeln!(out, "| layout | record B/slot | slot B (texels + record) | mid share: blocks | mid slots | mid records MB | high share: blocks | high slots | high records MB |").unwrap();
    writeln!(out, "|---|---|---|---|---|---|---|---|---|").unwrap();
    for (label, flags) in [
        ("today", 0u32),
        ("5x5 base", SPIKE_BASE5),
        ("wide texel (climate in record)", SPIKE_WIDE),
        ("5x5 + wide", SPIKE_BASE5 | SPIKE_WIDE),
    ] {
        set(flags, 0);
        let mid = planet_frame::pool::tier_compact_pool(planet_streaming::Tier::Mid, CELLS, 1).unwrap();
        let high = planet_frame::pool::tier_compact_pool(planet_streaming::Tier::High, CELLS, 1).unwrap();
        writeln!(
            out,
            "| {label} | {} | {} | {} | {} | {:.1} | {} | {} | {:.1} |",
            record_stride(),
            planet_generators::compact::slot_bytes(CELLS),
            mid.main_blocks,
            mid.main_slots(),
            mid.slot_record_total_bytes() as f64 / 1e6,
            high.main_blocks,
            high.main_slots(),
            high.slot_record_total_bytes() as f64 / 1e6
        )
        .unwrap();
        reset();
    }
    println!("{out}");
    std::fs::write(out_dir().join("t9-share-arithmetic.md"), out).unwrap();
}
