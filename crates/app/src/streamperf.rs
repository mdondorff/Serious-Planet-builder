//! The streaming perf scenes of ADR 0032 point 8 (TEST-019): `test-render --perf --scene dive|dive-fast|teleport-1m|teleport-2km|
//! look-1m|look-2km` (or `--scene streaming` for all six in one session), `cargo xtask perf stream`. Each drives the pipelined
//! managed loop on the real renderer (`pipelined::Rig`) along the camera script of `planet_frame::scenes` at 60 frames per
//! second of script time. The script is a function of the frame index only; the real clock paces the loop (so the streamer's
//! workers get a frame's worth of real time per frame) and measures it, and never enters a camera (CON-04).

use crate::pipelined::{RendererSink, Rig};
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::TileId;
use planet_frame::scenes::{build_script, lag_post_pass, FrameDraw, FrameLag, StreamScene, FRAME_STEP_S, LOOK_SEED, SETTLE_S};
use planet_frame::{pipeline::DrawKind, LodParams, TerrainRequest, TerrainView};
use planet_perf::streaming::{case_spec, case_verdicts, column_metrics, FrameRecord, MemoryFacts};
use planet_perf::{summarize, Protocol, RunReport};
use planet_render::GpuContext;
use planet_streaming::budget::Tier;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The cells per face edge of the production bake (GEN-014): the default of `--store`.
pub const PRODUCTION_BAKE_RES: u32 = 16384;
/// A settle that has not happened within this long is an error, not a hang.
const SETTLE_LIMIT_S: f64 = 900.0;
/// Seconds between the clock samples taken during a case.
const CLOCK_SAMPLE_PERIOD_S: u64 = 3;
/// Frames of the measured part of a smoke run.
const SMOKE_FRAMES: usize = 24;

/// Whether `--scene NAME` names a streaming scene (one of the six, or `streaming` for all of them).
pub fn is_stream_scene(name: &str) -> bool {
    name == "streaming" || StreamScene::parse(name).is_some()
}

/// The scenes a `--scene` value runs.
pub fn scenes_of(name: &str) -> Vec<StreamScene> {
    if name == "streaming" {
        StreamScene::ALL.to_vec()
    } else {
        StreamScene::parse(name).into_iter().collect()
    }
}

/// The source of the bake for the log (`protocol.source`) and whether it is the production one: the stored bake is, an
/// in-memory one is labelled and never valid for streaming budgets (a cold plan and a tile's generation differ over the store:
/// block decodes and the cache working set, SPIKE-05).
pub fn source_label(world: &WorldTerrain, from_store: bool) -> String {
    if from_store {
        world.source().to_string()
    } else {
        format!("{} (NOT the production source)", world.source())
    }
}

/// Reject option combinations the streaming scenes would ignore, and refuse the dive/teleport/look scenes without a store.
pub fn check_options(o: &Options) -> Result<(), String> {
    let stream = (o.perf || o.perf_smoke) && is_stream_scene(&o.scene);
    if !stream && (o.store.is_some() || o.allow_memory_bake) {
        return Err("--store and --allow-memory-bake only apply to the streaming perf scenes (dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km, streaming)".into());
    }
    if o.store.is_some() && o.allow_memory_bake {
        return Err("--store and --allow-memory-bake exclude each other: the store is the production source".into());
    }
    if stream && o.store.is_none() && !o.allow_memory_bake && !o.perf_smoke {
        return Err(format!(
            "the streaming scene '{}' measures cold plans and tile generation, which differ over the production store (block decodes, SPIKE-05): \
             give --store DIR, a bake of {PRODUCTION_BAKE_RES} cells per face edge made by `generate --global-dir DIR --global-res {PRODUCTION_BAKE_RES}` \
             (or `cargo xtask accept M3`'s target/accept/global16384), or --allow-memory-bake to run over the in-memory bake of --global-res cells, \
             which is labelled `NOT the production source` and not valid for streaming budgets",
            o.scene
        ));
    }
    Ok(())
}

/// What one frame of the loop produced, besides its [`FrameRecord`].
struct Tick {
    record: FrameRecord,
    settled: bool,
    /// The tiles of the plan this frame drew (resident and drawn): what the lag post-pass needs besides the camera.
    drawn: Vec<TileId>,
}

