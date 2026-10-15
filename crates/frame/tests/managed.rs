//! The managed loop on fakes (STRM-005, STRM-006, STRM-007, STRM-015, STRM-016, TEST-012): the residency manager and the
//! scheduler's classes driven through the pipelined loop, with R computed on the plan worker. Fake clock, fake I/O (a source
//! that makes meshes without vertices), manual spawners for plans and tiles, a counting sink for the renderer: counts only
//! (CON-20), no GPU, no wall time.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::managed::{FrameCounts, ManagedFrame, ManagedLoop, TileSink};
use planet_frame::pipeline::{DrawKind, PlanField, PlanPipeline, PlanWorker, ResidentRecords};
use planet_frame::prefetch::PrefetchConfig;
use planet_frame::upload::{UploadBudget, DEFAULT_TILE_BYTES_PER_FRAME};
use planet_frame::{
    plan_terrain_field, plan_terrain_threaded, residency_set_field, Camera, LodParams, SelectionCache, TerrainRequest, TerrainView,
};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::TileMesh;
use planet_streaming::{FakeClock, ManualSpawner, ResidencyManager, SlotRef, Spawner, StreamConfig, Streamer, ThreadPool, TileSource};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (160, 90);
const CELLS: u32 = 16;
const FRAME_MS: u64 = 33;

fn lod() -> LodParams {
    LodParams {
        cells: CELLS,
        viewport_h_px: 360.0,
        tau_px: 8.0,
        max_level: 16,
        max_height_m: Reference(1).max_abs_height_m(),
        ..LodParams::earth_1080p()
    }
}

fn position(dir: Vec3, height: f64) -> PlanetFixed {
    PlanetFixed(dir.normalized() * (R + height))
}

fn camera(pos: PlanetFixed, yaw: f64, pitch: f64) -> Camera {
    let hint = Vec3::new(libm::cos(yaw), libm::sin(yaw), 0.37 * libm::sin(2.0 * yaw) + 0.2);
    Camera::at_surface_point(pos, hint, pitch, 60f64.to_radians(), f64::from(SIZE.0) / f64::from(SIZE.1), 0.1)
}

fn request(pos: PlanetFixed, yaw: f64, pitch: f64) -> TerrainRequest {
    TerrainRequest {
        camera: camera(pos, yaw, pitch),
        size: SIZE,
        view: TerrainView::Level,
        lod: lod(),
        face_mapping: TangentWarp.id().to_string(),
        seed: 1,
    }
}

fn here() -> PlanetFixed {
    position(Vec3::new(0.3, 0.5, 0.8), 800.0)
}

fn there() -> PlanetFixed {
    position(Vec3::new(-0.6, -0.2, -0.5), 300.0)
}

/// Meshes without vertices: the key and origin are what the field says, which is what the plan expects of a resident tile.
struct Source {
    lod: LodParams,
}

impl TileSource<TileMesh> for Source {
    fn load(&self, tile: TileId) -> Result<TileMesh, String> {
        let field = Reference(1);
        let rec = ResidentRecords::new(&field, &TangentWarp, &self.lod).record(tile);
        Ok(TileMesh { id: tile, cells: CELLS, key: rec.mesh_key, origin: rec.origin, vertices: Vec::new() })
    }
}

/// Tile jobs that wait until the machine finishes them, `threads` at a time (all done when `run` returns): the completion
/// order differs with the thread count, the results must not.
#[derive(Default)]
struct BatchSpawner {
    queue: std::sync::Mutex<std::collections::VecDeque<planet_streaming::Job>>,
    threads: AtomicUsize,
}

impl BatchSpawner {
    fn run(&self, n: usize) -> usize {
        let jobs: Vec<planet_streaming::Job> = {
            let mut q = self.queue.lock().unwrap();
            let k = n.min(q.len());
            q.drain(..k).collect()
        };
        let count = jobs.len();
        let threads = self.threads.load(Ordering::SeqCst).max(1);
        if threads == 1 {
            jobs.into_iter().for_each(|j| j());
        } else {
            let jobs = std::sync::Mutex::new(jobs.into_iter().map(Some).collect::<Vec<_>>());
            let next = AtomicUsize::new(0);
            std::thread::scope(|s| {
                for _ in 0..threads {
                    s.spawn(|| loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let job = jobs.lock().unwrap().get_mut(i).and_then(Option::take);
                        match job {
                            Some(j) => j(),
                            None => break,
                        }
                    });
                }
            });
        }
        count
    }
}

impl Spawner for BatchSpawner {
    fn spawn(&self, job: planet_streaming::Job) {
        self.queue.lock().unwrap().push_back(job);
    }
}

/// The renderer's side as counts: which tile sits in which slot; a slot never holds two tiles.
#[derive(Default)]
struct CountingSink {
    by_tile: HashMap<TileId, SlotRef>,
    owner: HashMap<SlotRef, TileId>,
    uploads: u64,
    forgets: u64,
    /// Bytes one tile occupies in this sink's format (0: the fake meshes have no vertices, nothing is ever deferred).
    bytes: u64,
    /// Every tile written, in order.
    order: Vec<TileId>,
}

impl TileSink for CountingSink {
    fn upload_bytes(&self, _mesh: &TileMesh) -> u64 {
        self.bytes
    }

    fn upload_at(&mut self, mesh: &TileMesh, slot: SlotRef) -> Result<(), String> {
        if let Some(other) = self.owner.get(&slot) {
            if *other != mesh.id {
                return Err(format!("{slot:?} still holds {other:?} when {:?} is placed in it", mesh.id));
            }
        }
        if let Some(old) = self.by_tile.insert(mesh.id, slot) {
            self.owner.remove(&old);
        }
        self.owner.insert(slot, mesh.id);
        self.uploads += 1;
        self.order.push(mesh.id);
        Ok(())
    }

    fn forget(&mut self, tile: TileId) {
        if let Some(slot) = self.by_tile.remove(&tile) {
            self.owner.remove(&slot);
        }
        self.forgets += 1;
    }
}

struct Rig {
    lp: ManagedLoop,
    plan_spawner: Arc<ManualSpawner>,
    tile_spawner: Arc<BatchSpawner>,
    sink: CountingSink,
    capacity: usize,
    clock: Arc<FakeClock>,
    /// Tile jobs the machine finishes between two frames.
    tile_jobs_per_frame: usize,
}

impl Rig {
    fn new(capacity: usize) -> Rig {
        Rig::build(capacity, false)
    }

