//! The M1 walking skeleton: generator -> FramePlan -> GPU executor -> pixels, driven from all three run modes.

use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_tiles, DebugView, FramePlan, PlanRequest, ReproBundle};
use planet_generators::{generate_tile, TileData, GENERATOR_VERSION, IMPLEMENTATION_ID};
use planet_render::{execute, AdapterPolicy, ExecStats, GpuContext, RgbaFrame, TileResource};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

const FRAME_SIZE: (u32, u32) = (256, 128);
const RADIUS_M: f64 = 6_371_000.0;

/// `FACE,LEVEL,X,Y`
pub fn parse_tile(s: &str) -> Result<TileId, String> {
    let p: Vec<&str> = s.split(',').collect();
    let [f, l, x, y] = p.as_slice() else {
        return Err(format!("--tile expects FACE,LEVEL,X,Y, got '{s}'"));
    };
    let n = |v: &str| v.trim().parse::<u32>().map_err(|e| format!("--tile '{s}': {e}"));
    TileId::new(Face(n(f)? as u8), n(l)? as u8, n(x)?, n(y)?)
        .ok_or_else(|| format!("--tile '{s}' is not a valid tile (face 0..5, level 0..28, x and y below 2^level)"))
}

fn default_tiles() -> Vec<TileId> {
    let a = TileId::new(Face(3), 3, 5, 2).expect("valid");
    vec![a, a.neighbor(&TangentWarp, Side::East)]
}

fn tiles_of(o: &Options) -> Result<Vec<TileId>, String> {
    if o.tiles.is_empty() {
        Ok(default_tiles())
    } else {
        o.tiles.iter().map(|t| parse_tile(t)).collect()
    }
}

fn request_of(o: &Options) -> Result<PlanRequest, String> {
    let view = DebugView::parse(&o.view).ok_or_else(|| format!("--view must be face, tile-id or height, got '{}'", o.view))?;
    Ok(PlanRequest {
        tiles: tiles_of(o)?,
        // A fixed camera 12,000 km from the centre; the skeleton only needs a defined origin for camera-relative values.
        camera: PlanetFixed(Vec3::new(12.0e6, 1.0e6, -3.0e6)),
        size: FRAME_SIZE,
        view,
        radius_m: RADIUS_M,
        face_mapping: TangentWarp.id().to_string(),
    })
}

fn run_plan(ctx: &GpuContext, plan: &FramePlan, tiles: &[TileData]) -> Result<(RgbaFrame, ExecStats), String> {
    let resources: Vec<TileResource> = tiles.iter().map(|t| TileResource { id: t.id, res: t.res, heights: &t.heights }).collect();
    execute(ctx, plan, &resources).map_err(|e| e.to_string())
}

fn write_frame(frame: &RgbaFrame, path: &Path, out: &mut dyn Write) -> Result<(), String> {
    frame.write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
    writeln!(out, "wrote {}", path.display()).map_err(err)
}

pub fn render_tiles(ctx: &GpuContext, o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let request = request_of(o)?;
    let plan = plan_tiles(&request).map_err(|e| e.to_string())?;
    let tiles: Vec<TileData> = request.tiles.iter().map(|&id| generate_tile(o.seed, &TangentWarp, id, o.res)).collect();
    let (frame, stats) = run_plan(ctx, &plan, &tiles)?;
    writeln!(
        out,
        "plan {:016x}: {} draws, {} draw calls, {} texture bytes",
        plan.plan_hash(),
        plan.draws.len(),
        stats.draw_calls,
        stats.texture_bytes_uploaded
    )
    .map_err(err)?;
    if let Some(path) = &o.out {
        write_frame(&frame, path, out)?;
    }
    if let Some(path) = &o.bundle_out {
        let bundle = ReproBundle {
            generator_version: GENERATOR_VERSION,
            implementation_id: IMPLEMENTATION_ID.to_string(),
            seed: o.seed,
            res: o.res,
            definition_hash: 0,
            adapter: ctx.adapter_key(),
            request,
            plan,
        };
        std::fs::write(path, bundle.to_text()).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote repro bundle {}", path.display()).map_err(err)?;
    }
    Ok(())
}