/// The rig and the pacing of one case.
struct Driver<'a> {
    ctx: &'a GpuContext,
    rig: Rig,
    next_deadline: Instant,
}

impl Driver<'_> {
    /// One iteration of the loop for `req`: never waits for a plan; waits for the GPU only at the frames-in-flight bound.
    /// The time of the iteration is `wall_ms` (the pacing sleep and the lag bookkeeping, which follow it, are not in it).
    fn tick(&mut self, req: &TerrainRequest) -> Result<Tick, String> {
        let started = Instant::now();
        let ctx = self.ctx;
        let frame = self.rig.managed.begin_frame(req, &mut RendererSink { ctx, renderer: &mut self.rig.renderer })?;
        self.rig.fi.retire_to(self.rig.renderer.retired_frames(ctx));
        while self.rig.fi.is_full() {
            self.rig.fi.note_wait();
            self.rig.fi.retire_to(self.rig.renderer.wait_oldest(ctx));
        }
        let (timing, _) = self.rig.renderer.submit(ctx, &frame.pipeline.draw.plan).map_err(|e| e.to_string())?;
        self.rig.fi.submit();
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        // Bookkeeping, outside the frame's time: the drawn tile ids (the lag is judged after the case, against the fresh plan of
        // this frame's camera: TEST-020), the blank check and the counts.
        let draw = &frame.pipeline.draw;
        let drawn: Vec<TileId> = draw.plan.nodes.iter().map(|n| n.tile).collect();
        let mut blank_nodes = draw.blank;
        if draw.plan.nodes.is_empty() {
            // Nothing drawn at all is a blank frame (the roots always exist, STRM-002).
            blank_nodes = blank_nodes.max(1);
        }
        let c = frame.counts;
        let settled = draw.kind == DrawKind::Exact
            && c.in_flight == 0
            && c.queued == 0
            && c.deferred == 0
            && !self.rig.managed.pipeline().worker().busy();
        let record = FrameRecord {
            plan_ms: frame.pipeline.delivered.map_or(0.0, |d| d.1),
            scheduler_ms: frame.scheduler_ms,
            upload_ms: frame.upload_ms,
            record_ms: timing.cpu_ms,
            gpu_ms: None,
            wall_ms,
            lag: None,
            uncovered_nodes: 0,
            blank_nodes,
            upload_bytes: c.upload_bytes,
            resident: c.resident,
            in_flight: c.in_flight,
            queued: c.queued,
            dispatched: c.dispatched,
            uploaded: c.uploaded,
            cancelled: c.cancelled,
            degraded: c.degraded,
            allocated_bytes: self.rig.renderer.pool_allocated_bytes(),
        };
        self.pace();
        Ok(Tick { record, settled, drawn })
    }

    /// Hold the loop to the frame step of script time: the next iteration starts one step after this one was due (a late frame
    /// moves the schedule, it is not made up for). Sleeps most of the wait and spins the last two milliseconds.
    fn pace(&mut self) {
        self.next_deadline += Duration::from_secs_f64(FRAME_STEP_S);
        let now = Instant::now();
        if self.next_deadline <= now {
            self.next_deadline = now;
            return;
        }
        let wait = self.next_deadline - now;
        if wait > Duration::from_millis(3) {
            std::thread::sleep(wait - Duration::from_millis(2));
        }
        while Instant::now() < self.next_deadline {
            std::hint::spin_loop();
        }
    }
}

/// Put the post-pass lags into the frame records: every frame gets exactly one (a length mismatch is a harness bug, not a pass).
fn apply_lags(frames: &mut [FrameRecord], lags: &[FrameLag]) -> Result<(), String> {
    if frames.len() != lags.len() {
        return Err(format!("detail lag: {} frames recorded but {} lags computed", frames.len(), lags.len()));
    }
    for (f, l) in frames.iter_mut().zip(lags) {
        f.lag = Some(l.mean);
        f.uncovered_nodes = l.uncovered.len();
    }
    Ok(())
}

/// What a case produced.
struct CaseRun {
    frames: Vec<FrameRecord>,
    /// Frames (index in the measured part) that drew a blank node (nothing resident to show, not even a root: the draw knows a count,
    /// not ids, so the id list is empty).
    blanks: Vec<(usize, Vec<u64>)>,
    /// Seconds of the settling before frame 0 (`None`: the case starts from empty).
    settle_s: Option<f64>,
    clocks: Vec<planet_perf::ClockSample>,
    /// Seconds the case's teardown took (the loop dropped: queued jobs cancelled, in-flight ones waited for; STRM-005).
    teardown_s: f64,
}

