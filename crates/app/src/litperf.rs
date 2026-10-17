//! The M5 lit perf scenes (TEST-025, M5 item 8): the lit path (atmosphere, water, exposure, tone map) at 1440p with a GPU timestamp for
//! every pass, over the views of the viewer (`planet_frame::litscenes`: ground 1 m, aerial 20 km, the coast at 2 km and 3 m, orbit) and
//! the two coastal flights, with `water.shore_filter` 1 (the look's default) and 0 for inspection. `lit-formats` runs four of the views
//! in the vertex and the compact tile format back to back and logs the per-pass GPU ratio.
//!
//! The scenes follow `skeleton::perf_terrain`: the readiness gate, the warm-up to the clock floor (ADR 0034, ADR 0036), runs of waited
//! timed frames, the noise rule with one retry, the session validity of `planet_perf::validity`, the baseline comparison. What is new
//! is the lit setup shared with the streaming scenes' `--lit` variant (`LitSetup`: the look, the sea, the water plan per frame, the
//! warm-up of the lit path before the first measured frame, REND-017) and the per-pass rows (`planet_perf::lit`).

use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_frame::litscenes::{lit_views, LitView, ViewKind};
use planet_frame::look::file::LookParams;
use planet_frame::viewer::{lit_warm_plan, viewer_lit_plan};
use planet_frame::water::{empty_water_plan, plan_water, WaterRequest, WaterTileCache, WaterView};
use planet_frame::{Camera, LodParams, SelectionCache, TerrainPlan, TerrainRequest, TerrainView};
use planet_perf::lit::{metrics_of, LitFrame};
use planet_perf::{summarize, Protocol, RunReport};
use planet_render::{GpuContext, TerrainRenderer, TileFormat};
use std::io::Write;
use std::time::Instant;

/// Whether `--scene NAME` names a lit perf scene.
pub fn is_lit_scene(name: &str) -> bool {
    name == "lit" || name == "lit-formats"
}

/// The views `lit-formats` measures in both formats: the ones whose passes differ most by tile format (the terrain pass reads the
/// tiles; the others do not).
const FORMAT_VIEWS: [&str; 4] = ["ground-1m", "aerial-20km", "coast-2km", "orbit"];
/// Waves move with the clock: a flight's frame k is drawn at `WATER_T0 + k / 60` s.
const WATER_T0: f64 = 100.0;

/// What the lit frames of a run share: the look, the sea of the world, the liquids and the water plan cache.
pub(crate) struct LitSetup {
    pub look: LookParams,
    liquids: planet_world_def::LiquidTable,
    pub level_m: f64,
    lod: LodParams,
    radius_m: f64,
    cache: WaterTileCache,
    pub has_water: bool,
}

impl LitSetup {
    pub(crate) fn new(o: &Options, world: &WorldTerrain, lod: &LodParams) -> Result<LitSetup, String> {
        let look = crate::lookload::load(o)?.params(planet_world_def::BodyId::PLANET);
        if let Err(why) = viewer_lit_plan(&look, world.radius_m(), 0.0, true, true) {
            return Err(format!("the lit scenes measure the atmosphere, which does not fit this body: {why}"));
        }
        let level_m =
            world.definition().sea(planet_world_def::BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
        Ok(LitSetup {
            look,
            liquids: world.definition().liquids().map_err(|e| format!("the liquid table of the definition: {e}"))?,
            level_m,
            lod: lod.clone(),
            radius_m: world.radius_m(),
            cache: WaterTileCache::new(),
            has_water: world.has_water(),
        })
    }

    /// The plan of the throwaway frames that warm the lit path, the atmosphere and (in a world with water) the water pass before the
    /// first measured frame (REND-017).
    pub(crate) fn warm_plan(&self, camera: Camera, size: (u32, u32), cells: u32) -> Result<TerrainPlan, String> {
        let lit = viewer_lit_plan(&self.look, self.radius_m, 0.0, true, true)?.with_water_level(self.level_m);
        let mut warm = lit_warm_plan(camera, size, cells, lit);
        if self.has_water {
            warm.water = Some(empty_water_plan(&self.look.water));
        }
        Ok(warm)
    }

    /// `base` as a lit frame: the look's lit plan with the atmosphere (adaptive exposure on `dt_s`; `reset` snaps it) and the water
    /// plan of the nodes that carry water at `time_s`.
    pub(crate) fn plan(
        &mut self,
        world: &WorldTerrain,
        base: &TerrainPlan,
        time_s: f64,
        dt_s: f32,
        reset: bool,
    ) -> Result<TerrainPlan, String> {
        let lit = viewer_lit_plan(&self.look, self.radius_m, dt_s, reset, true)?.with_water_level(self.level_m);
        let mut plan = base.clone();
        plan.lit = Some(lit);
        let (look, liquids, lod) = (&self.look.water, &self.liquids, &self.lod);
        let cache = &mut self.cache;
        let water = world.with_water(|field| {
            plan_water(&WaterRequest { source: field, map: &TangentWarp, liquids, look, lod, time_s, view: WaterView::Lit }, cache, &plan)
        })?;
        if let Some(w) = water {
            plan.water = Some(w.map_err(|e| e.to_string())?);
        }
        Ok(plan)
    }
}

/// What one measured view produced.
struct ViewResult {
    path: String,
    reports: Vec<RunReport>,
    /// Every frame of the final batch, for the verdict rows.
    frames: Vec<LitFrame>,
    water_draws: u64,
    warmup: Option<planet_perf::clock::WarmUpOutcome>,
    load: planet_perf::load::CaseLoad,
    noisy: Option<f64>,
    noise_lines: Vec<String>,
    kind: ViewKind,
}

/// The settings every view of a run shares.
struct Rig<'a> {
    ctx: &'a GpuContext,
    world: &'a WorldTerrain,
    field: &'a dyn planet_generators::height::HeightField,
    lod: &'a LodParams,
    size: (u32, u32),
    smoke: bool,
    frames: usize,
    cells: u32,
    max_level: u8,
    radius_m: f64,
}