    /// With the prefetch source on (STRM-018), the camera path read on a fake clock that advances `FRAME_MS` a frame.
    fn with_prefetch(capacity: usize) -> Rig {
        Rig::build(capacity, true)
    }

    /// A rig whose tiles weigh `tile_bytes` in the sink and whose loop has the upload budget `budget` (STRM-006).
    fn budgeted(capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget) -> Rig {
        Rig::build_with(capacity, prefetch, tile_bytes, budget)
    }

    fn build(capacity: usize, prefetch: bool) -> Rig {
        Rig::build_with(capacity, prefetch, 0, UploadBudget::default())
    }

    fn build_with(capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget) -> Rig {
        let clock = Arc::new(FakeClock::default());
        let plan_spawner = Arc::new(ManualSpawner::default());
        let tile_spawner = Arc::new(BatchSpawner::default());
        let pipe = PlanPipeline::new(PlanWorker::new(plan_spawner.clone(), Arc::new(Reference(1)), 2));
        let config = StreamConfig {
            max_in_flight: 64,
            capacity,
            retire_frames: 2,
            retry_after_ms: 100,
            record_dispatches: true,
            record_changes: true,
            ..Default::default()
        };
        let mut streamer = Streamer::new(Arc::new(Source { lod: lod() }), tile_spawner.clone() as Arc<dyn Spawner>, clock.clone(), config);
        streamer.load_roots_blocking().unwrap();
        let mut lp = ManagedLoop::new(pipe, ResidencyManager::new(streamer)).with_upload_budget(budget);
        if prefetch {
            lp = lp.with_prefetch(PrefetchConfig::default(), R, clock.clone());
        }
        Rig {
            lp,
            plan_spawner,
            tile_spawner,
            sink: CountingSink { bytes: tile_bytes, ..Default::default() },
            capacity,
            clock,
            tile_jobs_per_frame: 10_000,
        }
    }

    /// One iteration, then the workers finish what they have (a plan, the tiles dispatched): the next frame sees the results.
    fn step(&mut self, req: &TerrainRequest) -> ManagedFrame {
        let f = self.begin(req);
        self.finish();
        f
    }

    /// The frame itself; the machine's work between frames is [`Rig::finish`].
    fn begin(&mut self, req: &TerrainRequest) -> ManagedFrame {
        self.clock.advance(FRAME_MS);
        let f = self.lp.begin_frame(req, &mut self.sink).unwrap();
        self.check_mirror();
        f
    }

    fn finish(&mut self) {
        self.plan_spawner.run(4);
        self.tile_spawner.run(self.tile_jobs_per_frame);
    }

    /// The sink holds exactly the streamer's resident tiles, each in the slot the manager chose, no slot twice, and the
    /// pool's accounting adds up (STRM-006, STRM-007).
    fn check_mirror(&self) {
        let streamer = self.lp.manager().streamer();
        let st = streamer.stats();
        // The sink holds the streamer's resident tiles except those the upload budget defers (STRM-006): they wait, never dropped.
        assert_eq!(
            self.sink.by_tile.len() + self.lp.pending().len(),
            st.resident,
            "the sink holds the resident tiles but the deferred ones"
        );
        assert_eq!(self.sink.owner.len(), self.sink.by_tile.len(), "no slot holds two tiles");
        for (tile, slot) in &self.sink.by_tile {
            assert!(streamer.is_resident(*tile), "{tile:?} left the streamer but not the sink");
            assert_eq!(streamer.slot_of(*tile), Some(*slot), "{tile:?} sits where the manager put it");
            assert_eq!(self.lp.slot_of(*tile), Some(*slot));
        }
        assert_eq!(st.main_in_use + st.main_retiring + st.main_free, self.capacity, "slots: in use + retiring + free = capacity");
        assert!(st.main_in_use <= self.capacity);
    }

    /// Frames at `req` until the frame is the synchronous frame and nothing is queued or running (R may not fit).
    fn settle_view(&mut self, req: &TerrainRequest) {
        for k in 0..600 {
            let f = self.step(req);
            if f.counts.queued == 0 && f.counts.in_flight == 0 && f.pipeline.draw.kind == DrawKind::Exact {
                return;
            }
            assert!(k < 599, "did not settle: {:?}", f.counts);
        }
    }

    /// Frames at `req` until nothing is queued or running and every tile of R is resident.
    fn settle(&mut self, req: &TerrainRequest) {
        for k in 0..600 {
            let f = self.step(req);
            let quiet = f.counts.queued == 0 && f.counts.in_flight == 0;
            let r = self.lp.wants().residency.clone();
            if quiet
                && !r.is_empty()
                && r.iter().all(|(t, _)| self.lp.resident().contains_key(t))
                && f.pipeline.draw.kind == DrawKind::Exact
            {
                return;
            }
            assert!(k < 599, "did not settle: {:?}", f.counts);
        }
    }
}

fn closure(tiles: impl IntoIterator<Item = TileId>) -> BTreeSet<TileId> {
    let mut out = BTreeSet::new();
    for t in tiles {
        let mut x = Some(t);
        while let Some(a) = x {
            out.insert(a);
            x = a.parent();
        }
    }
    out
}

// spec: STRM-005, STRM-015, STRM-016, STRM-017
#[test]
fn a_full_turn_through_the_loop_queues_dispatches_and_cancels_nothing_once_r_is_resident() {
    let mut rig = Rig::new(1_000_000);
    let first = request(here(), 0.0, -0.4);
    rig.settle(&first);
    let r_len = rig.lp.wants().residency.len();
    assert!(r_len > 200 && rig.lp.wants().draw.len() < r_len, "the scene must be non-trivial: D {} R {r_len}", rig.lp.wants().draw.len());
    let before = rig.lp.pipeline().worker().counts();
    let mut draw_sets = BTreeSet::new();
    let mut plans_seen = 0;
    for k in 0..36 {
        let (yaw, pitch) = (f64::from(k) * 10f64.to_radians(), [-1.0, -0.4, 0.2][k as usize % 3]);
        let req = request(here(), yaw, pitch);
        // Three frames per direction: the frame that asks for the plan, the one that receives it, the settled one.
        for i in 0..3 {
            let f = rig.step(&req);
            plans_seen += usize::from(f.pipeline.delivered.is_some());
            let c = f.counts;
            let ctx = format!("yaw {yaw} frame {i}");
            assert_eq!(
                (c.dispatched, c.cancelled, c.queued, c.in_flight, c.blocked),
                (0, 0, 0, 0, 0),
                "{ctx}: a rotation must not request: {c:?}"
            );
            assert_eq!((c.uploaded, c.evicted, c.degraded), (0, 0, 0), "{ctx}: a rotation must not upload or evict: {c:?}");
            assert_eq!(f.pipeline.draw.blank, 0, "{ctx}");
            if i == 2 {
                assert_eq!(f.pipeline.draw.kind, DrawKind::Exact, "{ctx}: the frame is the synchronous frame");
                assert_eq!(f.pipeline.draw.plan, plan_terrain_field(&req, &Reference(1)).unwrap(), "{ctx}");
                draw_sets.insert(f.pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect::<BTreeSet<_>>());
            }
        }
    }
    assert!(draw_sets.len() > 5, "the turn must change the draw set, got {} distinct", draw_sets.len());
    let after = rig.lp.pipeline().worker().counts();
    assert!(plans_seen >= 36 && after.completed - before.completed >= 36, "a plan per direction came from the worker");
    // R was selected once; every turn reused it (no selection, no field evaluation for it).
    assert_eq!(after.residencies, before.residencies, "the turn selected no residency set");
    assert!(after.residencies_reused - before.residencies_reused >= 36);
    assert_eq!(rig.sink.forgets, 0);
}