/// `editor --offscreen`: the same skeleton as the viewer will run, without a window (the window arrives in M2).
pub fn render_offscreen(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let ctx = GpuContext::new(o.adapter.unwrap_or(AdapterPolicy::Software)).map_err(|e| e.to_string())?;
    writeln!(out, "adapter: {:?} {} ({:?}), key {}", ctx.info.backend, ctx.info.name, ctx.info.device_type, ctx.adapter_key())
        .map_err(err)?;
    if let Some(bundle) = &o.repro {
        return replay(&ctx, bundle, o.out.as_deref(), out);
    }
    render_tiles(&ctx, o, out)
}

/// Replay a repro bundle: refuse on version mismatch, rebuild the plan from the recorded request, compare, render.
pub fn replay(ctx: &GpuContext, path: &Path, png: Option<&Path>, out: &mut dyn Write) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let b = ReproBundle::from_text(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if b.generator_version != GENERATOR_VERSION || b.implementation_id != IMPLEMENTATION_ID {
        return Err(format!(
            "bundle was recorded with generator {} ({}), this build has {} ({}); check out the matching commit to replay it",
            b.generator_version, b.implementation_id, GENERATOR_VERSION, IMPLEMENTATION_ID
        ));
    }
    let rebuilt = plan_tiles(&b.request).map_err(|e| e.to_string())?;
    let diff = rebuilt.diff(&b.plan);
    if diff.is_empty() && rebuilt.plan_hash() != b.plan.plan_hash() {
        return Err(format!(
            "replayed plan hash {:016x} differs from the recorded {:016x} although no field differs: the plan text form changed",
            rebuilt.plan_hash(),
            b.plan.plan_hash()
        ));
    }
    if !diff.is_empty() {
        return Err(format!("replayed plan differs from the recorded plan:\n  {}", diff.join("\n  ")));
    }
    writeln!(out, "replay: plan identical ({:016x}), recorded on adapter {}", rebuilt.plan_hash(), b.adapter).map_err(err)?;
    let map = planet_frame::plan::face_mapping_by_id(&b.request.face_mapping)
        .ok_or_else(|| format!("unknown face mapping '{}'", b.request.face_mapping))?;
    let tiles: Vec<TileData> = b.request.tiles.iter().map(|&id| generate_tile(b.seed, map.as_ref(), id, b.res)).collect();
    let (frame, stats) = run_plan(ctx, &rebuilt, &tiles)?;
    writeln!(out, "replay: {} draw calls", stats.draw_calls).map_err(err)?;
    if let Some(p) = png {
        write_frame(&frame, p, out)?;
    }
    Ok(())
}

/// `generate`: headless CPU generation of one tile with its hash, height map and optionally the face net.
pub fn generate(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    if o.repro.is_some() || o.bundle_out.is_some() {
        return Err("generate has no frames: --repro and --bundle belong to test-render and editor --offscreen".into());
    }
    let id = tiles_of(o)?[0];
    let tile = generate_tile(o.seed, &TangentWarp, id, o.res);
    writeln!(
        out,
        "tile {:#018x} face {} level {} ({}, {}): content hash {:016x}, cache key {:016x}",
        id.raw(),
        id.face().0,
        id.level(),
        id.x(),
        id.y(),
        tile.content_hash(),
        planet_generators::cache_key(&TangentWarp, o.seed, id, o.res)
    )
    .map_err(err)?;
    if let Some(path) = &o.out {
        planet_generators::dump::tile_height_map(&tile).write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote {}", path.display()).map_err(err)?;
    }
    if let Some(path) = &o.face_net {
        planet_generators::dump::face_net(&TangentWarp, 128, 2).write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote {}", path.display()).map_err(err)?;
    }
    Ok(())
}

const TERRAIN_SIZE: (u32, u32) = (256, 192);
const TERRAIN_CELLS: u32 = 16;