fn frame_of(t: &planet_render::FrameTiming) -> LitFrame {
    let mut pass_ms = [None; 8];
    for p in planet_render::GpuPass::ALL {
        pass_ms[p as usize] = t.passes.get(p);
    }
    LitFrame { total_ms: t.total_ms, cpu_ms: t.cpu_ms, pass_ms, scene_copy_ms: t.passes.scene_copy_ms, span_ms: t.passes.span_ms }
}

fn request(rig: &Rig<'_>, camera: Camera) -> TerrainRequest {
    TerrainRequest {
        camera,
        size: rig.size,
        view: TerrainView::Level,
        lod: rig.lod.clone(),
        face_mapping: TangentWarp.id().to_string(),
        seed: rig.field.seed(),
    }
}

/// A renderer for the run, with the pools of the tier and the lit path warmed before the first measured frame.
fn new_renderer(rig: &Rig<'_>, setup: &LitSetup, camera: Camera) -> Result<TerrainRenderer, String> {
    let mut r = TerrainRenderer::for_context(rig.ctx, rig.size, rig.cells, 1);
    r.warm_up(rig.ctx);
    r.warm_up_lit(rig.ctx, &setup.warm_plan(camera, rig.size, rig.cells)?).map_err(|e| format!("warming the lit path: {e}"))?;
    Ok(r)
}

fn count_water(plan: &TerrainPlan) -> u64 {
    plan.water.as_ref().map_or(0, |w| w.nodes.len() as u64)
}

/// Bring the GPU to its loaded state with chunks of frames of `plan` (ADR 0034, ADR 0036); not in a smoke run.
fn warm_clock(rig: &Rig<'_>, r: &mut TerrainRenderer, plan: &TerrainPlan) -> Result<Option<planet_perf::clock::WarmUpOutcome>, String> {
    if rig.smoke {
        return Ok(None);
    }
    let started = Instant::now();
    planet_perf::clock::warm_up(
        planet_perf::clock::WARMUP_CAP_S,
        || {
            for _ in 0..planet_perf::clock::WARMUP_CHUNK_FRAMES {
                r.render(rig.ctx, plan, true).map_err(|e| e.to_string())?;
            }
            Ok(())
        },
        || planet_perf::query_nvidia_smi().as_ref().map(planet_perf::clock::ClockPoint::from),
        || started.elapsed().as_secs_f64(),
    )
    .map(Some)
}

fn counts_of(
    r: &TerrainRenderer,
    plan: &TerrainPlan,
    stats: &planet_render::TerrainStats,
    water_draws: u64,
    format: TileFormat,
) -> Vec<(String, u64)> {
    vec![
        ("nodes".into(), plan.nodes.len() as u64),
        ("draw_calls".into(), u64::from(stats.draw_calls)),
        ("triangles".into(), stats.triangles),
        ("water_nodes".into(), water_draws),
        ("water_draws".into(), u64::from(stats.water_draws)),
        ("resident_tiles".into(), r.resident_tiles() as u64),
        ("tile_format_compact".into(), u64::from(format == TileFormat::Compact)),
        ("lit".into(), 1),
        ("atmosphere".into(), 1),
    ]
}

