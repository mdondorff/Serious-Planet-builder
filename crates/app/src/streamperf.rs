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
/// Frames of the measured part of a smoke run (`--frames N` asks for more, for diagnosis: a smoke run is never a measurement).
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

/// What a streaming scene does before its measured span (TEST-037): the cool start (ADR 0057, TEST-033: wait for the GPU to read
/// 60 C or less) in a measured run, nothing in a smoke run (no measurement, perhaps no `nvidia-smi`). There is no warm-up: the loop
/// is paced to 60 Hz and CPU-bound, so no amount of rendering it brings the GPU to the loaded clock (look-2km at 58 C stayed at P3
/// 1,087 MHz after 30 s unpaced), and the scenes take no GPU time for the clock to matter to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartSteps {
    pub cool: bool,
}

pub fn start_steps(smoke: bool) -> StartSteps {
    StartSteps { cool: !smoke }
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

/// Whether `scene` starts with the coarse levels pre-filled in the tile pack: the gated dive and `dive-fast`, which is judged like it
/// (owner, 2026-10-16, ADR 0045 point 3). The other scenes use the pack when it holds the tiles and say what it held.
pub fn needs_prefilled_pack(scene: StreamScene) -> bool {
    matches!(scene, StreamScene::Dive | StreamScene::DiveFast)
}

/// The marker of the log's `protocol.source` for a scene that reads the pack: `present` of the `total` tiles of levels
/// `floor..=levels` (`floor`: the render floor, LOD-011).
pub fn coarse_marker(present: usize, total: usize, floor: u8, levels: u8) -> String {
    if present == total {
        planet_perf::prefill::prefilled_marker(floor, levels)
    } else {
        planet_perf::prefill::absent_marker(present, total, floor, levels)
    }
}

/// Refuse a gated dive over a pack that lacks tiles of levels `floor..=levels`, naming the count and the command that fixes it
/// (TEST-019: a cold start is the scene `dive-cold`, never the gated dive).
pub fn require_prefilled(scene: &str, present: usize, total: usize, floor: u8, levels: u8, store: &str) -> Result<(), String> {
    if present == total {
        return Ok(());
    }
    Err(format!(
        "{scene} starts with levels {floor}-{levels} pre-filled in the tile pack (owner, 2026-10-16, ADR 0045 point 3), but {store} holds {present} of the {total} tiles \
         (compact form, cells and finest level of this run): pre-fill it once with `cargo run -p planet --release -- generate --prefill-dir <store> --global-res {PRODUCTION_BAKE_RES}` \
         (idempotent; the bake step of `generate --global-dir` does it for a new bake). The cold start is the separate scene `dive-cold`, which ignores the pack's coarse tiles and is never gated"
    ))
}

/// The log file of a scene for `--out VALUE`: a path ending in `.json` is the file itself; an existing directory, or a path with no
/// extension, is a directory that gets `<file_name>` (`<scene>[-lit].json`). Any other value is refused, so the case is never run
/// to write a log that cannot be written (TEST-019).
pub fn out_log_path(out: &std::path::Path, file_name: &str) -> Result<std::path::PathBuf, String> {
    let is_json = out.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if out.is_dir() || (out.extension().is_none() && !out.is_file()) {
        return Ok(out.join(file_name));
    }
    if is_json && !out.is_dir() {
        return Ok(out.to_path_buf());
    }
    Err(format!("--out {}: give a directory (the log is written there as {file_name}) or a path ending in .json", out.display()))
}

/// Reject option combinations the streaming scenes would ignore, and refuse the dive/teleport/look scenes without a store.
pub fn check_options(o: &Options) -> Result<(), String> {
    let capture = crate::litperf::is_capture_scene(&o.scene);
    if let Some(out) = &o.out {
        if (is_stream_scene(&o.scene) || crate::litperf::is_lit_scene(&o.scene)) && o.scene != "streaming" && (o.perf || o.perf_smoke) {
            out_log_path(out, "x.json")?;
        }
    }
    let stream = (o.perf || o.perf_smoke || capture) && (is_stream_scene(&o.scene) || crate::litperf::is_lit_scene(&o.scene) || capture);
    if !stream && (o.store.is_some() || o.allow_memory_bake) {
        return Err("--store and --allow-memory-bake only apply to the streaming perf scenes (dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km, streaming)".into());
    }
    if o.store.is_some() && o.allow_memory_bake {
        return Err("--store and --allow-memory-bake exclude each other: the store is the production source".into());
    }
    // A pre-fill depth below the render floor names no tile: the scenes would check nothing, so it is refused (`generate` only says so).
    if let Some(n) = o.prefill_levels.filter(|n| *n < o.floor_for(o.cells)) {
        if stream {
            return Err(format!(
                "--prefill-levels {n} is below the render floor {} (--floor-level, LOD-011): no tile of levels up to {n} is drawn, so the scenes would check none; name a depth of at least the floor",
                o.floor_for(o.cells)
            ));
        }
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

/// The world a perf scene reads: the stored production bake (`--store`) or the in-memory bake of `--global-res` cells; `cold` ignores
/// the tile pack's pre-filled tiles (`dive-cold`).
pub(crate) fn open_world(o: &Options, cold: bool) -> Result<WorldTerrain, String> {
    match &o.store {
        Some(dir) => {
            let res = if o.given.contains("--global-res") { o.global_res } else { PRODUCTION_BAKE_RES };
            let def = match &o.world {
                Some(p) => crate::worldfield::load_world(p)?,
                None => planet_world_def::WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
            };
            WorldTerrain::open_store(def, dir, res).map(|w| if cold { w.cold() } else { w })
        }
        None => WorldTerrain::of_perf_options(o, crate::skeleton::RADIUS_M),
    }
}

/// The read-only check of a stored world that a smoke run starts with (TEST-024): prints it and fails on a problem.
pub(crate) fn print_store_check(world: &WorldTerrain, o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let levels = o.prefill_levels.unwrap_or(planet_perf::prefill::GATED_PREFILL_LEVELS);
    let check = world.store_check(o.floor_for(o.cells), levels, o.cells, o.finest_level_for(o.cells))?;
    for l in check.lines() {
        writeln!(out, "{l}").map_err(err)?;
    }
    let problems = check.problems();
    if !problems.is_empty() {
        return Err(format!("store check FAILED: {}", problems.join("; ")));
    }
    writeln!(out, "  store check ok").map_err(err)
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
    /// The lit path (`--lit`, TEST-025): the look, the sea and the water plan cache, with the world they plan against.
    lit: Option<(crate::litperf::LitSetup, Arc<WorldTerrain>)>,
    /// The terrain's height field the water plan cuts its nodes with (ATMO-020); built with the lit path, before the case's counters.
    cut_field: Option<Box<dyn planet_generators::height::HeightField + 'a>>,
    /// Frames ticked, for the clock of the waves and the exposure's first frame.
    ticks: u64,
    /// SPIKE (water-motion-gap): the per-frame gap log.
    gap: Option<crate::watergap::GapLog>,
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
        // The lit path puts the look's lit plan (adaptive exposure on the frame step, the atmosphere) and the water of the frame on the
        // plan the loop decided, inside the timed frame as in the viewer (VIEW-007, VIEW-009).
        let lit_started = Instant::now();
        let (lit_plan, water) = match self.lit.as_mut() {
            Some((setup, world)) => {
                let before = setup.water_counters();
                let plan = setup.plan(
                    world,
                    self.cut_field.as_deref().ok_or("the lit path has no height field")?,
                    &frame.pipeline.draw.plan,
                    100.0 + self.ticks as f64 * FRAME_STEP_S,
                    FRAME_STEP_S as f32,
                    self.ticks == 0,
                )?;
                let mut plan = plan;
                if let Some(l) = std::env::var("WATER_GAP_RAMP_LEVEL").ok().and_then(|v| v.parse::<u8>().ok()) {
                    setup.clamp_ramp(&mut plan, l);
                }
                (Some(plan), setup.water_counters().since(before))
            }
            None => (None, planet_frame::water::WaterCounters::default()),
        };
        let lit_plan_ms = lit_started.elapsed().as_secs_f64() * 1000.0;
        // SPIKE (water-motion-gap)
        let mut gap_count = 0usize;
        if let (Some(g), Some((setup, _)), Some(field), Some(lp)) = (self.gap.as_mut(), self.lit.as_ref(), self.cut_field.as_deref(), lit_plan.as_ref()) {
            let f = &frame.pipeline;
            gap_count = g.frame(
                self.ticks,
                &format!("{:?}", f.draw.kind),
                f.draw.substituted,
                f.draw.blank,
                f.base_seq,
                f.frame,
                &req.camera,
                lp,
                setup.diag(),
                field,
                setup.level_m,
            );
        }
        self.ticks += 1;
        let (timing, _) =
            self.rig.renderer.submit(ctx, lit_plan.as_ref().unwrap_or(&frame.pipeline.draw.plan)).map_err(|e| e.to_string())?;
        self.rig.fi.submit();
        if let Some(g) = self.gap.as_mut() {
            if (gap_count > 0 && g.shots < 12 && self.ticks % 15 == 0) || (g.shots < 80 && self.ticks % 40 == 1) {
                g.shots += 1;
                let image = self.rig.renderer.read_color(self.ctx);
                let _ = image.write_png(&g.dir.join(format!("frame_{:05}_gaps{}.png", self.ticks, gap_count)));
                if let Some(lp) = lit_plan.as_ref() {
                    let mut p2 = lp.clone();
                    if let Some(w) = p2.water.as_mut() {
                        w.view = planet_frame::water::WaterView::Class;
                    }
                    let _ = self.rig.renderer.render(self.ctx, &p2, false);
                    let _ = self.rig.renderer.read_color(self.ctx).write_png(&g.dir.join(format!("frame_{:05}_class.png", self.ticks)));
                    let mut p3 = lp.clone();
                    p3.water = None;
                    let _ = self.rig.renderer.render(self.ctx, &p3, false);
                    let _ = self.rig.renderer.read_color(self.ctx).write_png(&g.dir.join(format!("frame_{:05}_nowater.png", self.ticks)));
                }
            }
        }
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
            uncovered_tiles: Vec::new(),
            blank_nodes,
            upload_bytes: c.upload_bytes,
            upload_calls: c.upload_calls,
            resident: c.resident,
            in_flight: c.in_flight,
            queued: c.queued,
            dispatched: c.dispatched,
            uploaded: c.uploaded,
            cancelled: c.cancelled,
            degraded: c.degraded,
            allocated_bytes: self.rig.renderer.pool_allocated_bytes(),
            queries: water.queries,
            tile_misses: water.tile_misses,
            band_queries: water.band_queries,
            lit_plan_ms,
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
        f.uncovered_tiles = l.uncovered.iter().map(|t| t.raw()).collect();
    }
    Ok(())
}

/// The source counters of a stretch of a run in the perf crate's words (microseconds, blocks).
fn load_counters(d: &crate::worldfield::SourceCounters) -> planet_perf::streaming::LoadCounters {
    let us = |ns: u64| ns / 1000;
    planet_perf::streaming::LoadCounters {
        block_decodes: d.blocks.decodes,
        block_lookups: d.blocks.shared_lookups,
        block_hits: d.blocks.shared_hits,
        block_evictions: d.blocks.evictions,
        block_decode_waits: d.blocks.decode_waits,
        block_lock_wait_us: us(d.blocks.lock_wait_ns),
        block_fetch_us: us(d.blocks.fetch_ns),
        block_decode_us: us(d.blocks.decode_ns),
        block_resident: d.blocks.resident_blocks as u64,
        block_budget: d.blocks.budget_blocks as u64,
        pack_gets: d.pack.gets,
        pack_found: d.pack.found,
        pack_get_us: us(d.pack.get_ns),
        prefill_lookups: d.prefill_lookups,
        prefill_found: d.prefill_found,
        prefill_skipped: d.prefill_skipped,
        prefill_us: us(d.prefill_ns),
        job_block_wait_us: us(d.job_block_wait_ns),
        job_block_decodes: d.job_block_decodes,
        field_builds: d.field_builds,
        field_us: us(d.field_ns),
    }
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
    /// The CPU's effective clock before and after the case (`None`: smoke run, or not readable on this host; recorded, never judged).
    cpu_before: Option<planet_perf::host::CpuClock>,
    cpu_after: Option<planet_perf::host::CpuClock>,
    /// The main pool the renderer was built with: its slots (the ceiling) and the bytes of one.
    pool: planet_streaming::PoolConfig,
    /// The log counts of the case's tile loads (STRM-022; every name starts with `load_`).
    load_counts: Vec<(String, u64)>,
    /// The host's CPU load and the competing processes during the measured part (TEST-023; `not measured` in a smoke run).
    host_load: planet_perf::load::CaseLoad,
}

#[allow(clippy::too_many_arguments)]
fn run_case(
    ctx: &GpuContext,
    world: &Arc<WorldTerrain>,
    script: &planet_frame::scenes::SceneScript,
    req: &(dyn Fn(planet_frame::Camera) -> TerrainRequest + Sync),
    smoke: bool,
    calibrate: bool,
    smoke_frames: usize,
    gen: planet_streaming::GenWorkers,
    lit: Option<crate::litperf::LitSetup>,
    out: &mut dyn Write,
) -> Result<CaseRun, String> {
    let first = req(script.prelude.unwrap_or(script.cameras[0]));
    let rig_started = Instant::now();
    let mut rig = Rig::new(ctx, world, &first, gen)?;
    // `enable_lit` is not part of `warm_up`: the lit and atmosphere passes (and the water pass) are enabled and used once per ring set
    // here, before any frame, so no measured frame pays for them (REND-017).
    if let Some(setup) = &lit {
        rig.renderer
            .warm_up_lit(ctx, &setup.warm_plan(first.camera, first.size, first.lod.cells)?)
            .map_err(|e| format!("warming the lit path: {e}"))?;
    }
    let cut_field = if lit.is_some() { Some(world.field()?) } else { None };
    let mut d = Driver { ctx, rig, next_deadline: Instant::now(), lit: lit.map(|s| (s, Arc::clone(world))), cut_field, ticks: 0, gap: None };
    if crate::watergap::enabled() {
        d.gap = crate::watergap::GapLog::open();
        if let Some((setup, _)) = d.lit.as_mut() {
            setup.diag_enable();
        }
    }
    // The machine's state as a fixed job sees it, alone and with the pool's thread count at once, before and after the case (STRM-022:
    // recorded only, never judged; the per-tile times of the log read against it). Nothing of the case runs yet.
    let calib_threads = gen.workers;
    let calib_before = (!smoke || calibrate).then(|| planet_perf::streaming::calibrate_cpu(calib_threads));
    writeln!(out, "{}: rig built in {:.2} s", script.scene.name(), rig_started.elapsed().as_secs_f64()).map_err(err)?;
    // The start-up load of the roots, before frame 0 (STRM-001): a reading of the cold start (with the pack ignored, `dive-cold`, it is
    // every root generated), recorded under the `load_` prefix like the tile loads.
    let roots = d.rig.roots;
    writeln!(out, "{}: start-up load of {} root tiles took {:.2} s", script.scene.name(), roots.0, roots.1.as_secs_f64()).map_err(err)?;
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
    // Both device losses of the lit dive (window 8) came after long runs on a hot GPU, so the measured span starts cool (ADR 0057,
    // TEST-033). No warm-up: see `StartSteps`.
    if start_steps(smoke).cool {
        writeln!(out, "{}", planet_perf::cool_gpu().line(script.scene.name())).map_err(err)?;
    }
    // The clock, before, during (a sampler thread: a process spawn never on the frame path) and after the case.
    let mut clocks: Vec<planet_perf::ClockSample> = Vec::new();
    let cpu_before = (!smoke).then(planet_perf::host::query_cpu_clock).flatten();
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
    // The host's load beside the measured part, on its own thread like the clock sampler (TEST-023).
    let load_sampler = (!smoke).then(|| planet_perf::load::LoadSampler::start(planet_perf::load::SAMPLE_PERIOD_S));
    let (counters_before, job_log) = (world.counters(), d.rig.managed.manager().streamer().job_log());
    let jobs_before = job_log.len();
    let n = if smoke { script.cameras.len().min(smoke_frames) } else { script.cameras.len() };
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
    // The tile loads of the measured part: the jobs that ran and what the source and the pack did (STRM-022, recorded only).
    let mut load_counts = {
        let jobs: Vec<planet_perf::streaming::LoadJob> = job_log
            .since(jobs_before)
            .iter()
            .map(|j| planet_perf::streaming::LoadJob {
                wait_us: j.wait_us,
                run_us: j.run_us,
                running: j.running,
                done: j.done,
                level: j.level,
            })
            .collect();
        planet_perf::streaming::load_counts(&jobs, &load_counters(&world.counters().since(&counters_before)))
    };
    if let Some((stop, samples, handle)) = sampler {
        stop.store(true, Ordering::Relaxed);
        let _ = handle.join();
        clocks.extend(samples.lock().expect("clock samples").iter().cloned());
        clocks.extend(planet_perf::query_nvidia_smi());
    }
    let host_load = load_sampler.map_or_else(planet_perf::load::CaseLoad::not_measured, planet_perf::load::LoadSampler::stop);
    let cpu_after = (!smoke).then(planet_perf::host::query_cpu_clock).flatten();
    let pool = d.rig.pool;
    let torn = Instant::now();
    drop(d);
    let teardown_s = torn.elapsed().as_secs_f64();
    load_counts.push(("load_startup_roots".into(), roots.0 as u64));
    load_counts.push(("load_startup_roots_ms".into(), u64::try_from(roots.1.as_millis()).unwrap_or(u64::MAX)));
    if let Some(c) = &calib_before {
        load_counts.extend(planet_perf::streaming::calibration_counts("before", c));
        load_counts.extend(planet_perf::streaming::calibration_counts("after", &planet_perf::streaming::calibrate_cpu(calib_threads)));
    }
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
    Ok(CaseRun { frames, blanks, settle_s, clocks, teardown_s, cpu_before, cpu_after, pool, load_counts, host_load })
}

/// The `BLANK TILE` lines of a case, which fail the run after the logs are written (ADR 0032's hard measure: a tile, root or
/// ancestor, for every visible node): the frames whose draw had a blank node, and, where the scene gates it (`spec.uncovered`),
/// the frames with nodes of their fresh selection that nothing drawn covers, each with its tile ids.
fn blank_failure_lines(
    scene: &str,
    spec: &planet_perf::streaming::CaseSpec,
    frames: &[FrameRecord],
    blanks: &[(usize, Vec<u64>)],
) -> Vec<String> {
    let mut lines: Vec<String> = blanks
        .iter()
        .map(|(k, ids)| {
            format!(
                "{scene}: frame {k} drew a blank node; tiles {}",
                if ids.is_empty() {
                    "nothing resident to show (not even a root)".to_string()
                } else {
                    ids.iter().map(|t| format!("{t:#018x}")).collect::<Vec<_>>().join(" ")
                }
            )
        })
        .collect();
    if spec.uncovered == planet_perf::streaming::Gate::Gated {
        for (k, f) in frames.iter().enumerate().filter(|(_, f)| f.uncovered_nodes > 0) {
            lines.push(format!(
                "{scene}: frame {k} had {} node(s) of its fresh selection with nothing drawn at, above or below them; tiles {}",
                f.uncovered_nodes,
                f.uncovered_tiles.iter().map(|t| format!("{t:#018x}")).collect::<Vec<_>>().join(" ")
            ));
        }
    }
    lines
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
    let smoke_frames = if o.given.contains("--frames") { o.frames } else { SMOKE_FRAMES };
    let scenes = scenes_of(&o.scene);
    if scenes.is_empty() {
        return Err(format!("unknown streaming scene '{}'", o.scene));
    }
    check_options(o)?;
    // The world and its source, before the gate so the bake's load is not measured as the machine's.
    let from_store = o.store.is_some();
    // The lit variant plans with the viewer's fields (the water of a tile whose seabed the view culls is kept: ADR 0053).
    let open = |cold: bool| -> Result<WorldTerrain, String> { open_world(o, cold).map(|w| if o.lit { w.bounding_water() } else { w }) };
    let opened = Instant::now();
    // `dive-cold` reads the bake as the others do and ignores the pack's pre-filled coarse tiles (ADR 0045 point 3).
    let world = Arc::new(open(scenes[0] == StreamScene::DiveCold)?);
    let field = world.field()?;
    writeln!(out, "world opened in {:.1} s", opened.elapsed().as_secs_f64()).map_err(err)?;
    let radius_m = world.radius_m();
    let measured_field = if o.lit {
        format!("{}; lit path (atmosphere, water), {} tiles", world.describe(), ctx.default_tile_format().name())
    } else {
        world.describe()
    };
    let source = source_label(&world, from_store);
    let production = from_store;
    let mut owner_override: Vec<String> = Vec::new();
    // The tile pack at the start (STRM-021, ADR 0045 point 3): the gated dives start with levels F-N pre-filled (F the render floor,
    // LOD-011), checked before the
    // gate so a store without them is refused at once; every other scene records what the pack held.
    let pack_levels = o.prefill_levels.unwrap_or(planet_perf::prefill::GATED_PREFILL_LEVELS);
    let pack_floor = o.floor_for(o.cells);
    let tile_format = ctx.default_tile_format();
    if from_store && !smoke {
        for scene in scenes.iter().filter(|s| needs_prefilled_pack(**s)) {
            let (present, total) = world.prefill_present(pack_floor, pack_levels, o.cells, o.finest_level_for(o.cells), tile_format)?;
            require_prefilled(scene.name(), present, total, pack_floor, pack_levels, world.source())?;
        }
    }
    // A smoke run over a store is also the check of the store before a window (TEST-024): it reads the ocean channel and every pre-filled
    // tile of the compact form, writes nothing, and fails here when the store would fail the window.
    if smoke && from_store {
        print_store_check(&world, o, out)?;
    }
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
        min_level: o.min_level(),
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
         sleep excluded); GPU time is not measured in the pipelined loop (no timestamp readback without a wait), RAM is the peak \
         working set of the process (the scene's own when one scene runs in the process, as in a child of `perf stream all`), driver \
         VRAM is the whole GPU (an upper bound for the scene, shown beside the allocator's slots)"
    )
    .map_err(err)?;
    let mem_base = MemoryFacts {
        share_bytes: Tier::Mid.terrain_share_bytes(),
        vram_cap_bytes: Tier::Mid.vram_cap_bytes(),
        driver_vram_bytes: None,
        process_peak_bytes: None,
        scene_alone: scenes.len() == 1,
        pool_slots: 0,
        share_slots: 0,
    };
    let power_plan = if smoke { None } else { planet_perf::host::query_power_plan() };
    let mut blank_failures: Vec<String> = Vec::new();
    let mut reports: Vec<(StreamScene, RunReport, Vec<String>, CaseRunSummary)> = Vec::new();
    for (index, scene) in scenes.into_iter().enumerate() {
        // Each scene reads the store through a fresh reader: the decoded-block cache of an earlier scene would make this one's
        // cold plans and tile generation warmer than a run of this scene alone (SPIKE-05; the OS file cache stays warm).
        let scene_world = if index == 0 || !from_store { Arc::clone(&world) } else { Arc::new(open(scene == StreamScene::DiveCold)?) };
        // What the tile pack held at the start of this scene, in the words the log records (STRM-021, TEST-021).
        let marker = if scene == StreamScene::DiveCold {
            Some(planet_perf::prefill::COLD_MARKER.to_string())
        } else if from_store {
            let (present, total) = scene_world.prefill_present(pack_floor, pack_levels, o.cells, lod.max_level, tile_format)?;
            if needs_prefilled_pack(scene) && !smoke {
                require_prefilled(scene.name(), present, total, pack_floor, pack_levels, &source)?;
            }
            Some(coarse_marker(present, total, pack_floor, pack_levels))
        } else {
            None
        };
        let scene_source = marker.as_ref().map_or_else(|| source.clone(), |m| planet_perf::prefill::labelled(&source, m));
        if let Some(m) = &marker {
            writeln!(out, "{}: tile pack at the start: {m}", scene.name()).map_err(err)?;
        }
        let built = Instant::now();
        let scene_field = scene_world.field()?;
        let script = if crate::watergap::enabled() {
            let is_sea = |d: planet_core::Vec3| crate::litperf::carries_sea(&scene_world, d);
            let level = scene_world.definition().sea(planet_world_def::BodyId::PLANET).map_err(|e| e.to_string())?.level_m;
            crate::watergap::script(radius_m, aspect, scene_field.as_ref(), level, &is_sea, scene)?
        } else {
            build_script(scene, radius_m, aspect, scene_field.as_ref(), LOOK_SEED)
        };
        drop(scene_field);
        let shown = if smoke { script.cameras.len().min(smoke_frames) } else { script.cameras.len() };
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
        let lit_setup = if o.lit { Some(crate::litperf::LitSetup::new(o, &scene_world, &lod)?) } else { None };
        let run = run_case(ctx, &scene_world, &script, &req_of, smoke, o.calibrate, smoke_frames, o.gen_workers(), lit_setup, out)?;
        writeln!(out, "{}: case ran in {:.1} s (teardown {:.2} s)", scene.name(), case_started.elapsed().as_secs_f64(), run.teardown_s)
            .map_err(err)?;
        if o.lit {
            writeln!(out, "{}: water of {} tiles computed in the tile jobs (ADR 0056)", scene.name(), scene_world.water_book().filled())
                .map_err(err)?;
        }
        let mem = MemoryFacts {
            driver_vram_bytes: if smoke { None } else { planet_perf::streaming::query_gpu_memory_used() },
            process_peak_bytes: planet_perf::streaming::query_process_peak_bytes(),
            pool_slots: u64::from(run.pool.main_slots()),
            share_slots: share_slots_of(&run.pool, mem_base.share_bytes),
            ..mem_base.clone()
        };
        let max_resident = run.frames.iter().map(|f| f.resident as u64).max().unwrap_or(0);
        if !smoke {
            writeln!(
                out,
                "{}: cpu_clock_before {}, cpu_clock_after {} (effective clock at an idle moment before and after the case, recorded not judged); power_plan {}",
                scene.name(),
                planet_perf::host::clock_text(run.cpu_before.as_ref()),
                planet_perf::host::clock_text(run.cpu_after.as_ref()),
                power_plan.as_ref().map_or("not measured".to_string(), ToString::to_string)
            )
            .map_err(err)?;
        }
        let memory_line = planet_perf::session::SceneMemory {
            scene: scene.name().to_string(),
            ram_peak_bytes: mem.process_peak_bytes,
            vram_used_bytes: mem.driver_vram_bytes,
            pool_slots: mem.pool_slots,
            max_resident,
            share_slots: mem.share_slots,
        }
        .line();
        writeln!(out, "{memory_line}").map_err(err)?;
        let spec = case_spec(scene.name(), run.frames.len(), FRAME_STEP_S, script.descent_frames).expect("a streaming scene");
        let verdicts = case_verdicts(&spec, &run.frames, &run.blanks, &mem);
        // The cold dive is never gated (ADR 0045 point 3): a measure over its budget reads "over (not gated)", not "OVER".
        let lines: Vec<String> = verdicts
            .iter()
            .map(|v| v.line(scene.name()))
            .map(|l| match (scene, l.strip_suffix(" OVER")) {
                (StreamScene::DiveCold, Some(rest)) => format!("{rest} over (not gated)"),
                _ => l,
            })
            .collect();
        blank_failures.extend(blank_failure_lines(scene.name(), &spec, &run.frames, &run.blanks));
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
        counts.push(("pool_slots".into(), mem.pool_slots));
        counts.push(("share_slots".into(), mem.share_slots));
        counts.push(("max_resident_slots".into(), max_resident));
        // Tiles the scene took from the pack instead of generating (0 for the cold reader).
        counts.push(("prefill_tiles_used".into(), scene_world.prefill_hits()));
        // Whether the figure above is the scene's own peak (one scene in the process): `accept M4` refuses RAM from a shared process (TEST-021).
        counts.push(("scene_alone".into(), u64::from(mem.scene_alone)));
        // The tile source's load during the measured part (recorded only, never compared: STRM-022).
        counts.extend(run.load_counts.iter().cloned());
        for (name, clock) in [("cpu_clock_before_mhz", run.cpu_before), ("cpu_clock_after_mhz", run.cpu_after)] {
            if let Some(c) = clock {
                counts.push((name.into(), c.effective_mhz().round() as u64));
            }
        }
        let report = RunReport {
            path_name: format!("{}/{}", if o.lit { "stream-lit" } else { "stream" }, scene.name()),
            vram_cap_gb: 8,
            stats,
            clocks_before: run.clocks.first().cloned(),
            clocks_after: lowest,
            metrics: column_metrics(&run.frames),
            counts,
        };
        let summary = CaseRunSummary {
            host_load: run.host_load,
            frames: run.frames,
            settle_s: run.settle_s,
            clocks: run.clocks.len(),
            source: scene_source,
        };
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
    let verdict = planet_perf::validity::SessionVerdict::assess(
        &crate::facts(ctx),
        scheme.as_deref(),
        &run_reports,
        smoke,
        &validity,
        &owner_override,
    );
    if smoke {
        // A smoke run prints no session verdict; its causes are still named.
        for w in verdict.warnings() {
            writeln!(out, "WARNING (not valid for time budgets): {w}").map_err(err)?;
        }
    } else {
        crate::session_verdict_line_for(out, &verdict, false)?;
    }
    let protocol = Protocol {
        warmup_frames: 0,
        measured_frames: 0,
        runs: 1,
        internal_resolution: size,
        field: Some(measured_field.clone()),
        source: Some(source.clone()),
        gen: Some(gen_settings(o)),
        smoke,
        ..Protocol::default()
    };
    let single = reports.len() == 1;
    // The lit variant keeps its own logs and baselines beside the debug path's (`<scene>-lit.json`).
    let lit_suffix = if o.lit { "-lit" } else { "" };
    for (scene, report, lines, summary) in &reports {
        let clock_state = planet_perf::view_gpu_validity(std::slice::from_ref(report), &report.path_name);
        let state = match &clock_state {
            planet_perf::clock::GpuValidity::Valid(s) => s.to_string(),
            planet_perf::clock::GpuValidity::Invalid(r) | planet_perf::clock::GpuValidity::Below(_, r) => r.clone(),
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
        // The scene's own source: the place the bake was read from and what the tile pack held at its start.
        let scene_protocol = Protocol { source: Some(summary.source.clone()), ..protocol.clone() };
        writeln!(out, "{}: {}", scene.name(), summary.host_load.line()).map_err(err)?;
        for n in summary.host_load.notes() {
            writeln!(out, "  {}: LOAD NOTE (recorded, the session's validity is unchanged): {n}", scene.name()).map_err(err)?;
        }
        let json = planet_perf::report_json_full(
            &crate::facts(ctx),
            scheme.as_deref(),
            &owner_override,
            &validity,
            &scene_protocol,
            one,
            &[],
            &[(report.path_name.clone(), summary.host_load.clone())],
        );
        let path = match (&o.out, single) {
            (Some(p), true) => out_log_path(p, &format!("{}{lit_suffix}.json", scene.name()))?,
            (Some(dir), false) => dir.join(format!("{}{lit_suffix}.json", scene.name())),
            (None, _) => std::path::Path::new("perf").join("logs").join(format!("{}{lit_suffix}.json", scene.name())),
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
        return Err(format!(
            "{} frame(s) drew a blank tile or left a visible node without one (STRM-002): {}",
            blank_failures.len(),
            blank_failures[0]
        ));
    }
    let over: Vec<String> = reports
        .iter()
        .flat_map(|(s, _, lines, _)| lines.iter().filter(|l| l.ends_with(" OVER")).map(move |l| format!("{}: {}", s.name(), l.trim())))
        .collect();
    writeln!(
        out,
        "{} gated measure(s) over budget{}",
        over.len(),
        if smoke || !verdict.frame_valid() { " (the session does not count)" } else { "" }
    )
    .map_err(err)?;
    Ok(())
}

/// The generation pool the session ran with, as the log records it (`protocol.gen`, TEST-012).
fn gen_settings(o: &Options) -> planet_perf::GenSettings {
    let g = o.gen_workers();
    planet_perf::GenSettings {
        workers: g.workers,
        ahead: g.ahead,
        reserve: o.gen_reserve_used(),
        below_normal: g.priority == planet_streaming::Priority::BelowNormal,
    }
}

/// Slots of the main pool the tier's share can hold: the share less the fixed items (small vertex pool, index buffers, node records,
/// slot records), in whole blocks (a block is `slot_bytes * block_slots`).
fn share_slots_of(pool: &planet_streaming::PoolConfig, share_bytes: u64) -> u64 {
    let block = pool.slot_bytes * u64::from(pool.block_slots);
    share_bytes.saturating_sub(pool.fixed_bytes()).checked_div(block).unwrap_or(0) * u64::from(pool.block_slots)
}

struct CaseRunSummary {
    host_load: planet_perf::load::CaseLoad,
    frames: Vec<FrameRecord>,
    settle_s: Option<f64>,
    clocks: usize,
    /// The log's `protocol.source` for this scene: the bake's place and what the tile pack held at the start (STRM-021).
    source: String,
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

    // spec: TEST-030
    #[test]
    fn the_out_of_a_single_scene_is_a_directory_or_a_json_file_and_anything_else_fails_before_the_case() {
        let d = tmp("outdir");
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(out_log_path(&d, "dive-lit.json").unwrap(), d.join("dive-lit.json"), "an existing directory");
        let fresh = d.join("logs");
        assert_eq!(out_log_path(&fresh, "dive-lit.json").unwrap(), fresh.join("dive-lit.json"), "a path without an extension");
        let file = d.join("mine.json");
        assert_eq!(out_log_path(&file, "dive-lit.json").unwrap(), file, "a .json path is the file");
        let odd = d.join("notes.txt");
        std::fs::write(&odd, "x").unwrap();
        let e = out_log_path(&odd, "dive-lit.json").unwrap_err();
        assert!(e.contains("give a directory") && e.contains("dive-lit.json"), "{e}");
        // The refusal comes from the option check, before any case runs.
        let o = opts(&["test-render", "--perf-smoke", "--scene", "dive", "--allow-memory-bake", "--out", odd.to_str().unwrap()]);
        assert!(check_options(&o).unwrap_err().contains("give a directory"));
        let lit = opts(&["test-render", "--perf-smoke", "--scene", "lit", "--allow-memory-bake", "--out", odd.to_str().unwrap()]);
        assert!(check_options(&lit).unwrap_err().contains("give a directory"), "a lit scene is refused before its case too");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: TEST-019
    #[test]
    fn the_registry_of_scenes_is_the_same_in_the_scripts_and_the_verdicts_and_names_the_six_and_the_cold_dive() {
        let scripts: Vec<&str> = StreamScene::ALL.iter().map(|s| s.name()).collect();
        assert_eq!(scripts, planet_perf::streaming::SCENES);
        assert_eq!(scripts, ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km", "dive-cold"]);
        for s in &scripts {
            assert!(is_stream_scene(s) && scenes_of(s).len() == 1, "{s}");
        }
        assert_eq!(scenes_of("streaming").len(), 7);
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

    // spec: TEST-037
    #[test]
    fn a_streaming_scene_cools_down_in_a_measured_run_and_never_warms_up() {
        // A smoke run takes no step (no measurement, perhaps no nvidia-smi); a measured run cools the GPU first.
        assert_eq!(start_steps(true), StartSteps { cool: false });
        assert_eq!(start_steps(false), StartSteps { cool: true });
        // No scene warms up, whether it starts from a settled view or from an empty cache: the loop is CPU-bound at 60 Hz, so a warm-up
        // cannot reach the clock floor (look-2km: P3 1,087 MHz after 30 s unpaced), and a warm-up from an empty cache would load what
        // the scene measures. The steps have no warm-up field to set.
        for scene in planet_frame::scenes::StreamScene::ALL {
            let script = planet_frame::scenes::build_script(scene, 6_371_000.0, 16.0 / 9.0, &planet_generators::height::Reference(1), 1);
            assert!(start_steps(false).cool, "{scene:?} ({} a settled start)", if script.prelude.is_some() { "has" } else { "has not" });
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
            uncovered_tiles: Vec::new(),
            blank_nodes: 0,
            upload_bytes: 0,
            upload_calls: 0,
            resident: 0,
            in_flight: 0,
            queued: 0,
            dispatched: 0,
            uploaded: 0,
            cancelled: 0,
            degraded: 0,
            allocated_bytes: 0,
            ..FrameRecord::default()
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

    // spec: TEST-018, TEST-019, TEST-020
    #[test]
    fn a_visible_node_without_a_drawn_tile_fails_the_run_like_a_blank_tile_naming_scene_frame_and_tiles_except_in_dive_fast() {
        let mut frames: Vec<FrameRecord> = (0..4).map(|_| FrameRecord::default()).collect();
        frames[2].uncovered_nodes = 2;
        frames[2].uncovered_tiles = vec![0x1234, 0x5678];
        for scene in ["dive", "teleport-1m", "teleport-2km", "look-1m", "look-2km"] {
            let spec = case_spec(scene, 4, FRAME_STEP_S, 0).unwrap();
            let lines = blank_failure_lines(scene, &spec, &frames, &[]);
            assert_eq!(lines.len(), 1, "{scene}: {lines:?}");
            assert!(
                lines[0].starts_with(&format!("{scene}: frame 2 had 2 node(s)"))
                    && lines[0].contains("0x0000000000001234 0x0000000000005678"),
                "{lines:?}"
            );
        }
        let spec = case_spec("dive-fast", 4, FRAME_STEP_S, 0).unwrap();
        assert!(blank_failure_lines("dive-fast", &spec, &frames, &[]).is_empty(), "dive-fast is report-only");
        // A blank node in the draw fails every scene, dive-fast included, as before.
        assert_eq!(blank_failure_lines("dive-fast", &spec, &frames, &[(1, vec![0x42])]).len(), 1);
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
            &[
                "generate",
                "--global-dir",
                dir.to_str().unwrap(),
                "--global-res",
                "8",
                "--seed",
                "3",
                "--threads",
                "2",
                "--prefill-levels",
                "0",
            ]
            .map(String::from),
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
        // spec: STRM-025
        // A store as an earlier build left it (every record but the water summary) is refused, with the command that adds it.
        let old = std::env::temp_dir().join(format!("planet-no-summary-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&old);
        {
            let world = planet_cache::global_store::GlobalWorld::from_definition(&def()).unwrap();
            let src = planet_cache::pack::TileStore::open(&dir, world.params()).unwrap();
            let mut writer =
                planet_cache::pack::PackWriter::open(&old, world.params(), planet_cache::pack::DEFAULT_MAX_PACK_BYTES).unwrap();
            for item in src.sample_records(0) {
                let rec = item.unwrap().record;
                if rec.kind != planet_cache::global_store::KIND_GLOBAL_WATER_SUMMARY {
                    writer.append(&rec).unwrap();
                }
            }
            writer.finish().unwrap();
        }
        let e = WorldTerrain::open_store(def(), &old, 8).err().unwrap();
        assert!(e.contains("water summary") && e.contains("generate --global-dir"), "{e}");
        let _ = std::fs::remove_dir_all(&old);
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

#[cfg(test)]
mod accept_constants_tests {
    use planet_streaming::budget::Tier;

    // spec: TEST-021
    #[test]
    fn the_acceptance_judges_the_share_and_cap_the_scene_runs_with() {
        assert_eq!(planet_perf::accept::MID_TIER_SHARE_BYTES, Tier::Mid.terrain_share_bytes());
        assert_eq!(planet_perf::accept::MID_TIER_CAP_BYTES, Tier::Mid.vram_cap_bytes());
        assert_eq!(planet_perf::accept::PRODUCTION_CELLS, u64::from(super::PRODUCTION_BAKE_RES));
    }
}

#[cfg(test)]
mod prefill_pack_tests {
    use super::*;

    // spec: TEST-019, STRM-021
    #[test]
    fn only_the_dives_that_are_judged_need_the_pre_filled_pack_and_a_pack_that_lacks_tiles_is_refused_with_the_remedy() {
        let needs: Vec<&str> = StreamScene::ALL.into_iter().filter(|s| needs_prefilled_pack(*s)).map(StreamScene::name).collect();
        assert_eq!(needs, ["dive", "dive-fast"], "dive-cold ignores the pack, the others use what is there");
        assert!(require_prefilled("dive", 1920, 1920, 3, 4, "store S").is_ok());
        let e = require_prefilled("dive", 126, 1920, 3, 4, "store D:/s, 16384 cells").unwrap_err();
        for needle in [
            "dive starts with levels 3-4 pre-filled",
            "holds 126 of the 1920 tiles",
            "store D:/s, 16384 cells",
            "generate --prefill-dir <store> --global-res 16384",
            "dive-cold",
        ] {
            assert!(e.contains(needle), "{needle}: {e}");
        }
        // The marker the log records: pre-filled only when every tile of the checked levels was there.
        assert_eq!(coarse_marker(1920, 1920, 3, 4), "prefilled coarse pack (levels 3-4 present at start)");
        assert_eq!(coarse_marker(2046, 2046, 0, 4), "prefilled coarse pack (levels 0-4 present at start)", "floor 0 as before");
        assert!(coarse_marker(1919, 1920, 3, 4).starts_with("no pre-filled coarse pack (1919 of 1920"));
        assert_eq!(
            planet_perf::prefill::parse(&planet_perf::prefill::labelled("store S, 16384 cells", &coarse_marker(1920, 1920, 3, 4))).1,
            planet_perf::prefill::PrefillState::Prefilled { floor: 3, levels: 4 }
        );
    }
}