// spec: STRM-005, STRM-002, STRM-015, STRM-017
#[test]
fn after_a_teleport_through_the_loop_the_draw_set_is_requested_before_the_rest_of_r_and_no_frame_is_blank() {
    let mut rig = Rig::new(1_000_000);
    rig.settle(&request(here(), 0.0, -0.4));
    let log_before = rig.lp.manager().streamer().dispatch_log.len();
    let b = request(there(), 1.0, -0.3);
    let f0 = rig.step(&b);
    assert_eq!(f0.pipeline.draw.kind, DrawKind::Cover, "the plan of A is no use at B");
    assert_eq!(f0.pipeline.draw.blank, 0);
    assert!(!f0.pipeline.draw.plan.nodes.is_empty());
    for _ in 0..3 {
        let f = rig.step(&b);
        assert_eq!(f.pipeline.draw.blank, 0, "never blank while the fill runs");
    }
    rig.settle(&b);
    let order = &rig.lp.manager().streamer().dispatch_log[log_before..];
    let d = closure(plan_terrain_field(&b, &Reference(1)).unwrap().nodes.iter().map(|n| n.tile));
    let new_draw = order.iter().filter(|t| d.contains(t)).count();
    assert!(new_draw > 20, "the jump must need new draw tiles, got {new_draw}");
    let last_draw = order.iter().rposition(|t| d.contains(t)).unwrap();
    let first_rest = order.iter().position(|t| !d.contains(t)).expect("R is larger than D");
    assert!(last_draw < first_rest, "a tile of the rest of R (#{first_rest}) was requested before a draw tile (#{last_draw})");
    let pos: HashMap<TileId, usize> = order.iter().enumerate().map(|(i, t)| (*t, i)).collect();
    for (t, i) in &pos {
        if let Some(a) = t.parent().and_then(|a| pos.get(&a)) {
            assert!(a < i, "{t:?} requested before its parent");
        }
    }
}

// spec: STRM-006, STRM-007, STRM-005, STRM-017
#[test]
fn with_a_share_smaller_than_r_every_frame_stays_inside_the_slots_and_the_view_stays_exact() {
    let field = Reference(1);
    let yaws: Vec<(f64, f64)> = (0..8).map(|k| (f64::from(k) * 45f64.to_radians(), [-1.0, -0.4, 0.2][k as usize % 3])).collect();
    let widest = yaws
        .iter()
        .map(|&(yaw, pitch)| closure(plan_terrain_field(&request(here(), yaw, pitch), &field).unwrap().nodes.iter().map(|n| n.tile)).len())
        .max()
        .unwrap();
    let r_full =
        closure(residency_set_field(&TangentWarp, here(), &field, &lod(), &mut SelectionCache::new()).tiles.iter().map(|e| e.tile)).len();
    // Room for the widest view, the roots and a little more, far below R.
    let capacity = widest + 6 + 20;
    assert!(capacity * 2 < r_full, "the share must be much smaller than R: {capacity} slots, R {r_full}");
    let mut rig = Rig::new(capacity);
    let (mut degraded_frames, mut evicted) = (0, 0);
    let mut exact = 0;
    for round in 0..2 {
        for &(yaw, pitch) in &yaws {
            let req = request(here(), yaw, pitch);
            // Frames at this view until its draw set is complete (the fill is 16 jobs a frame), every frame checked.
            for i in 0..400 {
                let f = rig.step(&req);
                let c: FrameCounts = f.counts;
                assert!(c.slots_in_use <= capacity, "frame over the share: {c:?}");
                assert!(c.resident <= capacity, "resident {} over the share {capacity}", c.resident);
                assert!(!c.budget_failure && c.draw_replaced == 0, "the draw set fits: {c:?}");
                assert_eq!(f.pipeline.draw.blank, 0);
                degraded_frames += usize::from(c.degraded > 0);
                evicted += c.evicted;
                if f.pipeline.draw.kind == DrawKind::Exact && c.queued == 0 && c.in_flight == 0 {
                    // Settled for this view: full detail, the synchronous frame.
                    assert_eq!(f.pipeline.draw.plan, plan_terrain_field(&req, &field).unwrap(), "round {round} yaw {yaw}");
                    exact += 1;
                    break;
                }
                assert!(i < 399, "round {round} yaw {yaw}: D did not complete within the share: {c:?}");
            }
        }
    }
    assert_eq!(exact, 16);
    assert!(degraded_frames > 0, "R does not fit: the surroundings must be degraded");
    assert!(evicted > 0, "turning with a small share evicts");
}

// spec: STRM-006, STRM-007, STRM-017
#[test]
fn a_walk_with_eviction_keeps_every_frame_inside_the_slots_and_the_sink_never_sees_a_slot_twice() {
    // The sink fails an upload into a slot that still holds a tile and check_mirror every frame: a long walk with a share
    // that forces eviction exercises both.
    let field = Reference(1);
    let widest = closure(plan_terrain_field(&request(here(), 0.0, -0.4), &field).unwrap().nodes.iter().map(|n| n.tile)).len();
    let capacity = widest * 2 + 10;
    let mut rig = Rig::new(capacity);
    let start = Vec3::new(0.3, 0.5, 0.8);
    for k in 0..40 {
        // A slow walk: 200 m per step.
        let pos = position(start, 800.0 + 200.0 * f64::from(k));
        let req = request(pos, 0.3, -0.4);
        for _ in 0..4 {
            let f = rig.step(&req);
            assert!(f.counts.slots_in_use <= capacity, "{:?}", f.counts);
            assert_eq!(f.pipeline.draw.blank, 0);
        }
    }
    rig.settle_view(&request(position(start, 800.0 + 200.0 * 39.0), 0.3, -0.4));
}