/// A held camera: planned and uploaded once, waited timed frames in runs, the noise rule with one retry (ADR 0020).
fn measure_static(
    rig: &Rig<'_>,
    r: &mut TerrainRenderer,
    setup: &mut LitSetup,
    view: &LitView,
    path: &str,
    format: TileFormat,
    out: &mut dyn Write,
) -> Result<ViewResult, String> {
    let camera = view.cameras[0];
    let req = request(rig, camera);
    let mut cache = SelectionCache::new();
    let plan = planet_frame::plan_terrain_cached(&req, rig.field, &mut cache).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let new = crate::skeleton::upload_missing_meshes(rig.ctx, r, &plan, rig.field, rig.radius_m, rig.cells, rig.max_level)?;
    // Seed the exposure at the view (reset), then hold it: later frames adapt by one 60 Hz step each.
    let seed = setup.plan(rig.world, &plan, WATER_T0, 0.0, true)?;
    r.render(rig.ctx, &seed, true).map_err(|e| e.to_string())?;
    let lit = setup.plan(rig.world, &plan, WATER_T0, 1.0 / 60.0, false)?;
    let water_draws = count_water(&lit);
    writeln!(
        out,
        "{path}: {} nodes ({new} new meshes in {:.1} s), {water_draws} with water",
        plan.nodes.len(),
        started.elapsed().as_secs_f64()
    )
    .map_err(err)?;
    let (warm, measured, runs) = if rig.smoke { (1usize, 3usize, 1usize) } else { (60usize, rig.frames.max(10), 3usize) };
    for _ in 0..warm {
        r.render(rig.ctx, &lit, true).map_err(|e| e.to_string())?;
    }
    let warmup = warm_clock(rig, r, &lit)?;
    let load_sampler = (!rig.smoke).then(|| planet_perf::load::LoadSampler::start(planet_perf::load::SAMPLE_PERIOD_S));
    let mut attempt = 0;
    let (mut reports, mut batch_frames, mut noisy, mut noise_lines);
    loop {
        reports = Vec::new();
        batch_frames = Vec::<LitFrame>::new();
        noisy = None;
        noise_lines = Vec::new();
        let mut stats_last = planet_render::TerrainStats::default();
        let mut totals_by_run: Vec<f64> = Vec::new();
        let mut atmo_by_run: Vec<f64> = Vec::new();
        let mut water_by_run: Vec<f64> = Vec::new();
        for _ in 0..runs {
            let before = planet_perf::query_nvidia_smi();
            let mut frames = Vec::with_capacity(measured);
            for _ in 0..measured {
                let (t, s) = r.render(rig.ctx, &lit, true).map_err(|e| e.to_string())?;
                frames.push(frame_of(&t));
                stats_last = s;
            }
            let totals: Vec<f64> = frames.iter().map(|f| f.total_ms).collect();
            let stats = summarize(&totals, 0).ok_or("no valid samples")?;
            let metrics = metrics_of(&frames);
            totals_by_run.push(stats.median);
            let med = |n: &str| metrics.iter().find(|(m, _)| m == n).map(|(_, s)| s.median);
            atmo_by_run.extend(med("gpu_atmosphere_ms"));
            water_by_run.extend(med("gpu_water_total_ms"));
            reports.push(RunReport {
                path_name: path.to_string(),
                vram_cap_gb: 8,
                stats,
                clocks_before: before,
                clocks_after: planet_perf::query_nvidia_smi(),
                metrics,
                counts: counts_of(r, &lit, &stats_last, water_draws, format),
            });
            batch_frames.extend(frames);
        }
        // The noise rule of the accepted ADRs (10 % of the waited frame median, one retry); the proposed ADR 0036 point 3 (the
        // absolute floor of 0.1 ms) is only reported, from the atmosphere and water medians, for the decision it waits for.
        let needs_retry = !rig.smoke && planet_perf::gate::noise_needs_retry(&totals_by_run);
        for (label, medians) in [("atmosphere", &atmo_by_run), ("water", &water_by_run)] {
            if let Some((rel, abs, now, floor)) = planet_perf::lit::noise_floor_report(medians) {
                noise_lines.push(format!(
                    "  {path}: {label} run-to-run noise {rel:.3} ({abs:.3} ms): above the 10 % rule: {}; above the proposed ADR 0036 point 3 floor too: {} (recorded, not applied)",
                    if now { "yes" } else { "no" },
                    if floor { "yes" } else { "no" }
                ));
            }
        }
        if needs_retry && attempt == 0 {
            writeln!(
                out,
                "{path}: run-to-run noise {:.3} above {}: one retry",
                planet_perf::run_to_run_noise(&totals_by_run).unwrap_or(f64::NAN),
                planet_perf::gate::MAX_NOISE
            )
            .map_err(err)?;
            attempt += 1;
            continue;
        }
        if needs_retry {
            noisy = planet_perf::run_to_run_noise(&totals_by_run);
        }
        break;
    }
    let load = load_sampler.map_or_else(planet_perf::load::CaseLoad::not_measured, planet_perf::load::LoadSampler::stop);
    Ok(ViewResult {
        path: path.to_string(),
        reports,
        frames: batch_frames,
        water_draws,
        warmup,
        load,
        noisy,
        noise_lines,
        kind: ViewKind::Static,
    })
}