fn run_case(
    ctx: &GpuContext,
    world: &Arc<WorldTerrain>,
    script: &planet_frame::scenes::SceneScript,
    req: &(dyn Fn(planet_frame::Camera) -> TerrainRequest + Sync),
    smoke: bool,
    out: &mut dyn Write,
) -> Result<CaseRun, String> {
    let first = req(script.prelude.unwrap_or(script.cameras[0]));
    let rig_started = Instant::now();
    let mut d = Driver { ctx, rig: Rig::new(ctx, world, &first)?, next_deadline: Instant::now() };
    writeln!(out, "{}: rig built in {:.2} s", script.scene.name(), rig_started.elapsed().as_secs_f64()).map_err(err)?;
    // Settle at the prelude camera (unmeasured): R fully resident, nothing queued, in flight or waiting, for one second of script
    // time. A smoke run only runs a few frames.
    let mut settle_s = None;
    if let Some(pre) = &script.prelude {
        let r = req(*pre);
        let started = Instant::now();
        let mut quiet = 0usize;
        let need = (SETTLE_S / FRAME_STEP_S).round() as usize;
        let mut frames = 0usize;
        loop {
            let t = d.tick(&r)?;
            frames += 1;
            quiet = if t.settled { quiet + 1 } else { 0 };
            if smoke && frames >= 12 || quiet >= need {
                break;
            }
            if started.elapsed().as_secs_f64() > SETTLE_LIMIT_S {
                return Err(format!(
                    "{}: the prelude view did not settle within {SETTLE_LIMIT_S} s ({:?})",
                    script.scene.name(),
                    d.rig.managed.manager().streamer().stats()
                ));
            }
        }
        settle_s = Some(started.elapsed().as_secs_f64());
        writeln!(out, "{}: settled at the start view after {:.1} s ({frames} frames)", script.scene.name(), settle_s.unwrap_or(0.0))
            .map_err(err)?;
    }
    // The clock, before, during (a sampler thread: a process spawn never on the frame path) and after the case.
    let mut clocks: Vec<planet_perf::ClockSample> = Vec::new();
    let sampler = (!smoke).then(|| {
        clocks.extend(planet_perf::query_nvidia_smi());
        let stop = Arc::new(AtomicBool::new(false));
        let samples: Arc<Mutex<Vec<planet_perf::ClockSample>>> = Arc::default();
        let (s2, v2) = (Arc::clone(&stop), Arc::clone(&samples));
        let handle = std::thread::spawn(move || {
            let mut last = Instant::now();
            while !s2.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(200));
                if last.elapsed().as_secs() >= CLOCK_SAMPLE_PERIOD_S {
                    last = Instant::now();
                    if let Some(c) = planet_perf::query_nvidia_smi() {
                        v2.lock().expect("clock samples").push(c);
                    }
                }
            }
        });
        (stop, samples, handle)
    });
    let n = if smoke { script.cameras.len().min(SMOKE_FRAMES) } else { script.cameras.len() };
    d.next_deadline = Instant::now();
    let (mut frames, mut blanks) = (Vec::with_capacity(n), Vec::new());
    // What the post-pass needs of each frame, kept as small as the plan actually drawn: the f64 pose and the drawn tile ids.
    let mut draws: Vec<FrameDraw> = Vec::with_capacity(n);
    for (k, cam) in script.cameras[..n].iter().enumerate() {
        let t = d.tick(&req(*cam))?;
        if t.record.blank_nodes > 0 {
            blanks.push((k, Vec::new()));
        }
        frames.push(t.record);
        draws.push(FrameDraw { camera: Some(*cam), drawn: t.drawn });
    }
    if let Some((stop, samples, handle)) = sampler {
        stop.store(true, Ordering::Relaxed);
        let _ = handle.join();
        clocks.extend(samples.lock().expect("clock samples").iter().cloned());
        clocks.extend(planet_perf::query_nvidia_smi());
    }
    let torn = Instant::now();
    drop(d);
    let teardown_s = torn.elapsed().as_secs_f64();
    // The lag post-pass, off the timed path: the loop is gone (queued jobs cancelled, in-flight ones finished), the case is over,
    // and every frame's lag is judged against the plan selected afresh for its own camera (TEST-020).
    let post = Instant::now();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let lags = lag_post_pass(&draws, req, &|| world.field(), threads)?;
    apply_lags(&mut frames, &lags)?;
    writeln!(
        out,
        "{}: detail lag post-pass over {} frames in {:.1} s ({threads} threads)",
        script.scene.name(),
        frames.len(),
        post.elapsed().as_secs_f64()
    )
    .map_err(err)?;
    Ok(CaseRun { frames, blanks, settle_s, clocks, teardown_s })
}