#[derive(Default, Clone)]
struct Counts {
    heights: Arc<AtomicUsize>,
    bounds: Arc<AtomicUsize>,
}

/// The reference field, counting every height and every height-bound query.
struct CountingField(Counts);

impl HeightField for CountingField {
    fn height(&self, dir: Vec3) -> f64 {
        self.0.heights.fetch_add(1, Ordering::SeqCst);
        Reference(1).height(dir)
    }
    fn seed(&self) -> u64 {
        1
    }
    fn implementation_id(&self) -> &'static str {
        Reference(1).implementation_id()
    }
    fn version(&self) -> u32 {
        Reference(1).version()
    }
    fn max_abs_height_m(&self) -> f64 {
        Reference(1).max_abs_height_m()
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        self.0.bounds.fetch_add(1, Ordering::SeqCst);
        Reference(1).height_bounds(centre, angular_radius)
    }
}

// spec: STRM-015, STRM-016, LOD-009
#[test]
fn r_costs_the_plans_cache_no_origin_height_and_a_repeat_or_a_rotation_costs_nothing() {
    let counts = Counts::default();
    let field = CountingField(counts.clone());
    let read = || (counts.heights.load(Ordering::SeqCst), counts.bounds.load(Ordering::SeqCst));
    let a = request(here(), 0.0, -0.4);
    let mut cache = SelectionCache::new();
    plan_terrain_threaded(&a, &field, &mut cache, 1).unwrap();
    let (h_plan, b_plan) = read();
    assert!(h_plan > 0 && b_plan > 0);
    // R on the plan's cache: the tiles the plan visited are hits; R needs no origin height of any tile.
    let r = PlanField::residency(&field, &a, &mut cache).unwrap();
    let (h_r, b_r) = read();
    assert_eq!(h_r, h_plan, "R evaluates no origin height: a resident tile carries its own origin");
    assert!(b_r > b_plan, "R adds the volumes of the tiles outside the frustum");
    // The same position again, and a turn at the same position: nothing is evaluated.
    assert_eq!(PlanField::residency(&field, &a, &mut cache).unwrap(), r);
    let turned = request(here(), 2.0, 0.1);
    assert_eq!(PlanField::residency(&field, &turned, &mut cache).unwrap(), r, "R does not depend on the view direction");
    assert_eq!(read(), (h_r, b_r), "a repeat or a rotation evaluates the field zero times");
    // The plan of the turned view finds every volume it needs already in the cache; only its new origin heights are evaluated.
    plan_terrain_threaded(&turned, &field, &mut cache, 1).unwrap();
    let (h_turn, b_turn) = read();
    assert_eq!(b_turn, b_r, "R's volumes serve the plan of every view from the position");
    assert!(h_turn >= h_r);
}

// spec: STRM-015, CON-14
#[test]
fn the_worker_makes_the_same_r_on_any_thread_count_and_hands_a_rotation_the_same_set() {
    let reqs = [request(here(), 0.0, -0.4), request(here(), 2.0, 0.1), request(there(), 1.0, -0.3)];
    let direct: Vec<_> = reqs
        .iter()
        .map(|q| residency_set_field(&TangentWarp, q.camera.position, &Reference(1), &q.lod, &mut SelectionCache::new()))
        .collect();
    for threads in [1usize, 2, 8] {
        let w = PlanWorker::new(Arc::new(ThreadPool::new(threads)), Arc::new(Reference(1)), threads);
        let mut sets = Vec::new();
        for (i, q) in reqs.iter().enumerate() {
            w.submit(i as u64 + 1, q.clone());
            let o = loop {
                if let Some(r) = w.take_residency() {
                    break r.unwrap();
                }
                std::thread::yield_now();
            };
            assert_eq!(o.seq, i as u64 + 1);
            assert_eq!(*o.residency, direct[i], "threads {threads}, request {i}: R is a pure function of the position");
            assert_eq!(o.reused, i == 1, "only the turn at the same position reuses R");
            sets.push(o.residency);
        }
        assert!(Arc::ptr_eq(&sets[0], &sets[1]), "the turn gets the very set");
        assert_eq!(w.counts().residencies, 2);
        assert_eq!(w.counts().residencies_reused, 1);
    }
}

// spec: STRM-017, STRM-015
#[test]
fn the_wants_are_the_delivered_plan_and_r_and_a_rotation_leaves_the_residency_wants_alone() {
    let mut rig = Rig::new(1_000_000);
    let a = request(here(), 0.0, -0.4);
    assert!(rig.lp.wants().draw.is_empty() && rig.lp.wants().residency.is_empty());
    rig.settle(&a);
    let plan = plan_terrain_field(&a, &Reference(1)).unwrap();
    let want_draw: Vec<(TileId, f64)> = plan.nodes.iter().enumerate().map(|(i, n)| (n.tile, i as f64)).collect();
    assert_eq!(rig.lp.wants().draw, want_draw, "D: the plan's nodes in plan order");
    let r = residency_set_field(&TangentWarp, here(), &Reference(1), &lod(), &mut SelectionCache::new());
    assert_eq!(rig.lp.wants().residency, r.wants(), "R: its tiles with their distances");
    let residency_before = rig.lp.wants().residency.clone();
    let b = request(here(), 2.0, 0.1);
    for _ in 0..3 {
        rig.step(&b);
    }
    let plan_b = plan_terrain_field(&b, &Reference(1)).unwrap();
    assert_eq!(rig.lp.wants().draw.iter().map(|w| w.0).collect::<Vec<_>>(), plan_b.nodes.iter().map(|n| n.tile).collect::<Vec<_>>());
    assert_eq!(rig.lp.wants().residency, residency_before, "a plan-only delivery leaves the residency wants alone");
}

