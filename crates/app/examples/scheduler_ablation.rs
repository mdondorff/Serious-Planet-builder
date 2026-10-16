//! M4 fix F2 ablation (docs/archive/M4/learnings.md): where the frame thread's scheduler time goes. Drives the managed loop (plan worker over
//! the default world's eroded field on an in-memory bake, residency manager, upload budget, prefetch) with the perf scenes'
//! camera scripts, a source that makes empty compact-format tiles of the right key and a counting sink, no GPU. Prints the
//! scheduler and upload columns (median/p99) and the sizes of D and R. Not production code.
//! `cargo run --release -p planet --example scheduler_ablation -- [--scene look-1m|look-2km|dive|teleport-1m|teleport-2km]
//! [--global-res 256] [--frames 300] [--pace-ms 16]`.

use planet::worldfield::WorldTerrain;
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::TileId;
use planet_frame::managed::{ManagedLoop, TileSink};
use planet_frame::pipeline::{residency_of, PlanField, PlanPipeline, PlanWorker, ResidentRecords};
use planet_frame::scenes::{build_script, StreamScene, LOOK_SEED};
use planet_frame::{plan_terrain_threaded, LodParams, PlanError, ResidencySet, SelectionCache, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::RenderTile;
use planet_generators::mesh::TileMesh;
use planet_streaming::{ResidencyManager, SlotRef, StreamConfig, Streamer, SystemClock, ThreadPool, TileSource};
use planet_world_def::WorldDefinition;
use std::sync::Arc;
use std::time::{Duration, Instant};

const RADIUS_M: f64 = 6_371_000.0;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

struct WorldPlanner(&'static WorldTerrain);

impl PlanField for WorldPlanner {
    fn plan(&self, req: &TerrainRequest, cache: &mut SelectionCache, threads: usize) -> Result<TerrainPlan, PlanError> {
        let field = self.0.field().map_err(PlanError::HeightField)?;
        plan_terrain_threaded(req, field.as_ref(), cache, threads)
    }

    fn residency(&self, req: &TerrainRequest, cache: &mut SelectionCache) -> Result<ResidencySet, PlanError> {
        let field = self.0.field().map_err(PlanError::HeightField)?;
        residency_of(req, field.as_ref(), cache)
    }
}

/// Tiles with the key and origin the plan expects and no vertices.
struct Source {
    world: &'static WorldTerrain,
    lod: LodParams,
}

impl TileSource<RenderTile> for Source {
    fn load(&self, tile: TileId) -> Result<RenderTile, String> {
        let field = self.world.field()?;
        let rec = ResidentRecords::new(field.as_ref(), &TangentWarp, &self.lod).record(tile);
        Ok(RenderTile::Vertex(TileMesh { id: tile, cells: self.lod.cells, key: rec.mesh_key, origin: rec.origin, vertices: Vec::new() }))
    }
}

struct Sink;

impl TileSink for Sink {
    fn upload_at(&mut self, _tile: &RenderTile, _slot: SlotRef) -> Result<(), String> {
        Ok(())
    }

    fn upload_bytes(&self, _tile: &RenderTile) -> u64 {
        8_840
    }

    fn forget(&mut self, _tile: TileId) {}
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn main() -> Result<(), String> {
    let scene = StreamScene::parse(&arg("--scene").unwrap_or_else(|| "look-1m".into())).ok_or("unknown scene")?;
    let res: u32 = arg("--global-res").map_or(256, |v| v.parse().expect("res"));
    let frames: usize = arg("--frames").map_or(300, |v| v.parse().expect("frames"));
    let pace = Duration::from_millis(arg("--pace-ms").map_or(16, |v| v.parse().expect("ms")));
    let world: &'static WorldTerrain = Box::leak(Box::new(WorldTerrain::open(WorldDefinition::single_planet(RADIUS_M, 1), res)?));
    let field = world.field()?;
    let size = (2560u32, 1440u32);
    let lod = LodParams {
        cells: 32,
        viewport_h_px: 1440.0,
        tau_px: 6.0,
        max_level: planet_frame::production_finest_level(32),
        radius_m: RADIUS_M,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    let req_of = |camera| TerrainRequest {
        camera,
        size,
        view: TerrainView::Level,
        lod: lod.clone(),
        face_mapping: TangentWarp.id().to_string(),
        seed: field.seed(),
    };
    let script = build_script(scene, RADIUS_M, f64::from(size.0) / f64::from(size.1), field.as_ref(), LOOK_SEED);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let pipe = PlanPipeline::new(PlanWorker::new(Arc::new(ThreadPool::new(1)), Arc::new(WorldPlanner(world)), threads));
    let clock: Arc<dyn planet_streaming::Clock> = Arc::new(SystemClock::new());
    let mut streamer = Streamer::new(
        Arc::new(Source { world, lod: lod.clone() }),
        Arc::new(ThreadPool::new(threads.clamp(2, 8))),
        Arc::clone(&clock),
        StreamConfig {
            max_in_flight: 8,
            queue_ahead: 0,
            capacity: 230_640,
            vertex_pool_slots: 256,
            retire_frames: 2,
            retry_after_ms: 1000,
            record_dispatches: false,
            record_changes: true,
        },
    );
    streamer.load_roots_blocking()?;
    let mut lp = ManagedLoop::new(pipe, ResidencyManager::new(streamer)).with_prefetch(
        planet_frame::prefetch::PrefetchConfig::default(),
        RADIUS_M,
        clock,
    );
    let mut sink = Sink;
    if let Some(prelude) = script.prelude {
        let req = req_of(prelude);
        let started = Instant::now();
        loop {
            let f = lp.begin_frame(&req, &mut sink)?;
            if f.counts.queued == 0 && f.counts.in_flight == 0 && f.pipeline.draw.kind == planet_frame::pipeline::DrawKind::Exact {
                break;
            }
            if started.elapsed().as_secs() > 600 {
                return Err("prelude did not settle".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        println!(
            "prelude settled in {:.1} s: D {} R {} resident {}",
            started.elapsed().as_secs_f64(),
            lp.wants().draw.len(),
            lp.wants().residency.len(),
            lp.manager().streamer().stats().resident
        );
    }
    let (mut sched, mut upload, mut requests) = (Vec::new(), Vec::new(), 0u64);
    let slow_ms: f64 = arg("--trace-slow-ms").map_or(f64::INFINITY, |v| v.parse().expect("ms"));
    for (k, cam) in script.cameras.iter().take(frames).enumerate() {
        let before = lp.manager().streamer().work();
        let f = lp.begin_frame(&req_of(*cam), &mut sink)?;
        if f.scheduler_ms > slow_ms {
            let w = lp.manager().streamer().work().since(&before);
            println!(
                "  slow frame {k}: scheduler {:.3} ms upload {:.3} ms, work scanned {} updated {} layers {:?}, dispatched {} cancelled {} evicted {} queued {} in_flight {} prefetch {}",
                f.scheduler_ms, f.upload_ms, w.scanned, w.updated, w.layer_updates, f.counts.dispatched, f.counts.cancelled, f.counts.evicted, f.counts.queued, f.counts.in_flight, lp.wants().prefetch.len()
            );
        }
        sched.push(f.scheduler_ms);
        upload.push(f.upload_ms);
        requests += f.counts.dispatched;
        std::thread::sleep(pace);
    }
    let st = lp.manager().streamer().stats();
    println!(
        "{} frames {}: D {} R {} resident {} dispatched {requests} | scheduler ms median {:.3} p99 {:.3} max {:.3} | upload ms median {:.3} p99 {:.3}",
        scene.name(),
        sched.len(),
        lp.wants().draw.len(),
        lp.wants().residency.len(),
        st.resident,
        pct(&mut sched.clone(), 0.5),
        pct(&mut sched.clone(), 0.99),
        pct(&mut sched, 1.0),
        pct(&mut upload.clone(), 0.5),
        pct(&mut upload, 0.99),
    );
    Ok(())
}
