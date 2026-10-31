//! The pipelined frame loop (STRM-016, ADR 0032 point 5): a loop that never waits for a plan and never waits for the GPU of the
//! frame it just submitted, driven by a script of cameras (`test-render --scene terrain --pipelined [--then SCRIPT]`). What it
//! draws is decided by `planet_frame::pipeline` (GPU-free); this file only moves data: plans come from a worker, meshes from
//! the streamer's workers, and the renderer executes the plan it is given. The synchronous path (`test-render` without
//! `--pipelined`) stays the reference: once the plans have settled the image is the same.

use crate::err;
use crate::skeleton::MeshSource;
use crate::worldfield::WorldTerrain;
use planet_frame::managed::{FrameCounts, ManagedLoop, TileSink, UploadStats};
use planet_frame::pipeline::{
    residency_of, DrawKind, FrameColumns, FramesInFlight, PlanField, PlanPipeline, PlanWorker, MAX_FRAMES_IN_FLIGHT,
};
use planet_frame::{PlanError, ResidencySet, SelectionCache, TerrainPlan, TerrainRequest};
use planet_generators::compact::CompactTile;
use planet_render::{GpuContext, RgbaFrame, TerrainRenderer};
use planet_streaming::jobs::{Class, JobSystem};
use planet_streaming::{Priority, ResidencyManager, SlotRef, Spawner, StreamConfig, Streamer, SystemClock};
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

/// Plans over a world: the field is built inside the job (it borrows the world's bake), so the job owns nothing but the world.
struct WorldPlanner(Arc<WorldTerrain>);

impl PlanField for WorldPlanner {
    fn item_revision(&self) -> u64 {
        self.0.definition().revision()
    }

    fn plan(&self, req: &TerrainRequest, cache: &mut SelectionCache, par: &dyn planet_core::par::Par) -> Result<TerrainPlan, PlanError> {
        let field = self.0.field().map_err(PlanError::HeightField)?;
        planet_frame::plan_terrain_par(req, field.as_ref(), cache, par)
    }

    fn residency(&self, req: &TerrainRequest, cache: &mut SelectionCache) -> Result<ResidencySet, PlanError> {
        let field = self.0.field().map_err(PlanError::HeightField)?;
        residency_of(req, field.as_ref(), cache)
    }
}

/// What the loop saw in one frame it drew.
#[derive(Clone, Debug)]
pub struct FrameLog {
    /// Loop iteration (a frame is drawn only when the plan to draw changed).
    pub iteration: u64,
    pub kind: DrawKind,
    pub nodes: usize,
    pub substituted: usize,
    pub columns: FrameColumns,
    /// What the scheduler and the manager did in the iteration that drew the frame (per-iteration counts).
    pub counts: FrameCounts,
    pub in_flight: u32,
    /// The frame whose camera the drawn plan was made for.
    pub base_seq: Option<u64>,
}

/// A script that did not settle within this long is an error, not a hang.
const SETTLE_LIMIT_S: u64 = 300;

/// The renderer as the managed loop's sink: it writes a tile into the slot the manager chose and decides nothing.
pub(crate) struct RendererSink<'a> {
    pub(crate) ctx: &'a GpuContext,
    pub(crate) renderer: &'a mut TerrainRenderer,
}

impl TileSink for RendererSink<'_> {
    fn upload_at(&mut self, tile: &CompactTile, slot: SlotRef) -> Result<(), String> {
        self.renderer.upload_tile_at(self.ctx, tile, slot).map_err(|e| e.to_string())
    }

    fn upload_batch(&mut self, tiles: &[(&CompactTile, SlotRef)]) -> Result<UploadStats, String> {
        let before = self.renderer.upload_counters();
        self.renderer.upload_tiles_at(self.ctx, tiles).map_err(|e| format!("placing a batch of {} tiles: {e}", tiles.len()))?;
        let after = self.renderer.upload_counters();
        Ok(UploadStats { calls: after.write_calls - before.write_calls, staging_bytes: after.staging_bytes - before.staging_bytes })
    }

    fn forget(&mut self, tile: planet_core::TileId) {
        self.renderer.forget(tile);
    }
}