// spec: STRM-015, STRM-016, LOD-009
#[test]
fn the_worker_runs_r_after_the_plan_on_the_plans_cache_and_a_rotation_evaluates_no_origin_height_for_it() {
    let counts = Counts::default();
    let spawner = Arc::new(ManualSpawner::default());
    let w = PlanWorker::new(spawner.clone(), Arc::new(CountingField(counts.clone())), 1);
    let read = || (counts.heights.load(Ordering::SeqCst), counts.bounds.load(Ordering::SeqCst));
    let a = request(here(), 0.0, -0.4);
    w.submit(1, a.clone());
    spawner.run(1);
    // One job delivered the plan and then R; the plan evaluated the same origin heights as a plan alone.
    let alone = Counts::default();
    plan_terrain_threaded(&a, &CountingField(alone.clone()), &mut SelectionCache::new(), 1).unwrap();
    let (h, b) = read();
    assert_eq!(h, alone.heights.load(Ordering::SeqCst), "R added no origin height");
    assert!(b > alone.bounds.load(Ordering::SeqCst), "R added the volumes outside the frustum");
    assert!(w.take().unwrap().is_ok() && w.take_residency().unwrap().is_ok());
    // A rotation: the plan is new, R is the same set and costs the field nothing.
    w.submit(2, request(here(), 2.0, 0.1));
    spawner.run(1);
    let o = w.take_residency().unwrap().unwrap();
    assert!(o.reused);
    assert_eq!(read().1, b, "the turned plan and its reused R found every volume in the cache");
}

// ---- prefetch along travel (STRM-018) ----

/// A camera on a straight ground track: `metres` along the east-ish tangent from the start, `height` above the sphere.
fn track(metres: f64, height: f64) -> PlanetFixed {
    let start = Vec3::new(0.3, 0.5, 0.8).normalized();
    let east = start.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
    position((start + east * (metres / R)).normalized(), height)
}

/// The dive script of ADR 0032 on fakes, shortened: a geometric descent over one spot from 20 km to 200 m, then a ground
/// segment at 600 m/s along a straight track (33 ms frames: 20 m a frame). The camera looks the same way throughout.
fn dive_position(k: usize) -> PlanetFixed {
    const DESCENT: usize = 45;
    if k < DESCENT {
        track(0.0, 20_000.0 * libm::pow(200.0 / 20_000.0, k as f64 / DESCENT as f64))
    } else {
        track((k - DESCENT) as f64 * 20.0, 200.0)
    }
}

#[derive(Debug, Default)]
struct Measure {
    /// Distinct draw-set tiles that were not resident when they first appeared in the wants ("missing visible").
    missing_visible: usize,
    /// ... and those that were already resident then.
    resident_on_first_sight: usize,
    prefetch_frames: usize,
    cancelled_prefetch: u64,
    blank: usize,
}

/// Run `frames` of `script` from where the rig stands; the machine finishes `rig.tile_jobs_per_frame` tile jobs a frame.
fn measure(rig: &mut Rig, frames: usize, script: &dyn Fn(usize) -> PlanetFixed) -> Measure {
    let mut seen: BTreeSet<TileId> = BTreeSet::new();
    let mut m = Measure::default();
    for k in 0..frames {
        for (t, _) in rig.lp.wants().draw.clone() {
            if seen.insert(t) {
                if rig.lp.resident().contains_key(&t) {
                    m.resident_on_first_sight += 1;
                } else {
                    m.missing_visible += 1;
                }
            }
        }
        let f = rig.begin(&request(script(k), 0.3, -0.4));
        m.prefetch_frames += usize::from(f.counts.prefetch > 0);
        m.cancelled_prefetch += f.counts.cancelled_prefetch;
        m.blank += f.pipeline.draw.blank;
        rig.finish();
    }
    m
}

// spec: STRM-018, STRM-005
#[test]
fn a_camera_that_only_rotates_prefetches_nothing_and_requests_nothing() {
    let mut rig = Rig::with_prefetch(1_000_000);
    rig.settle(&request(here(), 0.0, -0.4));
    // Still for longer than the estimator's window.
    for _ in 0..20 {
        rig.step(&request(here(), 0.0, -0.4));
    }
    for k in 0..36 {
        let req = request(here(), f64::from(k) * 10f64.to_radians(), [-1.0, -0.4, 0.2][k as usize % 3]);
        for i in 0..3 {
            let f = rig.step(&req);
            let c = f.counts;
            assert_eq!(
                (c.prefetch, c.cancelled_prefetch, c.dispatched, c.cancelled, c.queued),
                (0, 0, 0, 0, 0),
                "yaw {k} frame {i}: {c:?}"
            );
        }
        assert!(rig.lp.wants().prefetch.is_empty());
        assert_eq!(rig.lp.travel().unwrap().velocity().map(|v| v.length()), Some(0.0), "a rotation is no velocity");
        assert!(rig.lp.travel().unwrap().predict().is_none());
    }
}

// spec: STRM-018, STRM-005
#[test]
fn straight_travel_prefetches_ahead_and_fewer_draw_tiles_have_to_be_requested_as_missing() {
    let mut on = Rig::with_prefetch(1_000_000);
    let mut off = Rig::new(1_000_000);
    let ground = |k: usize| track(k as f64 * 20.0, 200.0);
    for rig in [&mut on, &mut off] {
        rig.settle(&request(ground(0), 0.3, -0.4));
        rig.tile_jobs_per_frame = 24;
    }
    let (a, b) = (measure(&mut on, 80, &ground), measure(&mut off, 80, &ground));
    eprintln!("straight travel, 80 frames, 24 tile jobs a frame\n  prefetch on : {a:?}\n  prefetch off: {b:?}");
    assert_eq!((a.blank, b.blank), (0, 0));
    assert_eq!(b.prefetch_frames, 0, "no prefetch source: no prefetch wants");
    assert!(a.prefetch_frames > 20, "travel prefetches on most frames: {a:?}");
    assert!(b.missing_visible > 10, "the script must make tiles enter the draw set: {b:?}");
    assert!(a.missing_visible * 2 <= b.missing_visible, "prefetch must at least halve the draw tiles requested as missing: {a:?} vs {b:?}");
    assert!(a.resident_on_first_sight > b.resident_on_first_sight, "tiles are resident before they are drawn: {a:?} vs {b:?}");
}