/// A camera path: every camera planned, its new tiles uploaded, then one untimed and `reps` timed waited frames; one run, report only.
fn measure_flight(
    rig: &Rig<'_>,
    r: &mut TerrainRenderer,
    setup: &mut LitSetup,
    view: &LitView,
    path: &str,
    format: TileFormat,
    out: &mut dyn Write,
) -> Result<ViewResult, String> {
    let reps = if rig.smoke { 1 } else { 2 };
    let mut cache = SelectionCache::new();
    let mut frames: Vec<LitFrame> = Vec::new();
    let (mut water_max, mut nodes_max) = (0u64, 0usize);
    let mut warmup = None;
    let mut before = None;
    let mut load_sampler = None;
    let started = Instant::now();
    let mut last_stats = planet_render::TerrainStats::default();
    for (k, camera) in view.cameras.iter().enumerate() {
        let plan = planet_frame::plan_terrain_cached(&request(rig, *camera), rig.field, &mut cache).map_err(|e| e.to_string())?;
        crate::skeleton::upload_missing_meshes(rig.ctx, r, &plan, rig.field, rig.radius_m, rig.cells, rig.max_level)?;
        let t = WATER_T0 + k as f64 / 60.0;
        let lit = setup.plan(rig.world, &plan, t, 1.0 / 60.0, k == 0)?;
        water_max = water_max.max(count_water(&lit));
        nodes_max = nodes_max.max(plan.nodes.len());
        if k == 0 {
            warmup = warm_clock(rig, r, &lit)?;
            before = planet_perf::query_nvidia_smi();
            load_sampler = (!rig.smoke).then(|| planet_perf::load::LoadSampler::start(planet_perf::load::SAMPLE_PERIOD_S));
        }
        // The first draw of a new set of tiles pays for their first use; it is not a measured frame.
        r.render(rig.ctx, &lit, false).map_err(|e| e.to_string())?;
        for _ in 0..reps {
            let (timing, s) = r.render(rig.ctx, &lit, true).map_err(|e| e.to_string())?;
            frames.push(frame_of(&timing));
            last_stats = s;
        }
    }
    let load = load_sampler.map_or_else(planet_perf::load::CaseLoad::not_measured, planet_perf::load::LoadSampler::stop);
    writeln!(
        out,
        "{path}: {} cameras x {reps} timed frames in {:.1} s, up to {nodes_max} nodes, {water_max} with water",
        view.cameras.len(),
        started.elapsed().as_secs_f64()
    )
    .map_err(err)?;
    let totals: Vec<f64> = frames.iter().map(|f| f.total_ms).collect();
    let stats = summarize(&totals, 0).ok_or("no valid samples")?;
    let mut counts = vec![("cameras".to_string(), view.cameras.len() as u64), ("timed_frames".to_string(), frames.len() as u64)];
    counts.extend(counts_of(
        r,
        &TerrainPlan { nodes: Vec::new(), ..lit_warm_plan(view.cameras[0], rig.size, rig.cells, planet_frame::look::LitPlan::fixed(0.0)) },
        &last_stats,
        water_max,
        format,
    ));
    counts.retain(|(n, _)| n != "nodes");
    counts.push(("nodes_max".into(), nodes_max as u64));
    let report = RunReport {
        path_name: path.to_string(),
        vram_cap_gb: 8,
        stats,
        clocks_before: before,
        clocks_after: planet_perf::query_nvidia_smi(),
        metrics: metrics_of(&frames),
        counts,
    };
    Ok(ViewResult {
        path: path.to_string(),
        reports: vec![report],
        frames,
        water_draws: water_max,
        warmup,
        load,
        noisy: None,
        noise_lines: Vec::new(),
        kind: ViewKind::Flight,
    })
}

/// The rows of a view on the console.
fn print_view(res: &ViewResult, gpu: &planet_perf::clock::GpuValidity, smoke: bool, out: &mut dyn Write) -> Result<(), String> {
    let totals: Vec<f64> = res.frames.iter().map(|f| f.total_ms).collect();
    let frame = summarize(&totals, 0).ok_or("no frames")?;
    let metrics = metrics_of(&res.frames);
    let gated = res.kind == ViewKind::Static;
    let state = match gpu {
        planet_perf::clock::GpuValidity::Valid(s) => format!("clock state {s}"),
        planet_perf::clock::GpuValidity::Invalid(r) => format!("GPU not valid: {r}"),
    };
    writeln!(
        out,
        "{}: frame (waited) median {:.3} / p99 {:.3} / max {:.3} ms{}{}",
        res.path,
        frame.median,
        frame.p99,
        frame.max,
        if smoke { String::new() } else { format!(" [{state}]") },
        if gated { "" } else { " [flight: report only]" }
    )
    .map_err(err)?;
    // A smoke run is no measurement: no budget rows, only the pass rows that prove the timestamps flow.
    if !smoke {
        for row in planet_perf::lit::view_verdicts(&metrics, &frame, gpu, gated, res.water_draws, true) {
            writeln!(out, "{}", row.line(&res.path)).map_err(err)?;
        }
    }
    writeln!(out, "  {}: {}", res.path, planet_perf::lit::pass_rows_line(&metrics)).map_err(err)?;
    // The sums need every pass of a frame: a frame that lost one drops out of the median, so the count is shown.
    let n_of = |m: &str| metrics.iter().find(|(n, _)| n == m).map_or(0, |(_, s)| s.samples);
    writeln!(
        out,
        "  {}: frames behind the sums: atmosphere {} of {}, water {} of {}",
        res.path,
        n_of("gpu_atmosphere_ms"),
        res.frames.len(),
        n_of("gpu_water_total_ms"),
        res.frames.len()
    )
    .map_err(err)?;
    if !smoke {
        writeln!(out, "  {}: {}", res.path, res.load.line()).map_err(err)?;
        for n in res.load.notes() {
            writeln!(out, "  {}: LOAD NOTE (recorded, the session's validity is unchanged): {n}", res.path).map_err(err)?;
        }
    }
    for l in &res.noise_lines {
        writeln!(out, "{l}").map_err(err)?;
    }
    Ok(())
}