// spec: TEST-019, TEST-018, TEST-012
/// `test-render --perf --scene dive|dive-fast|teleport-1m|teleport-2km|look-1m|look-2km|streaming`: ADR 0032's cases on the
/// reference GPU after the readiness gate (ADR 0020), over the stored production bake (`--store DIR`; `--allow-memory-bake` runs
/// over `--global-res` cells in memory, labelled and not valid for streaming budgets). Per case it prints the verdicts of
/// ADR 0032 point 4 (frame time at any clock state, upload, memory, detail lag: gated for the dive descent and mouse look,
/// tracked for the teleport), writes `<scene>.json` (compared with the last valid log of the same scene and source), the per-frame
/// columns in `<scene>.frames.txt`, and fails the run, after writing both, if any frame drew a blank tile.
pub fn perf_streaming(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    use planet_perf::compare;
    let smoke = o.perf_smoke;
    let scenes = scenes_of(&o.scene);
    if scenes.is_empty() {
        return Err(format!("unknown streaming scene '{}'", o.scene));
    }
    check_options(o)?;
    // The world and its source, before the gate so the bake's load is not measured as the machine's.
    let from_store = o.store.is_some();
    let open = || -> Result<WorldTerrain, String> {
        match &o.store {
            Some(dir) => {
                let res = if o.given.contains("--global-res") { o.global_res } else { PRODUCTION_BAKE_RES };
                let def = match &o.world {
                    Some(p) => crate::worldfield::load_world(p)?,
                    None => planet_world_def::WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
                };
                WorldTerrain::open_store(def, dir, res)
            }
            None => WorldTerrain::of_perf_options(o, crate::skeleton::RADIUS_M),
        }
    };
    let opened = Instant::now();
    let world = Arc::new(open()?);
    let field = world.field()?;
    writeln!(out, "world opened in {:.1} s", opened.elapsed().as_secs_f64()).map_err(err)?;
    let radius_m = world.radius_m();
    let measured_field = world.describe();
    let source = source_label(&world, from_store);
    let production = from_store;
    let mut owner_override: Vec<String> = Vec::new();
    if !smoke {
        planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
        owner_override = crate::readiness_gate(o, ctx, out)?;
    }
    let size = if smoke { (1280u32, 720u32) } else { (2560u32, 1440u32) };
    // The same viewport-to-tolerance ratio as the real run (240 at the defaults): the plan's steepness check depends on it.
    let tau = if smoke { o.tau * f64::from(size.1) / 1440.0 } else { o.tau };
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(size.1),
        tau_px: tau,
        max_level: o.finest_level_for(o.cells),
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, tau))?;
    let aspect = f64::from(size.0) / f64::from(size.1);
    let seed = field.seed();
    let req_of = |camera: planet_frame::Camera| TerrainRequest {
        camera,
        size,
        view: TerrainView::Level,
        lod: lod.clone(),
        face_mapping: TangentWarp.id().to_string(),
        seed,
    };
    if smoke {
        writeln!(out, "SMOKE RUN: exercises the streaming path only (a few frames, 1280x720, no gate); the numbers are not a measurement")
            .map_err(err)?;
    }
    writeln!(
        out,
        "streaming perf: {}x{}, cells {}, tau {} px, finest level {}, timestamps {}, {measured_field}; source: {source}",
        size.0,
        size.1,
        o.cells,
        tau,
        lod.max_level,
        ctx.timestamps_supported()
    )
    .map_err(err)?;
    writeln!(
        out,
        "  frame time = the loop's work per frame (plan not waited for; waits for the GPU only at the frames-in-flight bound; the pacing \
         sleep excluded); GPU time is not measured in the pipelined loop (no timestamp readback without a wait), RAM is the process's \
         peak working set, driver VRAM is the whole GPU"
    )
    .map_err(err)?;
    let mem_base = MemoryFacts {
        share_bytes: Tier::Mid.terrain_share_bytes(),
        vram_cap_bytes: Tier::Mid.vram_cap_bytes(),
        driver_vram_bytes: None,
        process_peak_bytes: None,
    };
    let mut blank_failures: Vec<String> = Vec::new();
    let mut reports: Vec<(StreamScene, RunReport, Vec<String>, CaseRunSummary)> = Vec::new();
    for (index, scene) in scenes.into_iter().enumerate() {
        // Each scene reads the store through a fresh reader: the decoded-block cache of an earlier scene would make this one's
        // cold plans and tile generation warmer than a run of this scene alone (SPIKE-05; the OS file cache stays warm).
        let scene_world = if from_store && index > 0 { Arc::new(open()?) } else { Arc::clone(&world) };
        let built = Instant::now();
        let scene_field = scene_world.field()?;
        let script = build_script(scene, radius_m, aspect, scene_field.as_ref(), LOOK_SEED);
        drop(scene_field);
        let shown = if smoke { script.cameras.len().min(SMOKE_FRAMES) } else { script.cameras.len() };
        writeln!(
            out,
            "{}: {shown} of {} frames of {:.4} s{}",
            scene.name(),
            script.cameras.len(),
            FRAME_STEP_S,
            if script.prelude.is_some() { ", settled start" } else { ", from an empty cache" }
        )
        .map_err(err)?;
        writeln!(out, "{}: script built in {:.1} s", scene.name(), built.elapsed().as_secs_f64()).map_err(err)?;
        let case_started = Instant::now();
        let run = run_case(ctx, &scene_world, &script, &req_of, smoke, out)?;
        writeln!(out, "{}: case ran in {:.1} s (teardown {:.2} s)", scene.name(), case_started.elapsed().as_secs_f64(), run.teardown_s)
            .map_err(err)?;
        let mem = MemoryFacts {
            driver_vram_bytes: if smoke { None } else { planet_perf::streaming::query_gpu_memory_used() },
            process_peak_bytes: planet_perf::streaming::query_process_peak_bytes(),
            ..mem_base.clone()
        };
        let spec = case_spec(scene.name(), run.frames.len(), FRAME_STEP_S, script.descent_frames).expect("a streaming scene");
        let verdicts = case_verdicts(&spec, &run.frames, &run.blanks, &mem);
        let lines: Vec<String> = verdicts.iter().map(|v| v.line(scene.name())).collect();
        for (k, ids) in &run.blanks {
            blank_failures.push(format!(
                "{}: frame {k} drew a blank node; tiles {}",
                scene.name(),
                if ids.is_empty() {
                    "nothing resident to show (not even a root)".to_string()
                } else {
                    ids.iter().map(|t| format!("{t:#018x}")).collect::<Vec<_>>().join(" ")
                }
            ));
        }
        let wall: Vec<f64> = run.frames.iter().map(|f| f.wall_ms).collect();
        let stats = summarize(&wall, 0).ok_or("no frames")?;
        // The clock: the first sample before the case, and the lowest-clocked sample of the case in the `after` slot, so a view is
        // GPU-valid only if every sample read the floor (ADR 0036; the log counts say how many samples there were).
        let lowest = run.clocks.iter().min_by(|a, b| a.graphics_mhz.total_cmp(&b.graphics_mhz)).cloned();
        let mut counts: Vec<(String, u64)> = vec![
            ("frames".into(), run.frames.len() as u64),
            ("blank_frames".into(), run.blanks.len() as u64),
            ("upload_bytes_max".into(), run.frames.iter().map(|f| f.upload_bytes).max().unwrap_or(0)),
            ("allocated_bytes_max".into(), run.frames.iter().map(|f| f.allocated_bytes).max().unwrap_or(0)),
            ("clock_samples".into(), run.clocks.len() as u64),
        ];
        if let Some(l) = &lowest {
            counts.push(("clock_min_mhz".into(), l.graphics_mhz as u64));
        }
        if let Some(b) = mem.process_peak_bytes {
            counts.push(("process_peak_bytes".into(), b));
        }
        if let Some(b) = mem.driver_vram_bytes {
            counts.push(("driver_vram_bytes".into(), b));
        }
        let report = RunReport {
            path_name: format!("stream/{}", scene.name()),
            vram_cap_gb: 8,
            stats,
            clocks_before: run.clocks.first().cloned(),
            clocks_after: lowest,
            metrics: column_metrics(&run.frames),
            counts,
        };
        let summary = CaseRunSummary { frames: run.frames, settle_s: run.settle_s, clocks: run.clocks.len() };
        reports.push((scene, report, lines, summary));
    }
    // The session: valid only on the production source, and per ADR 0020 and 0036 for throttling.
    let scheme = planet_perf::query_power_scheme();
    let run_reports: Vec<RunReport> = reports.iter().map(|(_, r, _, _)| r.clone()).collect();
    let throttled: Vec<String> = run_reports.iter().filter(|r| crate::run_throttled(r)).map(|r| r.path_name.clone()).collect();
    let mut validity = if smoke { planet_perf::gate::Validity::default() } else { planet_perf::gate::validity(&throttled, &[]) };
    if !production {
        validity.reasons.push(format!(
            "the bake is {source}: cold plans and tile generation differ over the production store, so the streaming budgets do not count"
        ));
    }
    if !smoke {
        crate::session_verdict_line(out, &validity, &owner_override)?;
    }
    for w in planet_perf::session_warnings(&crate::facts(ctx), scheme.as_deref(), &run_reports, smoke) {
        writeln!(out, "WARNING (not valid for time budgets): {w}").map_err(err)?;
    }
    let protocol = Protocol {
        warmup_frames: 0,
        measured_frames: 0,
        runs: 1,
        internal_resolution: size,
        field: Some(measured_field.clone()),
        source: Some(source.clone()),
        smoke,
        ..Protocol::default()
    };
    let single = reports.len() == 1;
    for (scene, report, lines, summary) in &reports {
        let clock_state = planet_perf::view_gpu_validity(std::slice::from_ref(report), &report.path_name);
        let state = match &clock_state {
            planet_perf::clock::GpuValidity::Valid(s) => s.to_string(),
            planet_perf::clock::GpuValidity::Invalid(r) => r.clone(),
        };
        let cap = if planet_perf::view_power_cap(std::slice::from_ref(report), &report.path_name) && !smoke {
            " [power cap 0x4 recorded, not invalidating]"
        } else {
            ""
        };
        let tag = if smoke { String::new() } else { format!(" [clock state {state}; {} samples]{cap}", summary.clocks) };
        if let Some(s) = summary.settle_s {
            writeln!(out, "{}: settled start after {s:.1} s", scene.name()).map_err(err)?;
        }
        writeln!(out, "{}: columns (median / p99 / max ms): {}{tag}", scene.name(), columns_line(&summary.frames)).map_err(err)?;
        writeln!(out, "  {}: GPU time: not measured (GPU verdicts: none; frame verdicts stand at any clock state)", scene.name())
            .map_err(err)?;
        for l in lines {
            writeln!(out, "{l}").map_err(err)?;
        }
        // The log, the per-frame columns and the comparison with the last valid log of this scene and source.
        let one = std::slice::from_ref(report);
        let json =
            planet_perf::report_json_with_warmups(&crate::facts(ctx), scheme.as_deref(), &owner_override, &validity, &protocol, one, &[]);
        let path = match (&o.out, single) {
            (Some(p), true) => p.clone(),
            (Some(dir), false) => dir.join(format!("{}.json", scene.name())),
            (None, _) => std::path::Path::new("perf").join("logs").join(format!("{}.json", scene.name())),
        };
        if smoke && o.out.is_none() {
            writeln!(out, "smoke run: no log written (pass --out to keep it)").map_err(err)?;
            continue;
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let baseline = compare::baseline_path(&path);
        let last_valid = compare::choose_baseline(std::fs::read(&baseline).ok(), std::fs::read(&path).ok());
        for line in compare::comparison_lines(last_valid.as_deref(), json.as_bytes()) {
            writeln!(out, "{line}").map_err(err)?;
        }
        std::fs::write(&path, &json).map_err(err)?;
        std::fs::write(path.with_extension("frames.txt"), planet_perf::streaming::frames_text(&summary.frames)).map_err(err)?;
        if compare::replaces_baseline(json.as_bytes()) {
            std::fs::write(&baseline, &json).map_err(err)?;
        }
        writeln!(out, "wrote {} (and {})", path.display(), path.with_extension("frames.txt").display()).map_err(err)?;
    }
    if !blank_failures.is_empty() {
        for f in &blank_failures {
            writeln!(out, "BLANK TILE: {f}").map_err(err)?;
        }
        return Err(format!("{} frame(s) drew a blank tile (STRM-002): {}", blank_failures.len(), blank_failures[0]));
    }
    let over: Vec<String> = reports
        .iter()
        .flat_map(|(s, _, lines, _)| lines.iter().filter(|l| l.ends_with(" OVER")).map(move |l| format!("{}: {}", s.name(), l.trim())))
        .collect();
    writeln!(
        out,
        "{} gated measure(s) over budget{}",
        over.len(),
        if smoke || !validity.is_valid() { " (the session does not count)" } else { "" }
    )
    .map_err(err)?;
    Ok(())
}