// spec: STRM-018, STRM-017
#[test]
fn on_the_dive_script_prefetch_never_makes_the_lag_worse_and_no_frame_is_blank() {
    let (mut on, mut off) = (Rig::with_prefetch(1_000_000), Rig::new(1_000_000));
    for rig in [&mut on, &mut off] {
        rig.settle(&request(dive_position(0), 0.3, -0.4));
        rig.tile_jobs_per_frame = 24;
    }
    let (a, b) = (measure(&mut on, 105, &dive_position), measure(&mut off, 105, &dive_position));
    eprintln!("dive script (45 descent + 60 ground frames), 24 tile jobs a frame\n  prefetch on : {a:?}\n  prefetch off: {b:?}");
    assert_eq!((a.blank, b.blank), (0, 0));
    assert!(a.prefetch_frames > 10, "{a:?}");
    assert!(
        a.missing_visible * 2 <= b.missing_visible,
        "prefetch must at least halve the missing visible tiles on the dive script: {a:?} vs {b:?}"
    );
}

// spec: STRM-018, STRM-005, STRM-006
#[test]
fn prefetch_takes_free_slots_only_and_a_full_share_makes_no_prefetch_request() {
    let field = Reference(1);
    let widest = closure(plan_terrain_field(&request(track(0.0, 200.0), 0.3, -0.4), &field).unwrap().nodes.iter().map(|n| n.tile)).len();
    let capacity = widest * 2 + 10;
    let mut rig = Rig::with_prefetch(capacity);
    rig.tile_jobs_per_frame = 24;
    let (mut full_frames, mut prefetch_frames) = (0, 0);
    for k in 0..90 {
        let f = rig.begin(&request(track(k as f64 * 20.0, 200.0), 0.3, -0.4));
        let c = f.counts;
        assert!(c.slots_in_use <= capacity && c.resident <= capacity, "over the share: {c:?}");
        assert!(!c.budget_failure && f.pipeline.draw.blank == 0, "{c:?}");
        let free = rig.lp.manager().streamer().stats().main_free;
        let missing = rig.lp.wants().prefetch.iter().filter(|(t, _)| !rig.lp.resident().contains_key(t)).count();
        assert!(missing <= free, "frame {k}: {missing} prefetch tiles wanted but only {free} slots are free");
        full_frames += usize::from(free == 0);
        prefetch_frames += usize::from(missing > 0);
        if free == 0 {
            assert_eq!(missing, 0, "frame {k}: a full share asks for no prefetch");
        }
        rig.finish();
    }
    assert!(prefetch_frames > 0, "the share must have room at times, or the test proves nothing");
    assert!(full_frames > 0, "the share must be full at times, or the test proves nothing");
}

// spec: STRM-018, STRM-005
#[test]
fn a_change_of_direction_cancels_the_prefetch_requests_that_left_the_set() {
    let mut rig = Rig::with_prefetch(1_000_000);
    rig.settle(&request(track(0.0, 200.0), 0.3, -0.4));
    // A slow machine keeps prefetch requests queued: 2 tile jobs a frame.
    rig.tile_jobs_per_frame = 2;
    let (mut queued_prefetch, mut cancelled_out, mut cancelled_back) = (0usize, 0u64, 0u64);
    for k in 0..40 {
        let f = rig.begin(&request(track(k as f64 * 20.0, 200.0), 0.3, -0.4));
        queued_prefetch = queued_prefetch.max(rig.lp.manager().streamer().stats().queued_by_class[2]);
        cancelled_out += f.counts.cancelled_prefetch;
        rig.finish();
    }
    assert!(queued_prefetch > 0, "prefetch requests must have been waiting");
    // Turn round and go back: the set ahead is now behind, and its queued requests are cancelled.
    for k in 0..40 {
        let f = rig.begin(&request(track((39 - k) as f64 * 20.0, 200.0), 0.3, -0.4));
        cancelled_back += f.counts.cancelled_prefetch;
        rig.finish();
    }
    eprintln!("prefetch requests cancelled: going out {cancelled_out}, after turning round {cancelled_back}");
    assert!(cancelled_back > 0, "turning round must cancel prefetch requests that left the set");
}

// spec: STRM-018, STRM-002
#[test]
fn a_teleport_resets_the_velocity_and_prefetches_nothing_from_the_old_direction() {
    let mut rig = Rig::with_prefetch(1_000_000);
    rig.settle(&request(track(0.0, 200.0), 0.3, -0.4));
    for k in 0..30 {
        rig.step(&request(track(k as f64 * 20.0, 200.0), 0.3, -0.4));
    }
    assert!(rig.lp.travel().unwrap().predict().is_some(), "moving: a position is predicted");
    assert!(!rig.lp.wants().prefetch.is_empty() || rig.lp.manager().streamer().stats().resident > 0);
    let b = request(there(), 1.0, -0.3);
    let f = rig.step(&b);
    assert_eq!(rig.lp.travel().unwrap().teleports(), 1);
    assert!(rig.lp.travel().unwrap().predict().is_none(), "the old velocity is gone");
    assert_eq!(f.pipeline.draw.blank, 0);
    // The prefetch of the old direction is replaced as soon as the new worker result is in (a plan and an R later).
    for _ in 0..3 {
        rig.step(&b);
    }
    assert!(rig.lp.wants().prefetch.is_empty(), "no prefetch from the old direction at the new place");
    rig.settle(&b);
    assert!(rig.lp.wants().prefetch.is_empty());
}

// spec: STRM-018, CON-14
#[test]
fn the_prefetch_set_is_the_same_on_any_thread_count_and_is_r_ahead_minus_r_here() {
    use planet_frame::prefetch::prefetch_set;
    let req = request(track(0.0, 200.0), 0.3, -0.4);
    let ahead_pos = track(1500.0, 200.0);
    let direct = {
        let mut cache = SelectionCache::new();
        let here_set = residency_set_field(&TangentWarp, req.camera.position, &Reference(1), &lod(), &mut cache);
        let ahead = residency_set_field(&TangentWarp, ahead_pos, &Reference(1), &lod(), &mut cache);
        prefetch_set(&here_set, &ahead)
    };
    assert!(direct.len() > 10, "the script must have something ahead: {}", direct.len());
    let here_set = residency_set_field(&TangentWarp, req.camera.position, &Reference(1), &lod(), &mut SelectionCache::new());
    for (t, _) in &direct {
        assert!(!here_set.contains(*t), "{t:?} is already in R");
    }
    for w in direct.windows(2) {
        assert!((w[0].0.level(), w[0].1) <= (w[1].0.level(), w[1].1), "coarse first, then nearer: {w:?}");
    }
    for threads in [1usize, 2, 8] {
        let w = PlanWorker::new(Arc::new(ThreadPool::new(threads)), Arc::new(Reference(1)), threads);
        w.submit_predicted(1, req.clone(), Some(ahead_pos));
        let o = loop {
            if let Some(r) = w.take_residency() {
                break r.unwrap();
            }
            std::thread::yield_now();
        };
        assert_eq!(*o.prefetch, direct, "threads {threads}");
    }
}