// spec: TEST-025, TEST-012, TEST-023
/// `test-render --perf --scene lit | lit-formats`: the lit path's GPU time by pass over the viewer's views and the coastal flights at
/// 1440p, on the reference GPU after the readiness gate; `--perf-smoke` proves the path on any adapter and is never a measurement.
pub fn perf_lit(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    use planet_perf::compare;
    let smoke = o.perf_smoke;
    let formats_scene = o.scene == "lit-formats";
    crate::streamperf::check_options(o)?;
    let from_store = o.store.is_some();
    let world = crate::streamperf::open_world(o, false)?.bounding_water();
    if !world.has_water() {
        return Err("the lit scenes measure the water pass, but this world has no ocean channel (a store made before GLOBAL_VERSION 4?): regenerate it with `generate --global-dir`".into());
    }
    let field = world.field()?;
    let radius_m = world.radius_m();
    let measured_field = world.describe();
    let source = crate::streamperf::source_label(&world, from_store);
    let mut owner_override: Vec<String> = Vec::new();
    if smoke && from_store {
        crate::streamperf::print_store_check(&world, o, out)?;
    }
    if !smoke {
        planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
        owner_override = crate::readiness_gate(o, ctx, out)?;
    }
    let size = if smoke { (640u32, 360u32) } else { (2560u32, 1440u32) };
    // A smoke run keeps the real tolerance in pixels where the frame allows it (the plan needs viewport / tau above about 64 for 32 cells).
    let tau = if smoke { o.tau.min(f64::from(size.1) / 72.0) } else { o.tau };
    let max_level = o.finest_level_for(o.cells);
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(size.1),
        tau_px: tau,
        max_level,
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, tau))?;
    let aspect = f64::from(size.0) / f64::from(size.1);
    let mut setup = LitSetup::new(o, &world, &lod)?;
    let is_sea = |d: planet_core::Vec3| {
        world.with_water(|f| planet_frame::water::WaterSource::water_at(f, d).ok().flatten().is_some()).ok().flatten().unwrap_or(false)
    };
    let flight_frames = if smoke && !o.given.contains("--flight-frames") { 4 } else { o.flight_frames };
    let mut views = lit_views(radius_m, aspect, field.as_ref(), setup.level_m, &is_sea, flight_frames)?;
    if let Some(names) = &o.views {
        for n in names {
            if !views.iter().any(|v| v.name == n) {
                return Err(format!("--views: no view '{n}' (known: {})", views.iter().map(|v| v.name).collect::<Vec<_>>().join(", ")));
            }
        }
        views.retain(|v| names.iter().any(|n| n == v.name));
    }
    if views.is_empty() {
        return Err("--views names no view".into());
    }
    let rig =
        Rig { ctx, world: &world, field: field.as_ref(), lod: &lod, size, smoke, frames: o.frames, cells: o.cells, max_level, radius_m };
    // The software adapter's device is removed by the compact format with the atmosphere (status.md, WARP): its smoke runs the vertex
    // format; the discrete GPU runs whatever `--tile-format` asks, the compact format by default.
    let software = crate::facts(ctx).kind == planet_perf::DeviceKind::Cpu;
    if smoke {
        writeln!(
            out,
            "SMOKE RUN: exercises the lit perf path only (a few frames, {}x{}, no gate); the numbers are not a measurement",
            size.0, size.1
        )
        .map_err(err)?;
    }
    if smoke && software && o.tile_format.is_none() {
        ctx.prefer_tile_format(TileFormat::Vertex);
    }
    let primary = ctx.default_tile_format();
    writeln!(
        out,
        "lit perf: {}x{}, cells {}, tau {} px, finest level {}, timestamps {}, tile format {}, {measured_field}; lit path (atmosphere, water); source: {source}",
        size.0, size.1, o.cells, tau, max_level, ctx.timestamps_supported(), primary.name()
    )
    .map_err(err)?;
    writeln!(
        out,
        "  GPU time per pass from timestamps of waited frames of the lit path (terrain, water + scene copy, sky view, aerial perspective, composite, exposure histogram and adapt, tone map); the atmosphere row is the sum of its three passes; the LUT upload is a queue write on a body's load, not a pass of a steady frame"
    )
    .map_err(err)?;
    let mut results: Vec<ViewResult> = Vec::new();
    let mut extra_lines: Vec<String> = Vec::new();
    let first_camera = views[0].cameras[0];
    if !formats_scene {
        let mut r = new_renderer(&rig, &setup, first_camera)?;
        for view in &views {
            let path = format!("lit/{}", view.name);
            match view.kind {
                ViewKind::Static => results.push(measure_static(&rig, &mut r, &mut setup, view, &path, primary, out)?),
                ViewKind::Flight => {
                    // Both settings of the shore filter, the look's default first (ATMO-017; the inspection of the gate).
                    let default = setup.look.water.shore_filter;
                    results.push(measure_flight(&rig, &mut r, &mut setup, view, &path, primary, out)?);
                    let other = if default == 0.0 { 1.0 } else { 0.0 };
                    setup.look.water.shore_filter = other;
                    let off = measure_flight(&rig, &mut r, &mut setup, view, &format!("{path}@filter{}", other as u32), primary, out);
                    setup.look.water.shore_filter = default;
                    results.push(off?);
                }
            }
        }
    } else {
        let formats: Vec<TileFormat> =
            if smoke && software { vec![TileFormat::Vertex] } else { vec![TileFormat::Vertex, TileFormat::Compact] };
        ctx.prefer_tile_format(formats[0]);
        let mut renderers: Vec<(TileFormat, TerrainRenderer)> = Vec::new();
        for f in &formats {
            ctx.prefer_tile_format(*f);
            renderers.push((*f, new_renderer(&rig, &setup, first_camera)?));
        }
        for name in FORMAT_VIEWS {
            let view = views.iter().find(|v| v.name == name).ok_or_else(|| format!("no view {name}"))?;
            let mut per_format: Vec<ViewResult> = Vec::new();
            for (f, r) in &mut renderers {
                ctx.prefer_tile_format(*f);
                per_format.push(measure_static(&rig, r, &mut setup, view, &format!("lit-formats/{name}/{}", f.name()), *f, out)?);
            }
            if let [v, c] = per_format.as_mut_slice() {
                // ADR 0036 point 2 and ADR 0034 points 5 and 6: GPU figures are compared only between two GPU-valid views at equal clock
                // state; otherwise the line says why and no ratio is stored. A smoke run has no clock state and proves the path only.
                let why_not = if smoke {
                    None
                } else {
                    match (planet_perf::view_gpu_validity(&v.reports, &v.path), planet_perf::view_gpu_validity(&c.reports, &c.path)) {
                        (planet_perf::clock::GpuValidity::Valid(a), planet_perf::clock::GpuValidity::Valid(b)) => {
                            (!planet_perf::clock::equal_clock_state(&a, &b)).then(|| format!("not comparable: clock state {a} vs {b}"))
                        }
                        (planet_perf::clock::GpuValidity::Invalid(r), _) => {
                            Some(format!("not compared: the vertex view is NOT VALID for GPU budgets ({r})"))
                        }
                        (_, planet_perf::clock::GpuValidity::Invalid(r)) => {
                            Some(format!("not compared: the compact view is NOT VALID for GPU budgets ({r})"))
                        }
                    }
                };
                if let Some(why) = why_not {
                    extra_lines.push(format!("lit-formats/{name}: compact / vertex GPU time ratio {why}"));
                } else {
                    let median =
                        |res: &ViewResult, m: &str| metrics_of(&res.frames).into_iter().find(|(n, _)| n == m).map(|(_, s)| s.median);
                    let mut parts = Vec::new();
                    for (label, key, m) in [
                        ("terrain pass", "terrain", "gpu_ms"),
                        ("atmosphere", "atmosphere", "gpu_atmosphere_ms"),
                        ("water", "water", "gpu_water_total_ms"),
                        ("GPU span", "span", "gpu_span_ms"),
                    ] {
                        if let (Some(a), Some(b)) = (median(c, m), median(v, m)) {
                            if b <= 0.0 {
                                continue;
                            }
                            let ratio = a / b;
                            parts.push(format!("{label} {ratio:.3} ({a:.3} / {b:.3} ms)"));
                            if let Some(rep) = c.reports.last_mut() {
                                rep.counts.push((format!("ratio_{key}_permille"), (ratio * 1000.0).round() as u64));
                            }
                        }
                    }
                    extra_lines.push(format!(
                        "lit-formats/{name}: compact / vertex GPU time ratio (median): {}",
                        if parts.is_empty() { "not measured".to_string() } else { parts.join(", ") }
                    ));
                }
            }
            results.extend(per_format);
        }
    }
    // The session: validity from the machine and the runs, in one place (planet_perf::validity).
    let all_reports: Vec<RunReport> = results.iter().flat_map(|r| r.reports.iter().cloned()).collect();
    let scheme = planet_perf::query_power_scheme();
    let throttled: Vec<String> = all_reports.iter().filter(|r| crate::run_throttled(r)).map(|r| r.path_name.clone()).collect();
    let noisy: Vec<(String, f64)> = results.iter().filter_map(|r| r.noisy.map(|n| (r.path.clone(), n))).collect();
    let mut validity = if smoke { planet_perf::gate::Validity::default() } else { planet_perf::gate::validity(&throttled, &noisy) };
    if !from_store {
        validity.reasons.push(format!("the bake is {source}: the lit budgets are judged over the production store only"));
    }
    let verdict = planet_perf::validity::SessionVerdict::assess(
        &crate::facts(ctx),
        scheme.as_deref(),
        &all_reports,
        smoke,
        &validity,
        &owner_override,
    );
    if smoke {
        for w in verdict.warnings() {
            writeln!(out, "WARNING (not valid for time budgets): {w}").map_err(err)?;
        }
    } else {
        crate::session_verdict_line(out, &verdict)?;
    }
    let mut failed: Vec<String> = Vec::new();
    for res in &results {
        let gpu = planet_perf::view_gpu_validity(&all_reports, &res.path);
        if let Some(w) = &res.warmup {
            writeln!(out, "{}", w.line(&res.path)).map_err(err)?;
        }
        print_view(res, &gpu, smoke, out)?;
        if res.kind == ViewKind::Static && !formats_scene && !smoke {
            let totals: Vec<f64> = res.frames.iter().map(|f| f.total_ms).collect();
            let frame = summarize(&totals, 0).ok_or("no frames")?;
            for row in planet_perf::lit::view_verdicts(&metrics_of(&res.frames), &frame, &gpu, true, res.water_draws, true) {
                if row.fails() && verdict.frame_valid() {
                    failed.push(row.line(&res.path));
                }
            }
        }
    }
    for l in &extra_lines {
        writeln!(out, "{l}").map_err(err)?;
    }
    let warmups: Vec<(String, planet_perf::clock::WarmUpOutcome)> =
        results.iter().filter_map(|r| r.warmup.clone().map(|w| (r.path.clone(), w))).collect();
    let loads: Vec<(String, planet_perf::load::CaseLoad)> = results.iter().map(|r| (r.path.clone(), r.load.clone())).collect();
    let protocol = Protocol {
        warmup_frames: if smoke { 1 } else { 60 },
        measured_frames: if smoke { 3 } else { o.frames.max(10) },
        runs: if smoke { 1 } else { 3 },
        internal_resolution: size,
        field: Some(format!("{measured_field}; lit path (atmosphere, water), {} tiles", primary.name())),
        source: Some(source.clone()),
        smoke,
        ..Protocol::default()
    };
    let json = planet_perf::report_json_full(
        &crate::facts(ctx),
        scheme.as_deref(),
        &owner_override,
        &validity,
        &protocol,
        &all_reports,
        &warmups,
        &loads,
    );
    if smoke && o.out.is_none() {
        writeln!(out, "smoke run: no log written (pass --out to keep it)").map_err(err)?;
    } else {
        let path = o.out.clone().unwrap_or_else(|| std::path::Path::new("perf").join("logs").join(format!("{}.json", o.scene)));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let baseline = compare::baseline_path(&path);
        let last_valid = compare::choose_baseline(std::fs::read(&baseline).ok(), std::fs::read(&path).ok());
        let gpu_baseline = compare::gpu_baseline_path(&path);
        let last_gpu_valid = compare::choose_baseline(std::fs::read(&gpu_baseline).ok(), std::fs::read(&path).ok());
        for line in compare::comparison_lines_gpu(last_valid.as_deref(), last_gpu_valid.as_deref(), json.as_bytes()) {
            writeln!(out, "{line}").map_err(err)?;
        }
        std::fs::write(&path, &json).map_err(err)?;
        // A partial log (`--views`) is not a window: it never becomes a baseline.
        if o.views.is_some() {
            writeln!(out, "--views: a partial log, not kept as a baseline").map_err(err)?;
        } else {
            if compare::replaces_baseline(json.as_bytes()) {
                std::fs::write(&baseline, &json).map_err(err)?;
            }
            if compare::replaces_gpu_baseline(json.as_bytes()) {
                std::fs::write(&gpu_baseline, &json).map_err(err)?;
            }
        }
        writeln!(out, "wrote {}", path.display()).map_err(err)?;
    }
    writeln!(
        out,
        "{} gated measure(s) over budget{}",
        failed.len(),
        if smoke || !verdict.frame_valid() { " (the session does not count)" } else { "" }
    )
    .map_err(err)?;
    for f in &failed {
        writeln!(out, "{f}").map_err(err)?;
    }
    Ok(())
}