/// The parts of the pipelined loop on the real renderer: the renderer, the managed loop (plan worker, residency manager over a
/// streamer, prefetch), the frames in flight and the pool configuration. Built once per script or perf case; `run_script` and
/// the streaming perf scenes (`streamperf.rs`) drive it.
pub(crate) struct Rig {
    pub renderer: TerrainRenderer,
    pub managed: ManagedLoop,
    pub fi: FramesInFlight,
    pub pool: planet_streaming::PoolConfig,
    /// The start-up load of the roots (the tiles of the render floor, STRM-001): how many and how long it waited for them (a reading,
    /// STRM-022: the cold start of a run without a pre-fill is the one without the pack's tiles).
    pub roots: (usize, std::time::Duration),
    /// The horizon bakes' pool: the idle class of the terrain share (`WorldTerrain::enable_viewer_horizon_on`).
    pub horizon_pool: Arc<dyn Spawner>,
    /// The job system the tile and plan workers run on (BUDG-004); last, so it is dropped after the streamer and the plan worker that feed it.
    pub _system: Arc<planet_streaming::jobs::JobSystem>,
}

impl Rig {
    /// A rig for requests like `first`: one tile size (cells) and one finest level, the pools sized for the first request.
    pub(crate) fn new(
        ctx: &GpuContext,
        world: &Arc<WorldTerrain>,
        first: &TerrainRequest,
        gen: planet_streaming::GenWorkers,
    ) -> Result<Rig, String> {
        Rig::new_with_progress(ctx, world, first, gen, &mut |_, _| {})
    }