/// `--scene terrain`: node selection -> TerrainPlan -> meshes -> GPU, one of the ten scripted cameras.
pub fn render_terrain(ctx: &GpuContext, o: &Options, out: &mut dyn Write) -> Result<(), String> {
    use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
    let view = TerrainView::parse(&o.view)
        .ok_or_else(|| format!("--view must be one of face, tile-id, level, morph, height, normals, depth, got '{}'", o.view))?;
    let views = scripted_views(RADIUS_M, f64::from(TERRAIN_SIZE.0) / f64::from(TERRAIN_SIZE.1), o.seed);
    let index = match o.script.parse::<usize>() {
        Ok(i) => i,
        Err(_) => views
            .iter()
            .position(|(n, _)| *n == o.script)
            .ok_or_else(|| format!("--script '{}' is neither an index nor one of the scripted names", o.script))?,
    };
    let (name, camera) = *views.get(index).ok_or_else(|| format!("--script {index} is out of range (0..{})", views.len()))?;
    let lod = LodParams {
        cells: TERRAIN_CELLS,
        viewport_h_px: f64::from(TERRAIN_SIZE.1),
        tau_px: 4.0,
        max_level: 26,
        ..LodParams::earth_1080p()
    };
    let plan =
        plan_terrain(&TerrainRequest { camera, size: TERRAIN_SIZE, view, lod, face_mapping: TangentWarp.id().to_string(), seed: o.seed })
            .map_err(|e| e.to_string())?;
    let meshes: Vec<std::sync::Arc<planet_generators::mesh::TileMesh>> = if o.stream {
        streamed_meshes(o, &plan)?
    } else {
        plan.nodes
            .iter()
            .map(|n| std::sync::Arc::new(planet_generators::mesh::tile_mesh(o.seed, &TangentWarp, n.tile, TERRAIN_CELLS, RADIUS_M)))
            .collect()
    };
    let refs: Vec<&planet_generators::mesh::TileMesh> = meshes.iter().map(|m| m.as_ref()).collect();
    let (frame, stats) = planet_render::execute_terrain(ctx, &plan, &refs).map_err(|e| e.to_string())?;
    writeln!(out, "terrain {name} ({}): {}; {} draw calls, {} triangles", view.name(), plan.summary(), stats.draw_calls, stats.triangles)
        .map_err(err)?;
    if let Some(path) = &o.out {
        write_frame(&frame, path, out)?;
    }
    Ok(())
}

struct MeshSource {
    seed: u64,
}

impl planet_streaming::TileSource<planet_generators::mesh::TileMesh> for MeshSource {
    fn load(&self, tile: TileId) -> Result<planet_generators::mesh::TileMesh, String> {
        Ok(planet_generators::mesh::tile_mesh(self.seed, &TangentWarp, tile, TERRAIN_CELLS, RADIUS_M))
    }
}