/// Whether `--scene NAME` names the flight capture (a tool, never a measurement).
pub fn is_capture_scene(name: &str) -> bool {
    name == "capture-flights"
}

/// `test-render --scene capture-flights --out-dir DIR [--store DIR | --allow-memory-bake --global-res N] [--size WxH] [--flight-frames N]`
/// (TEST-027): the two coastal flights drawn on the lit path, each with `water.shore_filter` at the look's default and at the other
/// setting, every frame written as `DIR/<flight>-filter<k>/frame_NNNN.png` (1440p by default) for the owner's look review (ADR 0037).
/// It takes no timestamp, no clock reading and no gate, prints no budget and writes no perf log: it is a separate tool beside the
/// harness and must not run during a perf window. `cargo xtask capture flights` runs it and encodes the videos when `ffmpeg` exists.
pub fn capture_flights(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.out_dir.clone().ok_or("capture-flights needs --out-dir")?;
    let from_store = o.store.is_some();
    let world = crate::streamperf::open_world(o, false)?.bounding_water();
    if !world.has_water() {
        return Err(
            "the flights need the water: this world has no ocean channel (regenerate the store with `generate --global-dir`)".into()
        );
    }
    let field = world.field()?;
    let radius_m = world.radius_m();
    let size = o.size.unwrap_or((2560, 1440));
    let max_level = o.finest_level_for(o.cells);
    // The production tolerance at 1440p; a smaller frame keeps the plan's viewport-to-tolerance ratio above its minimum.
    let tau = if size.1 >= 1440 { o.tau } else { o.tau.min(f64::from(size.1) / 72.0) };
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(size.1),
        tau_px: tau,
        max_level,
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, tau))?;
    let aspect = f64::from(size.0) / f64::from(size.1);
    let mut setup = LitSetup::new(o, &world, &lod)?;
    let is_sea = |d: planet_core::Vec3| {
        world.with_water(|f| planet_frame::water::WaterSource::water_at(f, d).ok().flatten().is_some()).ok().flatten().unwrap_or(false)
    };
    let frames = o.flight_frames;
    let views = lit_views(radius_m, aspect, field.as_ref(), setup.level_m, &is_sea, frames)?;
    let rig =
        Rig { ctx, world: &world, field: field.as_ref(), lod: &lod, size, smoke: true, frames: 0, cells: o.cells, max_level, radius_m };
    let software = crate::facts(ctx).kind == planet_perf::DeviceKind::Cpu;
    if software && o.tile_format.is_none() {
        ctx.prefer_tile_format(TileFormat::Vertex);
    }
    writeln!(
        out,
        "capture-flights: NOT a measurement (no gate, no timestamps, no log); {}x{}, {} cameras per flight, tile format {}, {}{}",
        size.0,
        size.1,
        frames,
        ctx.default_tile_format().name(),
        world.describe(),
        if from_store { "" } else { " (in-memory bake)" }
    )
    .map_err(err)?;
    std::fs::create_dir_all(&dir).map_err(err)?;
    let mut index = String::from("# directory frames width height shore_filter\n");
    let default = setup.look.water.shore_filter;
    let first = views[0].cameras[0];
    let mut r = new_renderer(&rig, &setup, first)?;
    for view in views.iter().filter(|v| v.kind == ViewKind::Flight) {
        for filter in [default, if default == 0.0 { 1.0 } else { 0.0 }] {
            setup.look.water.shore_filter = filter;
            let name = format!("{}-filter{}", view.name, filter as u32);
            let sub = dir.join(&name);
            std::fs::create_dir_all(&sub).map_err(err)?;
            let started = Instant::now();
            let mut cache = SelectionCache::new();
            for (k, camera) in view.cameras.iter().enumerate() {
                let plan = planet_frame::plan_terrain_cached(&request(&rig, *camera), rig.field, &mut cache).map_err(|e| e.to_string())?;
                crate::skeleton::upload_missing_meshes(ctx, &mut r, &plan, rig.field, radius_m, o.cells, max_level)?;
                let lit = setup.plan(&world, &plan, WATER_T0 + k as f64 / 60.0, 1.0 / 60.0, k == 0)?;
                r.render(ctx, &lit, false).map_err(|e| e.to_string())?;
                let image = r.read_color(ctx);
                image.write_png(&sub.join(format!("frame_{k:04}.png"))).map_err(|e| format!("writing frame {k} of {name}: {e}"))?;
                if k % 40 == 39 {
                    writeln!(out, "{name}: {} of {} frames", k + 1, view.cameras.len()).map_err(err)?;
                }
            }
            writeln!(out, "{name}: {} frames in {:.1} s -> {}", view.cameras.len(), started.elapsed().as_secs_f64(), sub.display())
                .map_err(err)?;
            index += &format!("{name} {} {} {} {}\n", view.cameras.len(), size.0, size.1, filter as u32);
        }
    }
    setup.look.water.shore_filter = default;
    std::fs::write(dir.join("capture.txt"), index).map_err(err)?;
    writeln!(out, "wrote {} (frames of each flight and shore filter, and capture.txt)", dir.display()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_frame::litscenes::{FLIGHTS, STATIC_VIEWS};

    // spec: TEST-025
    #[test]
    fn the_pass_names_of_the_renderer_and_of_the_log_are_the_same_list_in_the_same_order() {
        let render: Vec<&str> = planet_render::GpuPass::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(render, planet_perf::lit::PASS_NAMES);
        assert!(is_lit_scene("lit") && is_lit_scene("lit-formats") && !is_lit_scene("dive") && !is_lit_scene("terrain"));
        // The views measured in both formats exist in the view list.
        for v in FORMAT_VIEWS {
            assert!(STATIC_VIEWS.contains(&v), "{v}");
        }
        assert_eq!(FLIGHTS.len(), 2);
    }
}