    /// [`Rig::new`] reporting the start-up load of the roots: `progress(done, total)` as in
    /// `Streamer::load_roots_blocking_with` (the window shows it in its title).
    pub(crate) fn new_with_progress(
        ctx: &GpuContext,
        world: &Arc<WorldTerrain>,
        first: &TerrainRequest,
        gen: planet_streaming::GenWorkers,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<Rig, String> {
        crate::stall::phase_mark("rig start");
        let mut renderer = TerrainRenderer::with_frames_in_flight(ctx, first.size, first.lod.cells, u64::from(MAX_FRAMES_IN_FLIGHT));
        // REND-013: the pools' first block, the ring and every pipeline's first draw happen here, before frame 0 (frame 0 stays
        // a measured frame, ADR 0032).
        renderer.warm_up(ctx);
        crate::stall::phase_mark("renderer warm-up");
        // One job system for the rig (BUDG-004): the tile workers below normal priority (ADR 0046) and the plan worker, a frame-critical thread at normal priority,
        // on the one share `terrain`, whose ceiling holds both.
        // The horizon bakes (STRM-060, STRM-064) are the idle class of the same share: `horizon` more threads that run idle jobs only, so a bake never holds a worker a visible job needs and the tile jobs keep their `gen.workers` threads.
        let horizon = crate::horizonworld::horizon_workers(gen);
        let system = Arc::new(JobSystem::with_dedicated_workers(gen.workers, gen.priority, 1, Priority::Normal, horizon));
        let terrain = system.share_with_idle("terrain", gen.workers + 1 + horizon, horizon);
        let horizon_pool: Arc<dyn Spawner> = Arc::new(terrain.clone());
        world.set_job_par(Arc::new(terrain.par()));
        let plan_pool: Arc<dyn Spawner> = Arc::new(terrain.classed(Class::FrameCritical));
        let plan_par = Arc::new(terrain.par());
        let tile_pool: Arc<dyn Spawner> = Arc::new(terrain);
        let pipe = PlanPipeline::new(PlanWorker::with_par(plan_pool, Arc::new(WorldPlanner(Arc::clone(world))), plan_par));
        // The streamer counts its slots in the renderer's pools: what it places, the renderer can hold.
        let pool = *renderer.pool_config();
        let clock: Arc<dyn planet_streaming::Clock> = Arc::new(SystemClock::new());
        let mut streamer = Streamer::new(
            Arc::new(MeshSource { world: Arc::clone(world), max_level: first.lod.max_level, cells: first.lod.cells }),
            // The tile threads and the jobs running at once are one number (STRM-020).
            tile_pool,
            Arc::clone(&clock),
            gen.apply(StreamConfig {
                capacity: pool.main_slots() as usize,
                retire_frames: u64::from(MAX_FRAMES_IN_FLIGHT),
                retry_after_ms: 1000,
                floor: first.lod.floor_level(),
                record_dispatches: false,
                record_changes: true,
                ..Default::default()
            }),
        );
        // The one wait of the loop (CON-22): the roots (the tiles of the render floor, 384 at floor 3) exist before the first frame,
        // so there is always something to draw; a load step of the body, not a steady frame (STRM-001).
        let roots_started = Instant::now();
        streamer.load_roots_blocking_with(progress)?;
        crate::stall::phase_mark("root tiles");
        let roots = (streamer.stats().resident, roots_started.elapsed());
        // Prefetch along the camera's translation (STRM-018, ADR 0031): the same clock as the streamer's.
        let managed = ManagedLoop::new(pipe, ResidencyManager::new(streamer)).with_prefetch(
            planet_frame::prefetch::PrefetchConfig::default(),
            first.lod.radius_m,
            clock,
        );
        let mut managed = managed;
        // The roots go to the GPU here too (STRM-001, ADR 0064): the start-up load is a step of the body, so frame 0 is a steady frame and
        // carries no root upload (a measured frame stays under the cap of STRM-006 and the CON-21 limits).
        managed.upload_roots(&mut RendererSink { ctx, renderer: &mut renderer })?;
        Ok(Rig { renderer, managed, fi: FramesInFlight::new(MAX_FRAMES_IN_FLIGHT), pool, roots, horizon_pool, _system: system })
    }
}

/// Drive `requests` in order: each camera is held until the frame drawn is the synchronous frame (every node of the plan made for
/// it resident). Returns the last frame's image and the log of the frames drawn.
///
/// The residency manager drives the requests: the plan worker computes the draw set D and, after it, the residency set R from the
/// camera position only; the manager requests D first, then the rest of R, within the slots of the renderer's pools, and the
/// loop executes its placement with `upload_at` (STRM-005, STRM-006, STRM-007).
///
/// With `lockstep` every iteration ends by waiting until the plan worker is idle and no generation job is running, so what
/// arrives before the next iteration is everything the last one asked for: the sequence of frames and the tiles each uploads are
/// then a function of the script, not of thread timing (a test mode; the loop otherwise never waits).
pub fn run_script(
    ctx: &GpuContext,
    world: &Arc<WorldTerrain>,
    requests: &[TerrainRequest],
    gen: planet_streaming::GenWorkers,
    lockstep: bool,
    out: &mut dyn Write,
) -> Result<(RgbaFrame, Vec<FrameLog>), String> {
    let first = requests.first().ok_or("an empty script")?;
    // One tile size and one finest level per script: the pools are sized for the first request's cells, and the streamer does not
    // re-fetch resident tiles when the cap changes, so a script that changes it is refused.
    if let Some(r) = requests.iter().find(|r| r.lod.cells != first.lod.cells || r.lod.max_level != first.lod.max_level) {
        return Err(format!(
            "every request of a script must use the first one's cells ({}) and finest level ({}), got cells {} and level {}",
            first.lod.cells, first.lod.max_level, r.lod.cells, r.lod.max_level
        ));
    }
    // The streamer's roots are the tiles of the first request's render floor and are never evicted: another floor is refused.
    if let Some(r) = requests.iter().find(|r| r.lod.floor_level() != first.lod.floor_level()) {
        return Err(format!(
            "every request of a script must use the first one's render floor ({}), got {}",
            first.lod.floor_level(),
            r.lod.floor_level()
        ));
    }
    // The job system stays bound for the whole run: dropping it ends the workers.
    let Rig { mut renderer, mut managed, mut fi, pool, _system, .. } = Rig::new(ctx, world, first, gen)?;
    let mut log: Vec<FrameLog> = Vec::new();
    let mut last_drawn: Option<TerrainPlan> = None;
    let mut iteration = 0u64;
    // Worker time of plans delivered since the last frame drawn (a delivery on an iteration that draws nothing is not lost).
    let mut plan_ms_carry = 0.0f64;
    let mut first_cover = 0usize;
    for (n, req) in requests.iter().enumerate() {
        let started = Instant::now();
        loop {
            iteration += 1;
            let wall = Instant::now();
            // Scheduler and upload: the manager requests, evicts and collects; the renderer receives what became resident in
            // the slot chosen for it. The plan for this camera is asked for, never waited for; the frame draws what it can.
            let frame = managed.begin_frame_strict(req, &mut RendererSink { ctx, renderer: &mut renderer })?;
            plan_ms_carry += frame.pipeline.delivered.map_or(0.0, |d| d.1);
            if n == 0 && log.is_empty() {
                first_cover = frame.pipeline.draw.plan.nodes.len();
            }
            let draw = &frame.pipeline.draw;
            let settled = draw.kind == DrawKind::Exact;
            // A frame is drawn when its plan differs from the last one drawn (a real loop draws every frame; a harness on a
            // software adapter has no use for identical ones).
            if last_drawn.as_ref() != Some(&draw.plan) {
                fi.retire_to(renderer.retired_frames(ctx));
                while fi.is_full() {
                    // The bound is reached: the only wait the loop knows.
                    fi.note_wait();
                    fi.retire_to(renderer.wait_oldest(ctx));
                }
                renderer.set_preview_of(draw);
                let (timing, _) = renderer.submit(ctx, &draw.plan).map_err(|e| format!("frame {iteration}: {e}"))?;
                fi.submit();
                debug_assert!(fi.in_flight() <= MAX_FRAMES_IN_FLIGHT);
                log.push(FrameLog {
                    iteration,
                    kind: draw.kind,
                    nodes: draw.plan.nodes.len(),
                    substituted: draw.substituted,
                    columns: FrameColumns {
                        plan_ms: std::mem::take(&mut plan_ms_carry),
                        scheduler_ms: frame.scheduler_ms,
                        upload_ms: frame.upload_ms,
                        record_ms: timing.cpu_ms,
                        gpu_ms: None,
                        wall_ms: wall.elapsed().as_secs_f64() * 1000.0,
                    },
                    counts: frame.counts,
                    in_flight: fi.in_flight(),
                    base_seq: frame.pipeline.base_seq,
                });
                last_drawn = Some(draw.plan.clone());
            }
            if settled {
                break;
            }
            if started.elapsed().as_secs() > SETTLE_LIMIT_S {
                return Err(format!(
                    "view {n} did not settle within {SETTLE_LIMIT_S} s ({:?}, {} frames in flight)",
                    managed.manager().streamer().stats(),
                    fi.in_flight()
                ));
            }
            if lockstep {
                // Everything this iteration asked for arrives before the next one looks (bounded like the settle itself).
                while managed.pipeline().worker().busy() || managed.manager().streamer().unfinished_jobs() > 0 {
                    if started.elapsed().as_secs() > SETTLE_LIMIT_S {
                        return Err(format!("view {n}: the plan worker or the generation did not go idle within {SETTLE_LIMIT_S} s"));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    // The settled frame is the last one submitted; reading the colour target waits for it.
    let image = renderer.read_color(ctx);
    let count = |k: DrawKind| log.iter().filter(|f| f.kind == k).count();
    let stats = managed.manager().streamer().stats();
    writeln!(
        out,
        "pipelined: {} frames drawn in {iteration} iterations (cover {}, stale {}, exact {}), first frame {first_cover} nodes, peak {} of {} in flight, {} waits for the GPU; plans {:?}; slots {} resident of {} (dispatched {}, completed {}, cancelled {}, evicted {}, blocked {})",
        log.len(),
        count(DrawKind::Cover),
        count(DrawKind::Stale),
        count(DrawKind::Exact),
        fi.peak(),
        fi.max(),
        fi.waits(),
        managed.pipeline().worker().counts(),
        stats.resident,
        pool.main_slots(),
        stats.dispatched,
        stats.completed,
        stats.cancelled,
        stats.evicted,
        stats.blocked,
    )
    .map_err(err)?;
    Ok((image, log))
}

/// The per-frame log as text: one line per frame drawn, the time columns of TEST-012, then the scheduler's counts.
pub fn log_text(log: &[FrameLog]) -> String {
    let mut s = format!(
        "# iteration kind nodes substituted in_flight base_seq {} {}\n",
        FrameColumns::NAMES.join(" "),
        FrameCounts::NAMES.join(" ")
    );
    for f in log {
        let base = f.base_seq.map_or("-".to_string(), |b| b.to_string());
        let kind = match f.kind {
            DrawKind::Exact => "exact",
            DrawKind::Stale => "stale",
            DrawKind::Cover => "cover",
        };
        let v = f.columns.values();
        let counts: Vec<String> = f.counts.values().iter().map(u64::to_string).collect();
        s.push_str(&format!(
            "{} {kind} {} {} {} {base} {:.3} {:.3} {:.3} {:.3} {} {:.3} {}\n",
            f.iteration,
            f.nodes,
            f.substituted,
            f.in_flight,
            v[0],
            v[1],
            v[2],
            v[3],
            if v[4].is_nan() { "-".to_string() } else { format!("{:.3}", v[4]) },
            v[5],
            counts.join(" ")
        ));
    }
    s
}