// spec: STRM-018, STRM-016, CON-22
#[test]
fn the_prefetch_set_is_computed_on_the_worker_and_never_on_the_frame() {
    let counts = Counts::default();
    let spawner = Arc::new(ManualSpawner::default());
    let mut pipe = PlanPipeline::new(PlanWorker::new(spawner.clone(), Arc::new(CountingField(counts.clone())), 1));
    let read = || (counts.heights.load(Ordering::SeqCst), counts.bounds.load(Ordering::SeqCst));
    let req = request(track(0.0, 200.0), 0.3, -0.4);
    let none = HashMap::new();
    pipe.begin_frame_predicted(&req, Some(track(1500.0, 200.0)), &none).unwrap();
    assert_eq!(read(), (0, 0), "the frame evaluated nothing: the set is the worker's");
    assert!(pipe.residency().is_none());
    spawner.run(1);
    pipe.begin_frame_predicted(&req, Some(track(1500.0, 200.0)), &none).unwrap();
    assert!(read().1 > 0, "the worker evaluated the volumes");
    assert!(!pipe.residency().unwrap().prefetch.is_empty(), "the set arrived with R");
    // An unchanged prediction asks for nothing again; a new one (the camera stopped) asks again and empties the set.
    assert!(!pipe.begin_frame_predicted(&req, Some(track(1500.0, 200.0)), &none).unwrap().requested);
    assert!(pipe.begin_frame_predicted(&req, None, &none).unwrap().requested);
    spawner.run(1);
    pipe.begin_frame_predicted(&req, None, &none).unwrap();
    assert!(pipe.residency().unwrap().prefetch.is_empty());
}

// ---- the per-frame upload budget (STRM-006 upload cap) ----

/// Bytes of one vertex-format tile at cells 32 (what the renderer uploads today).
const VERTEX_TILE_BYTES: u64 = 53_724;

/// Rank of a tile in the upload order, by the wants the frame started with, worked out here from first principles: the draw
/// set with its ancestors and the roots, then R with its ancestors, then the prefetch set, then the rest; coarse before fine.
fn upload_ranker(wants: &planet_streaming::Wants) -> impl Fn(TileId) -> (u8, u8) {
    let d = closure(wants.draw.iter().map(|(t, _)| *t));
    let r = closure(wants.residency.iter().map(|(t, _)| *t));
    let p = closure(wants.prefetch.iter().map(|(t, _)| *t));
    move |tile: TileId| {
        let class = if d.contains(&tile) || tile.level() == 0 {
            0
        } else if r.contains(&tile) {
            1
        } else if p.contains(&tile) {
            2
        } else {
            3
        };
        (class, tile.level())
    }
}

/// What the frame did, for comparing runs.
#[derive(Debug, PartialEq)]
struct UploadFrame {
    written: Vec<TileId>,
    bytes: u64,
    deferred: usize,
    resident: usize,
}

/// Run `frames` of `script` and check every frame against the budget's rules; return the frames.
fn run_budgeted(rig: &mut Rig, frames: usize, script: &dyn Fn(usize) -> PlanetFixed, check_blank: bool) -> Vec<UploadFrame> {
    let cap = rig.lp.upload_budget().tile_bytes_per_frame;
    let bytes = rig.sink.bytes;
    let mut out = Vec::new();
    for k in 0..frames {
        let wants = rig.lp.wants().clone();
        let before = rig.sink.order.len();
        let f = rig.begin(&request(script(k), 0.3, -0.4));
        let c = f.counts;
        let written: Vec<TileId> = rig.sink.order[before..].to_vec();
        assert_eq!(c.uploaded as usize, written.len());
        assert_eq!(c.upload_bytes, written.len() as u64 * bytes, "frame {k}: the counts report what was written");
        // The cap is exceeded only by the one-tile rule; a frame that defers is full (nothing more fits).
        assert!(c.upload_bytes <= cap || c.uploaded == 1, "frame {k}: {} bytes in {} tiles over the cap {cap}", c.upload_bytes, c.uploaded);
        if c.deferred > 0 {
            assert!(c.uploaded >= 1, "frame {k}: tiles wait but none was written (starvation)");
            assert!(
                c.upload_bytes + bytes > cap,
                "frame {k}: deferred {} tiles though {} of {cap} bytes were used",
                c.deferred,
                c.upload_bytes
            );
        }
        // Class order: the written tiles are in rank order, and nothing that waits ranks before the last one written.
        let rank = upload_ranker(&wants);
        let ranks: Vec<_> = written.iter().map(|t| rank(*t)).collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "frame {k}: written out of order: {ranks:?}");
        if let Some(last) = ranks.last() {
            for p in rig.lp.pending() {
                assert!(rank(p) >= *last, "frame {k}: {p:?} waits though a later tile ({last:?}) was written");
            }
        }
        // Nothing is dropped: the streamer's resident tiles are the sink's plus the waiting ones.
        let st = rig.lp.manager().streamer().stats();
        assert_eq!(rig.sink.by_tile.len() + rig.lp.pending().len(), st.resident, "frame {k}");
        assert_eq!(c.deferred, rig.lp.pending().len());
        if check_blank {
            assert_eq!(f.pipeline.draw.blank, 0, "frame {k}: blank while tiles wait");
        }
        out.push(UploadFrame { written, bytes: c.upload_bytes, deferred: c.deferred, resident: st.resident });
        rig.finish();
    }
    out
}