/// Generate the plan's tile meshes through the asynchronous streaming path (thread pool, parent-first requests) and
/// wait, frame by frame, until every wanted tile is resident. The result equals the synchronous path.
fn streamed_meshes(
    o: &Options,
    plan: &planet_frame::TerrainPlan,
) -> Result<Vec<std::sync::Arc<planet_generators::mesh::TileMesh>>, String> {
    use planet_streaming::{StreamConfig, Streamer, SystemClock, ThreadPool};
    let pool = std::sync::Arc::new(ThreadPool::new(4));
    let config = StreamConfig { max_in_flight: 8, capacity: plan.nodes.len() * 2 + 64, retry_after_ms: 1000, record_dispatches: false };
    let mut streamer =
        Streamer::new(std::sync::Arc::new(MeshSource { seed: o.seed }), pool, std::sync::Arc::new(SystemClock::new()), config);
    streamer.load_roots_blocking()?;
    let wanted: Vec<(TileId, f64)> = plan.nodes.iter().enumerate().map(|(i, n)| (n.tile, i as f64)).collect();
    let started = Instant::now();
    loop {
        let resolved = streamer.begin_frame(&wanted);
        if resolved.iter().all(|r| r.exact()) {
            return Ok(wanted.iter().map(|(t, _)| streamer.get(*t).expect("resident")).collect());
        }
        if started.elapsed().as_secs() > 120 {
            return Err(format!("streaming did not finish within 120 s ({:?})", streamer.stats()));
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// `test-render --perf --scene terrain`: the terrain frame workload at 1440p on the reference GPU (CON-21). Meshes are
/// generated and uploaded once per view; frames then only update two small buffers and draw. Reports GPU time (timestamp
/// queries), CPU record/submit time, wall time, CPU planning time and the deterministic counts, per run, and checks the
/// terrain budget (median GPU <= 8 ms, p99 frame <= 20 ms).
pub fn perf_terrain(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
    use planet_perf::{summarize, Protocol, RunReport, Stats};
    use planet_render::TerrainRenderer;
    // `--smoke` runs the same code path briefly on any adapter to prove it works; it is labelled and never a measurement.
    let smoke = o.perf_smoke;
    if !smoke {
        if !o.machine_ready {
            return Err(
                "--machine-ready is required: the owner must confirm charger, performance power profile and discrete GPU mode".into()
            );
        }
        planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
    }
    let size = if smoke { (1280u32, 720u32) } else { (2560u32, 1440u32) };
    let lod = LodParams { cells: o.cells, viewport_h_px: f64::from(size.1), tau_px: o.tau, max_level: 26, ..LodParams::earth_1080p() };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, o.tau))?;
    let (warmup, measured, runs) = if smoke { (2usize, 5usize, 1usize) } else { (60usize, o.frames.max(10), 3usize) };
    let protocol =
        Protocol { warmup_frames: warmup, measured_frames: measured, runs, internal_resolution: size, smoke, ..Protocol::default() };
    let views = scripted_views(RADIUS_M, f64::from(size.0) / f64::from(size.1), o.seed);
    let mut renderer = TerrainRenderer::new(ctx, size, o.cells);
    let mut reports: Vec<RunReport> = Vec::new();
    let mut budget_lines = Vec::new();
    if smoke {
        writeln!(out, "SMOKE RUN: exercises the perf path only; the numbers are not a measurement").map_err(err)?;
    }
    writeln!(out, "terrain perf: {}x{}, cells {}, tau {} px, timestamps {}", size.0, size.1, o.cells, o.tau, ctx.timestamps_supported())
        .map_err(err)?;
    // Heaviest first: the light orbit view never raises idle clocks by itself, so it runs after the GPU is busy.
    for index in [6usize, 5, 3, 0] {
        let (name, camera) = views[index];
        let req = TerrainRequest {
            camera,
            size,
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: TangentWarp.id().to_string(),
            seed: o.seed,
        };
        // CPU planning time (selection + plan), measured separately: the camera is static during frame measurements.
        let mut plan_samples = Vec::new();
        let mut plan = plan_terrain(&req).map_err(|e| e.to_string())?;
        for _ in 0..20 {
            let t = Instant::now();
            plan = plan_terrain(&req).map_err(|e| e.to_string())?;
            plan_samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        // Generate the missing meshes in parallel, then upload.
        let missing: Vec<TileId> = plan.nodes.iter().map(|n| n.tile).filter(|t| !renderer.has_tile(*t)).collect();
        let started = Instant::now();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let chunk = missing.len().div_ceil(threads).max(1);
        let (seed, cells) = (o.seed, o.cells);
        let meshes: Vec<planet_generators::mesh::TileMesh> = std::thread::scope(|s| {
            let handles: Vec<_> = missing
                .chunks(chunk)
                .map(|part| {
                    s.spawn(move || {
                        part.iter().map(|&t| planet_generators::mesh::tile_mesh(seed, &TangentWarp, t, cells, RADIUS_M)).collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().expect("mesh thread")).collect()
        });
        for m in &meshes {
            renderer.upload(ctx, m).map_err(|e| e.to_string())?;
        }
        writeln!(
            out,
            "{name}: {} nodes ({} new meshes in {:.1} s), plan {:.2} ms",
            plan.nodes.len(),
            meshes.len(),
            started.elapsed().as_secs_f64(),
            summarize(&plan_samples, 0).map_or(f64::NAN, |s| s.median)
        )
        .map_err(err)?;
        for _ in 0..warmup {
            renderer.render(ctx, &plan, true).map_err(|e| e.to_string())?;
        }
        for _ in 0..runs {
            let before = planet_perf::query_nvidia_smi();
            let (mut total, mut cpu, mut gpu) = (Vec::new(), Vec::new(), Vec::new());
            let mut counts = planet_render::TerrainStats::default();
            for _ in 0..measured {
                let (t, s) = renderer.render(ctx, &plan, true).map_err(|e| e.to_string())?;
                total.push(t.total_ms);
                cpu.push(t.cpu_ms);
                gpu.extend(t.gpu_ms);
                counts = s;
            }
            let stat = |v: &[f64]| summarize(v, 0).ok_or_else(|| "no valid samples".to_string());
            let mut metrics: Vec<(String, Stats)> = vec![("cpu_ms".into(), stat(&cpu)?), ("plan_ms".into(), stat(&plan_samples)?)];
            if !gpu.is_empty() {
                metrics.push(("gpu_ms".into(), stat(&gpu)?));
            }
            let (t_stats, g_stats) = (stat(&total)?, metrics.iter().find(|(n, _)| n == "gpu_ms").map(|(_, s)| s.clone()));
            budget_lines.push((name, t_stats.clone(), g_stats));
            reports.push(RunReport {
                path_name: format!("terrain/{name}"),
                vram_cap_gb: 8,
                stats: t_stats,
                clocks_before: before,
                clocks_after: planet_perf::query_nvidia_smi(),
                metrics,
                counts: vec![
                    ("nodes".into(), plan.nodes.len() as u64),
                    ("draw_calls".into(), u64::from(counts.draw_calls)),
                    ("triangles".into(), counts.triangles),
                    ("vertex_bytes_resident".into(), renderer.uploaded_bytes),
                    ("resident_tiles".into(), renderer.resident_tiles() as u64),
                ],
            });
        }
    }
    let scheme = planet_perf::query_power_scheme();
    for w in planet_perf::session_warnings(&crate::facts(ctx), scheme.as_deref(), &reports, smoke) {
        writeln!(out, "WARNING (not valid for time budgets): {w}").map_err(err)?;
    }
    for (name, total, gpu) in &budget_lines {
        let v = planet_perf::budget_verdict(gpu.as_ref().map(|g| g.median), total.p99);
        let word = |ok: bool| if ok { "ok" } else { "OVER" };
        let verdict = match gpu {
            Some(g) => format!(
                "GPU median {:.2} ms (budget {}) {}; frame p99 {:.2} ms (budget {}) {}",
                g.median,
                planet_perf::TERRAIN_GPU_BUDGET_MS,
                word(v.gpu_ok == Some(true)),
                total.p99,
                planet_perf::FRAME_P99_BUDGET_MS,
                word(v.frame_ok)
            ),
            None => format!("no GPU timestamps; frame median {:.2} ms, p99 {:.2} ms (no GPU verdict)", total.median, total.p99),
        };
        writeln!(out, "  {name}: {verdict}").map_err(err)?;
    }
    // Run-to-run noise within each view (its three runs), reported as the worst view.
    let mut worst_noise = 0.0f64;
    let names: std::collections::BTreeSet<&str> = budget_lines.iter().map(|(n, _, _)| *n).collect();
    for name in names {
        let medians: Vec<f64> = reports.iter().filter(|r| r.path_name == format!("terrain/{name}")).map(|r| r.stats.median).collect();
        worst_noise = worst_noise.max(planet_perf::run_to_run_noise(&medians).unwrap_or(0.0));
    }
    writeln!(out, "worst run-to-run noise within a view (max-min)/median: {worst_noise:.3}").map_err(err)?;
    let json = planet_perf::report_json(&crate::facts(ctx), scheme.as_deref(), o.machine_ready && !smoke, &protocol, &reports);
    if smoke && o.out.is_none() {
        return writeln!(out, "smoke run: no log written (pass --out to keep it)").map_err(err);
    }
    let path = o.out.clone().unwrap_or_else(|| std::path::Path::new("perf").join("logs").join("terrain.json"));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)
}
