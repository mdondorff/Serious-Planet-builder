//! The pipelined frame loop (STRM-016, ADR 0032 point 5): a loop that never waits for a plan and never waits for the GPU of the
//! frame it just submitted, driven by a script of cameras (`test-render --scene terrain --pipelined [--then SCRIPT]`). What it
//! draws is decided by `planet_frame::pipeline` (GPU-free); this file only moves data: plans come from a worker, meshes from
//! the streamer's workers, and the renderer executes the plan it is given. The synchronous path (`test-render` without
//! `--pipelined`) stays the reference: once the plans have settled the image is the same.

use crate::err;
use crate::skeleton::MeshSource;
use crate::worldfield::WorldTerrain;
use planet_core::TileId;
use planet_frame::pipeline::{
    DrawKind, FrameColumns, FramesInFlight, PlanField, PlanPipeline, PlanWorker, ResidentTile, MAX_FRAMES_IN_FLIGHT,
};
use planet_frame::{plan_terrain_threaded, PlanError, SelectionCache, TerrainPlan, TerrainRequest};
use planet_render::{GpuContext, RgbaFrame, TerrainRenderer};
use planet_streaming::{StreamConfig, Streamer, SystemClock, ThreadPool};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

/// Plans over a world: the field is built inside the job (it borrows the world's bake), so the job owns nothing but the world.
struct WorldPlanner(Arc<WorldTerrain>);