// spec: STRM-006, STRM-017, STRM-002
#[test]
fn on_the_dive_and_a_teleport_the_upload_cap_holds_in_every_frame_in_class_order_and_no_frame_is_blank() {
    let mut rig = Rig::budgeted(1_000_000, true, VERTEX_TILE_BYTES, UploadBudget::default());
    let cap = DEFAULT_TILE_BYTES_PER_FRAME;
    assert_eq!(cap, 1 << 20);
    assert_eq!(VERTEX_TILE_BYTES, planet_generators::mesh::vertex_count(32) as u64 * planet_generators::mesh::MeshVertex::STRIDE);
    let c = FrameCounts { upload_bytes: 7, deferred: 8, node_bytes: 9, uploaded: 3, ..Default::default() };
    assert_eq!(FrameCounts::NAMES.len(), c.values().len());
    assert_eq!(&c.values()[7..], &[3, 0, 7, 8, 9]);
    assert_eq!(&FrameCounts::NAMES[9..], &["upload_bytes", "deferred", "node_bytes"]);
    let dive = run_budgeted(&mut rig, 105, &dive_position, true);
    // The teleport: the dive's last place to the other side of the planet, then hold.
    let b = there();
    let teleport = run_budgeted(&mut rig, 80, &|_| b, true);
    let deferring = dive.iter().chain(&teleport).filter(|f| f.deferred > 0).count();
    let peak = dive.iter().chain(&teleport).map(|f| f.bytes).max().unwrap();
    eprintln!("dive+teleport: {deferring} frames deferred tiles, peak {peak} B of {cap} B");
    assert!(deferring > 5, "the scripts must hit the cap, or the test proves nothing: {deferring}");
    assert!(peak <= cap && peak > cap - VERTEX_TILE_BYTES, "full frames fill the cap to within one tile: {peak}");
    // The teleport fill drains, and once it has the frame is the synchronous frame.
    let mut last = None;
    for _ in 0..200 {
        let f = rig.step(&request(b, 0.3, -0.4));
        if rig.lp.pending().is_empty() && f.pipeline.draw.kind == DrawKind::Exact && f.counts.queued == 0 && f.counts.in_flight == 0 {
            last = Some(f);
            break;
        }
    }
    let f = last.expect("the fill did not drain");
    assert_eq!(f.pipeline.draw.plan, plan_terrain_field(&request(b, 0.3, -0.4), &Reference(1)).unwrap());
    assert_eq!(f.counts.node_bytes, planet_frame::draw::indirect_frame_bytes(f.pipeline.draw.plan.nodes.len()));
}

// spec: STRM-006, STRM-002
#[test]
fn a_tile_larger_than_the_cap_is_still_written_one_a_frame_and_nothing_starves() {
    let mut rig =
        Rig::budgeted(1_000_000, false, VERTEX_TILE_BYTES, UploadBudget { tile_bytes_per_frame: 10_000, node_records_in_cap: false });
    // The six roots go first, one per frame; the view is blank only until they are in.
    let frames = run_budgeted(&mut rig, 40, &|_| here(), false);
    assert!(frames.iter().all(|f| f.written.len() <= 1), "one tile a frame at most, though every tile is over the cap");
    assert!(frames.iter().take(30).all(|f| f.written.len() == 1), "and one a frame while any waits: {frames:?}");
    let roots: Vec<_> = frames.iter().take(6).flat_map(|f| f.written.iter()).collect();
    assert!(roots.iter().all(|t| t.level() == 0), "the roots first: {roots:?}");
    let f = rig.step(&request(here(), 0.3, -0.4));
    assert_eq!(f.pipeline.draw.blank, 0, "never blank once the roots are in");
}

// spec: STRM-006
#[test]
fn node_records_are_their_own_line_and_count_inside_the_cap_only_when_asked() {
    let own = UploadBudget { tile_bytes_per_frame: 400_000, node_records_in_cap: false };
    let inside = UploadBudget { node_records_in_cap: true, ..own };
    let mut rigs = [Rig::budgeted(1_000_000, false, VERTEX_TILE_BYTES, own), Rig::budgeted(1_000_000, false, VERTEX_TILE_BYTES, inside)];
    let mut prev = [0u64; 2];
    let mut shrunk = false;
    for k in 0..60 {
        for (i, rig) in rigs.iter_mut().enumerate() {
            let f = rig.begin(&request(dive_position(k), 0.3, -0.4));
            let c = f.counts;
            let allowance = rig.lp.upload_budget().allowance(prev[i]);
            assert!(c.upload_bytes <= allowance || c.uploaded == 1, "run {i} frame {k}: {} over {allowance}", c.upload_bytes);
            assert_eq!(c.node_bytes, planet_frame::draw::indirect_frame_bytes(f.pipeline.draw.plan.nodes.len()));
            if i == 0 {
                assert_eq!(allowance, 400_000, "the node records are not in this cap");
            } else {
                shrunk |= allowance < 400_000 && c.node_bytes > 0;
            }
            prev[i] = c.node_bytes;
            rig.finish();
        }
    }
    assert!(shrunk, "the node records must reduce the tile allowance when they are counted inside");
}

// spec: STRM-006, CON-14
#[test]
fn the_upload_order_and_what_is_written_each_frame_are_the_same_on_1_2_and_8_threads() {
    let script = |k: usize| if k < 35 { dive_position(k) } else { there() };
    let mut runs = Vec::new();
    for threads in [1usize, 2, 8] {
        let mut rig =
            Rig::budgeted(1_000_000, true, VERTEX_TILE_BYTES, UploadBudget { tile_bytes_per_frame: 300_000, node_records_in_cap: false });
        rig.tile_spawner.threads.store(threads, Ordering::SeqCst);
        runs.push(run_budgeted(&mut rig, 60, &script, true));
    }
    assert!(runs[0].iter().any(|f| f.deferred > 0));
    assert_eq!(runs[0], runs[1], "1 vs 2 threads");
    assert_eq!(runs[0], runs[2], "1 vs 8 threads");
}

// spec: STRM-006, STRM-017
#[test]
fn a_full_fill_at_the_cap_takes_the_frames_the_cap_allows_and_defers_only_full_frames() {
    // Ground view from a cold start, every tile finishing within the frame it is asked for: the cap alone paces the fill.
    let mut rig = Rig::budgeted(1_000_000, false, VERTEX_TILE_BYTES, UploadBudget::default());
    let ground = track(0.0, 2.0);
    let frames = run_budgeted(&mut rig, 120, &|_| ground, true);
    let total: u64 = frames.iter().map(|f| f.written.len() as u64).sum();
    let per_frame = (DEFAULT_TILE_BYTES_PER_FRAME / VERTEX_TILE_BYTES) as usize;
    let busy = frames.iter().filter(|f| f.bytes > 0).count();
    eprintln!("ground fill: {total} tiles in {busy} upload frames at {per_frame} tiles of {VERTEX_TILE_BYTES} B a frame");
    assert!(total > 400, "the fill must be large enough to span many frames: {total}");
    assert!(rig.lp.pending().is_empty(), "the fill drained within the script");
    assert!(busy >= total.div_ceil(per_frame as u64) as usize, "never faster than the cap allows");
    assert!(frames.iter().all(|f| f.written.len() <= per_frame));
    assert!(frames.iter().any(|f| f.deferred > 0));
}