struct CaseRunSummary {
    frames: Vec<FrameRecord>,
    settle_s: Option<f64>,
    clocks: usize,
}

/// `plan scheduler upload record wall` as median / p99 / max, for the console.
fn columns_line(frames: &[FrameRecord]) -> String {
    type Column = (&'static str, fn(&FrameRecord) -> f64);
    let cols: [Column; 5] = [
        ("plan", |f| f.plan_ms),
        ("scheduler", |f| f.scheduler_ms),
        ("upload", |f| f.upload_ms),
        ("record", |f| f.record_ms),
        ("wall", |f| f.wall_ms),
    ];
    cols.iter()
        .map(|(n, get)| {
            let v: Vec<f64> = frames.iter().map(get).collect();
            summarize(&v, 0).map_or(format!("{n} -"), |s| format!("{n} {:.2}/{:.2}/{:.2}", s.median, s.p99, s.max))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(args: &[&str]) -> Options {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        crate::parse_args(&args).unwrap()
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("planet-streamperf-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    // spec: TEST-019
    #[test]
    fn the_registry_of_scenes_is_the_same_in_the_scripts_and_the_verdicts_and_names_the_six() {
        let scripts: Vec<&str> = StreamScene::ALL.iter().map(|s| s.name()).collect();
        assert_eq!(scripts, planet_perf::streaming::SCENES);
        assert_eq!(scripts, ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km"]);
        for s in &scripts {
            assert!(is_stream_scene(s) && scenes_of(s).len() == 1, "{s}");
        }
        assert_eq!(scenes_of("streaming").len(), 6);
        assert!(!is_stream_scene("terrain") && !is_stream_scene("global-bake") && scenes_of("terrain").is_empty());
        // The verdicts' upload cap is the loop's default budget (ADR 0033 point 9).
        assert_eq!(planet_perf::streaming::UPLOAD_BYTES_BUDGET, planet_frame::upload::DEFAULT_TILE_BYTES_PER_FRAME);
        // One definition: the case of a scene has as many frames as its script.
        for scene in StreamScene::ALL {
            let script = build_script(scene, 6_371_000.0, 16.0 / 9.0, &planet_generators::height::Reference(1), LOOK_SEED);
            let spec = case_spec(scene.name(), script.cameras.len(), FRAME_STEP_S, script.descent_frames).unwrap();
            assert_eq!(spec.frames, script.cameras.len());
        }
    }

    // spec: TEST-020
    #[test]
    fn the_post_pass_lags_fill_every_frame_and_a_count_mismatch_is_an_error_not_a_pass() {
        let frame = || FrameRecord {
            plan_ms: 0.0,
            scheduler_ms: 0.0,
            upload_ms: 0.0,
            record_ms: 0.0,
            gpu_ms: None,
            wall_ms: 1.0,
            lag: None,
            uncovered_nodes: 0,
            blank_nodes: 0,
            upload_bytes: 0,
            resident: 0,
            in_flight: 0,
            queued: 0,
            dispatched: 0,
            uploaded: 0,
            cancelled: 0,
            degraded: 0,
            allocated_bytes: 0,
        };
        let mut frames = vec![frame(), frame()];
        let lags = vec![
            FrameLag { mean: 0.5, worst: 2, uncovered: vec![] },
            FrameLag { mean: 0.0, worst: 0, uncovered: vec![TileId::new(planet_core::Face(0), 1, 0, 0).unwrap()] },
        ];
        apply_lags(&mut frames, &lags).unwrap();
        assert_eq!((frames[0].lag, frames[1].lag, frames[0].uncovered_nodes, frames[1].uncovered_nodes), (Some(0.5), Some(0.0), 0, 1));
        let e = apply_lags(&mut frames, &lags[..1]).unwrap_err();
        assert!(e.contains("2 frames recorded but 1 lags"), "{e}");
    }

    // spec: TEST-019
    #[test]
    fn the_scenes_refuse_to_run_without_a_store_unless_the_memory_bake_is_allowed_and_labelled() {
        let e = check_options(&opts(&["test-render", "--perf", "--scene", "dive"])).unwrap_err();
        assert!(
            e.contains("--store DIR")
                && e.contains("--allow-memory-bake")
                && e.contains("16384")
                && e.contains("NOT the production source"),
            "{e}"
        );
        assert!(check_options(&opts(&["test-render", "--perf", "--scene", "teleport-1m", "--store", "x"])).is_ok());
        assert!(check_options(&opts(&["test-render", "--perf", "--scene", "look-2km", "--allow-memory-bake"])).is_ok());
        assert!(
            check_options(&opts(&["test-render", "--perf-smoke", "--scene", "dive"])).is_ok(),
            "a smoke run proves the path on a tiny bake"
        );
        let e = check_options(&opts(&["test-render", "--perf", "--scene", "dive", "--store", "x", "--allow-memory-bake"])).unwrap_err();
        assert!(e.contains("exclude each other"), "{e}");
        let e = check_options(&opts(&["test-render", "--perf", "--scene", "terrain", "--store", "x"])).unwrap_err();
        assert!(e.contains("only apply to the streaming perf scenes"), "{e}");
        // The terrain scene keeps its default (in-memory 256-cell bake) and needs no store.
        assert!(check_options(&opts(&["test-render", "--perf", "--scene", "terrain"])).is_ok());
    }

    // spec: TEST-019
    #[test]
    fn the_source_is_logged_with_the_store_path_and_cells_and_an_in_memory_bake_is_labelled() {
        let dir = tmp("store");
        let mut sink = Vec::new();
        crate::run(
            &["generate", "--global-dir", dir.to_str().unwrap(), "--global-res", "8", "--seed", "3", "--threads", "2"].map(String::from),
            &mut sink,
        )
        .unwrap();
        let def = || planet_world_def::WorldDefinition::single_planet(crate::skeleton::RADIUS_M, 3);
        let stored = WorldTerrain::open_store(def(), &dir, 8).unwrap();
        let label = source_label(&stored, true);
        assert!(label.starts_with("store ") && label.contains("8 cells") && label.contains(dir.to_str().unwrap()), "{label}");
        assert!(!label.contains("NOT the production source"));
        assert!(stored.describe().contains("cpu-erosion-v1 over a 8-cell bake"), "{}", stored.describe());
        // The stored field reads the bake: a height is finite where a missing bake would give NaN.
        let h = stored.field().unwrap().height(planet_core::Vec3::new(0.3, 0.5, 0.8).normalized());
        assert!(h.is_finite(), "{h}");
        // A store without the bake of this definition or resolution is refused now, with the remedy.
        let e = WorldTerrain::open_store(def(), &dir, 16).err().unwrap();
        assert!(e.contains("no bake of 16 cells") || e.contains("16 cells"), "{e}");
        let e = WorldTerrain::open_store(planet_world_def::WorldDefinition::single_planet(crate::skeleton::RADIUS_M, 4), &dir, 8)
            .err()
            .unwrap();
        assert!(e.contains("holds no bake"), "{e}");
        let e = WorldTerrain::open_store(def(), &dir.join("nope"), 8).err().unwrap();
        assert!(e.contains("no such store directory"), "{e}");
        // An in-memory bake is labelled and the label is what the log records in `protocol.source`.
        let mem = WorldTerrain::open(def(), 8).unwrap();
        let label = source_label(&mem, false);
        assert_eq!(label, "in-memory 8 cells (NOT the production source)");
        let protocol = Protocol { source: Some(label.clone()), ..Protocol::default() };
        let a = planet_perf::AdapterFacts { name: "x".into(), backend: "y".into(), kind: planet_perf::DeviceKind::Cpu };
        let json = planet_perf::report_json(&a, None, &[], &planet_perf::gate::Validity::default(), &protocol, &[]);
        assert!(json.contains(&format!("\"source\":\"{label}\"")), "{json}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