impl PlanField for WorldPlanner {
    fn plan(&self, req: &TerrainRequest, cache: &mut SelectionCache, threads: usize) -> Result<TerrainPlan, PlanError> {
        let field = self.0.field().map_err(PlanError::HeightField)?;
        plan_terrain_threaded(req, field.as_ref(), cache, threads)
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
    pub in_flight: u32,
    /// The frame whose camera the drawn plan was made for.
    pub base_seq: Option<u64>,
}

/// A script that did not settle within this long is an error, not a hang.
const SETTLE_LIMIT_S: u64 = 300;

/// Drive `requests` in order: each camera is held until the frame drawn is the synchronous frame (every node of the plan made for
/// it resident). Returns the last frame's image and the log of the frames drawn.
pub fn run_script(
    ctx: &GpuContext,
    world: &Arc<WorldTerrain>,
    requests: &[TerrainRequest],
    out: &mut dyn Write,
) -> Result<(RgbaFrame, Vec<FrameLog>), String> {
    let first = requests.first().ok_or("an empty script")?;
    let mut renderer = TerrainRenderer::with_frames_in_flight(ctx, first.size, first.lod.cells, u64::from(MAX_FRAMES_IN_FLIGHT));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let plan_pool = Arc::new(ThreadPool::new(1));
    let mut pipe = PlanPipeline::new(PlanWorker::new(plan_pool, Arc::new(WorldPlanner(Arc::clone(world))), threads));
    let mut streamer = Streamer::new(
        Arc::new(MeshSource { world: Arc::clone(world) }),
        Arc::new(ThreadPool::new(threads.clamp(2, 8))),
        Arc::new(SystemClock::new()),
        StreamConfig {
            max_in_flight: 8,
            capacity: 16_384,
            retire_frames: u64::from(MAX_FRAMES_IN_FLIGHT),
            retry_after_ms: 1000,
            record_dispatches: false,
            ..Default::default()
        },
    );
    // The one wait of the loop (CON-22): the six roots exist before the first frame, so there is always something to draw.
    streamer.load_roots_blocking()?;
    let mut uploaded: HashMap<TileId, ResidentTile> = HashMap::new();
    let mut fi = FramesInFlight::new(MAX_FRAMES_IN_FLIGHT);
    let mut log: Vec<FrameLog> = Vec::new();
    let mut wanted: Vec<(TileId, f64)> = Vec::new();
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
            // Scheduler: what the newest completed plan wants; collect finished tiles; keep the renderer's tiles to the streamer's.
            let t = Instant::now();
            streamer.begin_frame(&wanted);
            let scheduler_ms = t.elapsed().as_secs_f64() * 1000.0;
            // Upload: every wanted tile and ancestor the streamer holds and the renderer does not.
            let t = Instant::now();
            uploaded.retain(|tile, _| {
                let keep = streamer.is_resident(*tile);
                if !keep {
                    renderer.evict(*tile);
                }
                keep
            });
            let mut seen: HashSet<TileId> = HashSet::new();
            for (tile, _) in &wanted {
                let mut up = Some(*tile);
                while let Some(a) = up {
                    if !seen.insert(a) {
                        break;
                    }
                    upload_resident(ctx, &mut renderer, &streamer, &mut uploaded, a)?;
                    up = a.parent();
                }
            }
            for root in planet_core::Face::ALL {
                let id = TileId::new(root, 0, 0, 0).expect("root");
                if !upload_resident(ctx, &mut renderer, &streamer, &mut uploaded, id)? {
                    return Err("a root left the streamer".into());
                }
            }
            let upload_ms = t.elapsed().as_secs_f64() * 1000.0;
            // The plan for this camera is asked for, never waited for; the frame draws what it can.
            let frame = pipe.begin_frame(req, &uploaded)?;
            plan_ms_carry += frame.delivered.map_or(0.0, |d| d.1);
            wanted = frame.wanted.clone();
            if n == 0 && log.is_empty() {
                first_cover = frame.draw.plan.nodes.len();
            }
            let settled = frame.draw.kind == DrawKind::Exact;
            // A frame is drawn when its plan differs from the last one drawn (a real loop draws every frame; a harness on a
            // software adapter has no use for identical ones).
            if last_drawn.as_ref() != Some(&frame.draw.plan) {
                fi.retire_to(renderer.retired_frames(ctx));
                while fi.is_full() {
                    // The bound is reached: the only wait the loop knows.
                    fi.note_wait();
                    fi.retire_to(renderer.wait_oldest(ctx));
                }
                let (timing, _) = renderer.submit(ctx, &frame.draw.plan).map_err(|e| format!("frame {iteration}: {e}"))?;
                fi.submit();
                debug_assert!(fi.in_flight() <= MAX_FRAMES_IN_FLIGHT);
                log.push(FrameLog {
                    iteration,
                    kind: frame.draw.kind,
                    nodes: frame.draw.plan.nodes.len(),
                    substituted: frame.draw.substituted,
                    columns: FrameColumns {
                        plan_ms: std::mem::take(&mut plan_ms_carry),
                        scheduler_ms,
                        upload_ms,
                        record_ms: timing.cpu_ms,
                        gpu_ms: None,
                        wall_ms: wall.elapsed().as_secs_f64() * 1000.0,
                    },
                    in_flight: fi.in_flight(),
                    base_seq: frame.base_seq,
                });
                last_drawn = Some(frame.draw.plan);
            }
            if settled {
                break;
            }
            if started.elapsed().as_secs() > SETTLE_LIMIT_S {
                return Err(format!(
                    "view {n} did not settle within {SETTLE_LIMIT_S} s ({:?}, {} frames in flight)",
                    streamer.stats(),
                    fi.in_flight()
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    // The settled frame is the last one submitted; reading the colour target waits for it.
    let image = renderer.read_color(ctx);
    let count = |k: DrawKind| log.iter().filter(|f| f.kind == k).count();
    writeln!(
        out,
        "pipelined: {} frames drawn in {iteration} iterations (cover {}, stale {}, exact {}), first frame {first_cover} nodes, peak {} of {} in flight, {} waits for the GPU; plans {:?}",
        log.len(),
        count(DrawKind::Cover),
        count(DrawKind::Stale),
        count(DrawKind::Exact),
        fi.peak(),
        fi.max(),
        fi.waits(),
        pipe.worker().counts()
    )
    .map_err(err)?;
    Ok((image, log))
}

/// The per-frame log as text: one line per frame drawn, the time columns of TEST-012 last.
pub fn log_text(log: &[FrameLog]) -> String {
    let mut s = format!("# iteration kind nodes substituted in_flight base_seq {}\n", FrameColumns::NAMES.join(" "));
    for f in log {
        let base = f.base_seq.map_or("-".to_string(), |b| b.to_string());
        let kind = match f.kind {
            DrawKind::Exact => "exact",
            DrawKind::Stale => "stale",
            DrawKind::Cover => "cover",
        };
        let v = f.columns.values();
        s.push_str(&format!(
            "{} {kind} {} {} {} {base} {:.3} {:.3} {:.3} {:.3} {} {:.3}\n",
            f.iteration,
            f.nodes,
            f.substituted,
            f.in_flight,
            v[0],
            v[1],
            v[2],
            v[3],
            if v[4].is_nan() { "-".to_string() } else { format!("{:.3}", v[4]) },
            v[5]
        ));
    }
    s
}

/// Upload `tile` if the streamer holds it and the renderer does not yet; whether the renderer holds it afterwards.
fn upload_resident(
    ctx: &GpuContext,
    renderer: &mut TerrainRenderer,
    streamer: &Streamer<planet_generators::mesh::TileMesh>,
    uploaded: &mut HashMap<TileId, ResidentTile>,
    tile: TileId,
) -> Result<bool, String> {
    match uploaded.entry(tile) {
        std::collections::hash_map::Entry::Occupied(_) => Ok(true),
        std::collections::hash_map::Entry::Vacant(v) => match streamer.get(tile) {
            Some(mesh) => {
                renderer.upload(ctx, &mesh).map_err(|e| e.to_string())?;
                v.insert(ResidentTile { mesh_key: mesh.key, origin: mesh.origin });
                Ok(true)
            }
            None => Ok(false),
        },
    }
}
