//! The managed loop on fakes (STRM-005, STRM-006, STRM-007, STRM-015, STRM-016, TEST-012): the residency manager and the
//! scheduler's classes driven through the pipelined loop, with R computed on the plan worker. Fake clock, fake I/O (a source
//! that makes meshes without vertices), manual spawners for plans and tiles, a counting sink for the renderer: counts only
//! (CON-20), no GPU, no wall time.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::managed::{FrameCounts, ManagedFrame, ManagedLoop, TileSink, UploadStats};
use planet_frame::pipeline::{DrawKind, PlanField, PlanPipeline, PlanWorker, ResidentRecords};
use planet_frame::prefetch::PrefetchConfig;
use planet_frame::upload::{UploadBudget, DEFAULT_TILE_BYTES_PER_FRAME};
use planet_frame::{
    plan_terrain_field, plan_terrain_threaded, residency_set_field, Camera, LodParams, SelectionCache, TerrainRequest, TerrainView,
};
use planet_generators::compact::CompactTile;
use planet_generators::height::{HeightField, Reference};
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
        min_level: 0,
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

impl TileSource<CompactTile> for Source {
    fn load(&self, tile: TileId) -> Result<CompactTile, String> {
        let field = Reference(1);
        let rec = ResidentRecords::new(&field, &TangentWarp, &self.lod).record(tile);
        Ok(CompactTile {
            id: tile,
            cells: CELLS,
            key: rec.mesh_key,
            origin: rec.origin,
            ctrl: [[0.0; 3]; 9],
            h_origin: 0.0,
            hr_min: 0.0,
            hr_step: 1.0,
            skirt_depth: 0.0,
            flip: false,
            relief_m: 0.0,
            climate_words: [0; planet_generators::climate::CLIMATE_TEXELS],
            texels: Vec::new(),
            change: None,
        })
    }
}

/// `Reference(1)` whose tiles on the line of descent to `region` (its ancestors, itself and its descendants) read an edit revision: an
/// edit bumps `rev`, and exactly those tiles change their mesh key (a stamp's footprint, WDEF-007, without any stamp).
struct EditField {
    inner: Reference,
    rev: Arc<std::sync::atomic::AtomicU64>,
    region: TileId,
}

impl EditField {
    fn touches(&self, tile: TileId) -> bool {
        let under = |a: TileId, b: TileId| {
            let mut x = Some(b);
            while let Some(t) = x {
                if t == a {
                    return true;
                }
                x = t.parent();
            }
            false
        };
        under(tile, self.region) || under(self.region, tile)
    }
}

impl HeightField for EditField {
    fn height(&self, d: Vec3) -> f64 {
        self.inner.height(d)
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "edit-test"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m()
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        self.inner.height_bounds(centre, angular_radius)
    }
    fn reads_cells(&self) -> bool {
        true
    }
    fn read_records_for(&self, tile: TileId, _map: &dyn FaceMapping) -> Vec<planet_world_def::ReadRecord> {
        let mut reads = planet_world_def::single_planet_seed_records(self.seed());
        if self.touches(tile) {
            reads.push(planet_world_def::ReadRecord::Node {
                path: "edit.rev".into(),
                value: planet_world_def::NodeValue::U64(self.rev.load(Ordering::SeqCst)),
            });
        }
        reads
    }
}

/// Like [`Source`] over an [`EditField`]: the tile it makes carries the key the field gives it now.
struct EditSource {
    lod: LodParams,
    field: Arc<EditField>,
}

impl TileSource<CompactTile> for EditSource {
    fn load(&self, tile: TileId) -> Result<CompactTile, String> {
        let rec = ResidentRecords::new(self.field.as_ref(), &TangentWarp, &self.lod).record(tile);
        Source { lod: self.lod.clone() }.load(tile).map(|t| CompactTile { key: rec.mesh_key, origin: rec.origin, ..t })
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

    fn help(&self) -> bool {
        self.run(1) > 0
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
    /// A sink that writes runs of adjacent slots in one call (the renderer's way, REND-016): `upload_batch` reports one call per
    /// run. Off, the batch is the default loop over `upload_at`, one call per tile.
    coalesce: bool,
    /// The size of every batch the loop handed over, and the slot of every tile written, in order.
    batches: Vec<usize>,
    slots_written: Vec<SlotRef>,
}

impl TileSink for CountingSink {
    fn upload_bytes(&self, _tile: &CompactTile) -> u64 {
        self.bytes
    }

    fn upload_at(&mut self, tile: &CompactTile, slot: SlotRef) -> Result<(), String> {
        let id = tile.id();
        if let Some(other) = self.owner.get(&slot) {
            if *other != id {
                return Err(format!("{slot:?} still holds {other:?} when {id:?} is placed in it"));
            }
        }
        if let Some(old) = self.by_tile.insert(id, slot) {
            self.owner.remove(&old);
        }
        self.owner.insert(slot, id);
        self.uploads += 1;
        self.order.push(id);
        self.slots_written.push(slot);
        Ok(())
    }

    fn upload_batch(&mut self, tiles: &[(&CompactTile, SlotRef)]) -> Result<UploadStats, String> {
        self.batches.push(tiles.len());
        let mut stats = UploadStats::default();
        for (tile, slot) in tiles {
            self.upload_at(tile, *slot)?;
            stats.staging_bytes += self.bytes;
        }
        stats.calls = if self.coalesce { runs_of(&tiles.iter().map(|(_, s)| *s).collect::<Vec<_>>()) } else { tiles.len() as u64 };
        Ok(stats)
    }

    fn forget(&mut self, tile: TileId) {
        if let Some(slot) = self.by_tile.remove(&tile) {
            self.owner.remove(&slot);
        }
        self.forgets += 1;
    }
}

/// Runs of adjacent slots (same pool, consecutive indices) once sorted: the calls a coalescing sink needs.
fn runs_of(slots: &[SlotRef]) -> u64 {
    let mut v: Vec<(bool, u32)> = slots.iter().map(|s| (s.pool == planet_streaming::PoolKind::Main, s.index)).collect();
    v.sort_unstable();
    v.dedup();
    (usize::from(!v.is_empty()) + v.windows(2).filter(|w| w[0].0 != w[1].0 || w[1].1 != w[0].1 + 1).count()) as u64
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
        Rig::build_full(capacity, prefetch, tile_bytes, budget, 64)
    }

    fn build_full(capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget, max_in_flight: usize) -> Rig {
        Rig::build_edited(capacity, prefetch, tile_bytes, budget, max_in_flight, None)
    }

    /// A rig over an [`EditField`] (the plan worker and the tile source read the same field), or over `Reference(1)` with `None`.
    fn build_edited(
        capacity: usize,
        prefetch: bool,
        tile_bytes: u64,
        budget: UploadBudget,
        max_in_flight: usize,
        edit: Option<Arc<EditField>>,
    ) -> Rig {
        let clock = Arc::new(FakeClock::default());
        let plan_spawner = Arc::new(ManualSpawner::default());
        let tile_spawner = Arc::new(BatchSpawner::default());
        let plan_field: Arc<dyn PlanField> = match &edit {
            Some(f) => Arc::clone(f) as Arc<dyn PlanField>,
            None => Arc::new(Reference(1)),
        };
        let pipe = PlanPipeline::new(PlanWorker::new(plan_spawner.clone(), plan_field, 2));
        let config = StreamConfig {
            max_in_flight,
            capacity,
            retire_frames: 2,
            retry_after_ms: 100,
            record_dispatches: true,
            record_changes: true,
            ..Default::default()
        };
        let source: Arc<dyn TileSource<CompactTile>> = match edit {
            Some(field) => Arc::new(EditSource { lod: lod(), field }),
            None => Arc::new(Source { lod: lod() }),
        };
        let mut streamer = Streamer::new(source, tile_spawner.clone() as Arc<dyn Spawner>, clock.clone(), config);
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
    // The plan of a field that can be too steep to morph walks the position-only selection itself, for the ceiling of its tiles (ADR 0074):
    // the volumes of the tiles outside the frustum are the plan's, and R adds none.
    assert_eq!(b_r, b_plan, "R adds no volume: the plan's job has evaluated the position-only selection");
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
    assert_eq!(
        b,
        alone.bounds.load(Ordering::SeqCst),
        "R added no volume: the plan's job evaluated the position-only selection (ADR 0074)"
    );
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

/// A tile of 58,608 B, the size of the former vertex tile at cells 32: the cap logic does not depend on the format, and tiles this big make
/// the scripts of these tests reach the 1 MiB cap (compact tiles of 9,168 B, `COMPACT_TILE_BYTES`, do not: the dive peaks at 586,752 B).
const FAT_TILE_BYTES: u64 = 58_608;

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

/// What the frame did, for comparing runs. `bytes` are the steady bytes: the roots written by the start-up load (STRM-001) are
/// exempt from the cap and not counted (these tests run at floor 0: the roots are the six level-0 tiles).
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
        // The roots (level 0 at floor 0) are the start-up load: exempt from the cap, written whatever their bytes (STRM-001); the cap
        // governs the tiles behind them.
        let roots = written.iter().filter(|t| t.level() == 0).count() as u64;
        let (steady_bytes, steady) = (c.upload_bytes - roots * bytes, c.uploaded - roots);
        // The cap is exceeded only by the one-tile rule; a frame that defers is full (nothing more fits).
        assert!(steady_bytes <= cap || steady == 1, "frame {k}: {steady_bytes} bytes in {steady} tiles over the cap {cap}");
        if c.deferred > 0 {
            assert!(steady >= 1, "frame {k}: tiles wait but none was written (starvation)");
            assert!(steady_bytes + bytes > cap, "frame {k}: deferred {} tiles though {steady_bytes} of {cap} bytes were used", c.deferred);
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
        out.push(UploadFrame { written, bytes: steady_bytes, deferred: c.deferred, resident: st.resident });
        rig.finish();
    }
    out
}

// spec: STRM-006, STRM-017, STRM-002
#[test]
fn on_the_dive_and_a_teleport_the_upload_cap_holds_in_every_frame_in_class_order_and_no_frame_is_blank() {
    let mut rig = Rig::budgeted(1_000_000, true, FAT_TILE_BYTES, UploadBudget::default());
    let cap = DEFAULT_TILE_BYTES_PER_FRAME;
    assert_eq!(cap, 1 << 20);
    let c = FrameCounts { upload_bytes: 7, deferred: 8, node_bytes: 9, uploaded: 3, ..Default::default() };
    assert_eq!(FrameCounts::NAMES.len(), c.values().len());
    assert_eq!(&c.values()[7..12], &[3, 0, 7, 8, 9]);
    assert_eq!(&FrameCounts::NAMES[9..12], &["upload_bytes", "deferred", "node_bytes"]);
    // spec: TEST-012 (the call count is the last column, after the bytes: names consistent with the existing upload columns)
    assert_eq!(FrameCounts::NAMES[12], "upload_calls");
    assert_eq!(FrameCounts { upload_calls: 5, ..Default::default() }.values()[12], 5);
    let dive = run_budgeted(&mut rig, 105, &dive_position, true);
    // The teleport: the dive's last place to the other side of the planet, then hold.
    let b = there();
    let teleport = run_budgeted(&mut rig, 80, &|_| b, true);
    let deferring = dive.iter().chain(&teleport).filter(|f| f.deferred > 0).count();
    let peak = dive.iter().chain(&teleport).map(|f| f.bytes).max().unwrap();
    eprintln!("dive+teleport: {deferring} frames deferred tiles, peak {peak} B of {cap} B");
    assert!(deferring > 5, "the scripts must hit the cap, or the test proves nothing: {deferring}");
    assert!(peak <= cap && peak > cap - FAT_TILE_BYTES, "full frames fill the cap to within one tile: {peak}");
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
        Rig::budgeted(1_000_000, false, FAT_TILE_BYTES, UploadBudget { tile_bytes_per_frame: 10_000, node_records_in_cap: false });
    // The six roots are the start-up load: all written in frame 0 whatever their bytes (STRM-001, exempt from the cap); every tile
    // after them is a steady one and goes one a frame, though each is over the cap.
    let frames = run_budgeted(&mut rig, 40, &|_| here(), false);
    assert_eq!(frames[0].written.len(), 6, "the roots in frame 0, over the cap: {:?}", frames[0]);
    assert!(frames[0].written.iter().all(|t| t.level() == 0), "the roots first: {:?}", frames[0].written);
    assert!(frames[1..].iter().all(|f| f.written.len() <= 1), "one tile a frame at most, though every tile is over the cap");
    assert!(frames[1..].iter().filter(|f| f.deferred > 0).count() >= 20, "the script must keep tiles waiting: {frames:?}");
    assert!(frames[1..].iter().all(|f| f.deferred == 0 || f.written.len() == 1), "and one a frame while any waits: {frames:?}");
    let f = rig.step(&request(here(), 0.3, -0.4));
    assert_eq!(f.pipeline.draw.blank, 0, "never blank once the roots are in");
}

// spec: STRM-006
#[test]
fn node_records_are_their_own_line_and_count_inside_the_cap_only_when_asked() {
    let own = UploadBudget { tile_bytes_per_frame: 400_000, node_records_in_cap: false };
    let inside = UploadBudget { node_records_in_cap: true, ..own };
    let mut rigs = [Rig::budgeted(1_000_000, false, FAT_TILE_BYTES, own), Rig::budgeted(1_000_000, false, FAT_TILE_BYTES, inside)];
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
            Rig::budgeted(1_000_000, true, FAT_TILE_BYTES, UploadBudget { tile_bytes_per_frame: 300_000, node_records_in_cap: false });
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
    let mut rig = Rig::budgeted(1_000_000, false, FAT_TILE_BYTES, UploadBudget::default());
    let ground = track(0.0, 2.0);
    let frames = run_budgeted(&mut rig, 120, &|_| ground, true);
    let total: u64 = frames.iter().map(|f| f.written.len() as u64).sum();
    let per_frame = (DEFAULT_TILE_BYTES_PER_FRAME / FAT_TILE_BYTES) as usize;
    let busy = frames.iter().filter(|f| f.bytes > 0).count();
    eprintln!("ground fill: {total} tiles in {busy} upload frames at {per_frame} tiles of {FAT_TILE_BYTES} B a frame");
    assert!(total > 400, "the fill must be large enough to span many frames: {total}");
    assert!(rig.lp.pending().is_empty(), "the fill drained within the script");
    assert!(busy >= total.div_ceil(per_frame as u64) as usize, "never faster than the cap allows");
    assert!(frames.iter().all(|f| f.written.len() <= per_frame));
    assert!(frames.iter().any(|f| f.deferred > 0));
}

// spec: REND-018, STRM-006, STRM-019
#[test]
fn a_frame_that_uploads_under_unchanged_wants_builds_no_set_over_the_wants() {
    // Two tiles a frame against a fill of hundreds: tiles wait for many frames, the camera stays, the wants settle.
    let cap = 2 * FAT_TILE_BYTES;
    let mut rig = Rig::budgeted(1_000_000, true, FAT_TILE_BYTES, UploadBudget { tile_bytes_per_frame: cap, node_records_in_cap: false });
    let ground = track(0.0, 2.0);
    let (mut uploading, mut draw_changes, mut prefetch_changes) = (0u64, 0u64, 0u64);
    let mut draw_after_last_change = 0u64;
    for k in 0..300 {
        let before = rig.lp.wants().clone();
        let f = rig.step(&request(ground, 0.3, -0.4));
        let after = rig.lp.wants();
        if after.draw != before.draw {
            draw_changes += 1;
            draw_after_last_change = 0;
        }
        if after.prefetch != before.prefetch {
            prefetch_changes += 1;
        }
        if f.counts.uploaded > 0 {
            uploading += 1;
            draw_after_last_change += 1;
        }
        if rig.lp.pending().is_empty() && k > 20 {
            break;
        }
    }
    let (class, prefetch) = rig.lp.ranking_rebuilds();
    eprintln!("{uploading} uploading frames, {draw_changes} draw changes, {prefetch_changes} prefetch changes, {class} class and {prefetch} prefetch rebuilds");
    assert!(uploading > 30, "the fill must span many uploading frames: {uploading}");
    // A set is built when the wants it is over changed and a tile waits, never because a frame uploads.
    assert!(
        class >= 1 && class <= draw_changes + 1,
        "{class} draw-class builds for {draw_changes} changes of the draw wants (and the first, before any)"
    );
    assert!(prefetch <= prefetch_changes + 1, "{prefetch} prefetch builds for {prefetch_changes} changes of the prefetch wants");
    assert!(class < uploading, "builds ({class}) must be fewer than uploading frames ({uploading})");
    assert!(draw_after_last_change > 10, "frames that upload after the wants settled: {draw_after_last_change}");
}

// ---- teardown and teleport cancellation (STRM-005) ----
type LockedGate = (std::sync::Mutex<std::sync::mpsc::Sender<()>>, std::sync::Mutex<std::sync::mpsc::Receiver<()>>);

/// The reference field with counted plan and residency calls; the first plan call can be held at a gate.
struct CountedField {
    plans: Arc<AtomicUsize>,
    residencies: Arc<AtomicUsize>,
    gate: Option<LockedGate>,
}

impl PlanField for CountedField {
    fn plan(
        &self,
        req: &TerrainRequest,
        cache: &mut SelectionCache,
        threads: usize,
    ) -> Result<planet_frame::TerrainPlan, planet_frame::PlanError> {
        if self.plans.fetch_add(1, Ordering::SeqCst) == 0 {
            if let Some((entered, release)) = &self.gate {
                entered.lock().unwrap().send(()).unwrap();
                release.lock().unwrap().recv().unwrap();
            }
        }
        PlanField::plan(&Reference(1), req, cache, threads)
    }

    fn residency(&self, req: &TerrainRequest, cache: &mut SelectionCache) -> Result<planet_frame::ResidencySet, planet_frame::PlanError> {
        self.residencies.fetch_add(1, Ordering::SeqCst);
        PlanField::residency(&Reference(1), req, cache)
    }
}

struct CountedSource {
    inner: Source,
    loads: Arc<AtomicUsize>,
}

impl TileSource<CompactTile> for CountedSource {
    fn load(&self, tile: TileId) -> Result<CompactTile, String> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        self.inner.load(tile)
    }
}

struct Torn {
    lp: ManagedLoop,
    plan_spawner: Arc<ManualSpawner>,
    tile_spawner: Arc<ManualSpawner>,
    plans: Arc<AtomicUsize>,
    residencies: Arc<AtomicUsize>,
    loads: Arc<AtomicUsize>,
}

type Gate = (std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>);

fn torn_rig(gate: Option<Gate>) -> Torn {
    let clock = Arc::new(FakeClock::default());
    let (plan_spawner, tile_spawner) = (Arc::new(ManualSpawner::default()), Arc::new(ManualSpawner::default()));
    let (plans, residencies, loads) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let field = CountedField {
        plans: plans.clone(),
        residencies: residencies.clone(),
        gate: gate.map(|(a, b)| (std::sync::Mutex::new(a), std::sync::Mutex::new(b))),
    };
    let pipe = PlanPipeline::new(PlanWorker::new(plan_spawner.clone(), Arc::new(field), 2));
    let config = StreamConfig { max_in_flight: 16, capacity: 1_000_000, retire_frames: 2, record_changes: true, ..Default::default() };
    let mut streamer = Streamer::new(
        Arc::new(CountedSource { inner: Source { lod: lod() }, loads: loads.clone() }),
        tile_spawner.clone() as Arc<dyn Spawner>,
        clock,
        config,
    );
    streamer.load_roots_blocking().unwrap();
    Torn { lp: ManagedLoop::new(pipe, ResidencyManager::new(streamer)), plan_spawner, tile_spawner, plans, residencies, loads }
}

// spec: STRM-005
#[test]
fn dropping_the_loop_cancels_every_queued_tile_job_and_every_plan_job_that_has_not_started() {
    let mut t = torn_rig(None);
    let mut sink = CountingSink::default();
    let a = request(here(), 0.0, -0.4);
    t.lp.begin_frame(&a, &mut sink).unwrap(); // the plan job is asked for
    assert_eq!(t.plan_spawner.run(1), 1);
    // The plan and R arrive; the tile requests go out: 16 dispatched to the spawner, the rest queued in the streamer.
    for _ in 0..3 {
        t.lp.begin_frame(&a, &mut sink).unwrap();
    }
    let stats = t.lp.manager().streamer().stats();
    assert_eq!((stats.in_flight, t.tile_spawner.pending()), (16, 16));
    assert!(stats.queued > 100, "many requests wait: {}", stats.queued);
    // A plan job for another camera is waiting in the plan worker's spawner too.
    t.lp.begin_frame(&request(there(), 1.0, -0.3), &mut sink).unwrap();
    assert_eq!(t.plan_spawner.pending(), 1);
    let (plans, loads) = (t.plans.load(Ordering::SeqCst), t.loads.load(Ordering::SeqCst));
    assert_eq!(loads, 6, "only the six roots were loaded");
    let Torn { lp, plan_spawner, tile_spawner, plans: plan_count, loads: load_count, .. } = t;
    drop(lp);
    // The machine now finds the dispatched tile jobs and the waiting plan job: none of them does any work.
    assert_eq!(tile_spawner.run(1_000), 16, "the dispatched jobs are still found in the spawner...");
    assert_eq!(plan_spawner.run(10), 1);
    assert_eq!(load_count.load(Ordering::SeqCst), loads, "...and none of them loaded a tile");
    assert_eq!(plan_count.load(Ordering::SeqCst), plans, "the waiting plan job planned nothing");
}

// spec: STRM-005
#[test]
fn a_plan_job_in_flight_at_teardown_finishes_its_plan_and_starts_nothing_after_it() {
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let t = torn_rig(Some((entered_tx, release_rx)));
    let Torn { mut lp, plan_spawner, plans, residencies, .. } = t;
    let mut sink = CountingSink::default();
    lp.begin_frame(&request(here(), 0.0, -0.4), &mut sink).unwrap();
    let sp = plan_spawner.clone();
    let job = std::thread::spawn(move || sp.run_one());
    entered_rx.recv().unwrap(); // the plan job is running
    lp.begin_frame(&request(there(), 1.0, -0.3), &mut sink).unwrap(); // a newer request waits behind it
    drop(lp);
    release_tx.send(()).unwrap();
    assert!(job.join().unwrap());
    assert_eq!(plans.load(Ordering::SeqCst), 1, "the plan in flight ran; the pending request never did");
    assert_eq!(residencies.load(Ordering::SeqCst), 0, "R is not selected after the teardown began");
    assert_eq!(plan_spawner.pending(), 0, "no follow-up job was spawned");
}

// spec: STRM-005
#[test]
fn a_teleport_cancels_the_queued_requests_that_left_r_before_they_start() {
    let mut rig = Rig::new(1_000_000);
    let a = request(here(), 0.0, -0.4);
    // At A the plan and R arrive but no tile job finishes: 64 are in flight, the rest of R waits in the streamer's queue.
    for _ in 0..6 {
        rig.begin(&a);
        rig.plan_spawner.run(4);
    }
    let at_a = rig.lp.manager().streamer().stats();
    assert_eq!(at_a.in_flight, 64);
    assert!(at_a.queued > 100, "many requests wait: {}", at_a.queued);
    let r_of_a: BTreeSet<TileId> = rig.lp.wants().residency.iter().map(|(t, _)| *t).collect();
    let jump_at = rig.lp.manager().streamer().dispatch_log.len();
    // The camera jumps to B: B's plan arrives while the 64 jobs still run, so the new wants replace the old ones before any
    // queued request of A can start.
    let b = request(there(), 1.0, -0.3);
    for _ in 0..4 {
        rig.begin(&b);
        rig.plan_spawner.run(4);
    }
    assert_eq!(rig.lp.manager().streamer().dispatch_log.len(), jump_at, "nothing of A's queue started while the jobs ran");
    rig.settle(&b);
    let st = rig.lp.manager().streamer().stats();
    assert!(st.cancelled > 50, "the requests of A's R that B does not need are cancelled: {}", st.cancelled);
    let wanted_b = closure(rig.lp.wants().residency.iter().map(|(t, _)| *t).chain(rig.lp.wants().draw.iter().map(|(t, _)| *t)));
    let started_after: Vec<TileId> = rig.lp.manager().streamer().dispatch_log[jump_at..].to_vec();
    assert!(
        started_after.iter().all(|t| wanted_b.contains(t)),
        "a request that left R started after the jump: {:?}",
        started_after.iter().filter(|t| !wanted_b.contains(t)).take(5).collect::<Vec<_>>()
    );
    assert!(r_of_a.iter().any(|t| !wanted_b.contains(t)), "the jump must leave part of A's R");
}

// ---- detail lag against a fresh plan of each frame's camera (TEST-020) ----

use planet_frame::scenes::{detail_lag, fresh_detail_lag, lag_post_pass, DrawnSet, FrameDraw, FrameLag};

/// What a case logs of a frame for the post-pass, and the lag computed inline on the fake clock at the same moment.
struct LoggedRun {
    draws: Vec<FrameDraw>,
    inline: Vec<FrameLag>,
    reqs: Vec<TerrainRequest>,
}

fn run_logged(rig: &mut Rig, reqs: Vec<TerrainRequest>) -> LoggedRun {
    let field = Reference(1);
    let mut cache = SelectionCache::new();
    let (mut draws, mut inline) = (Vec::new(), Vec::new());
    for req in &reqs {
        let f = rig.begin(req);
        let drawn: Vec<TileId> = f.pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect();
        inline.push(fresh_detail_lag(req, &field, &mut cache, &DrawnSet::new(drawn.iter().copied())).unwrap());
        draws.push(FrameDraw { camera: Some(req.camera), drawn });
        rig.finish();
    }
    LoggedRun { draws, inline, reqs }
}

fn post_pass(run: &LoggedRun, threads: usize) -> Vec<FrameLag> {
    let req_of = |camera: Camera| TerrainRequest { camera, ..run.reqs[0].clone() };
    lag_post_pass(&run.draws, &req_of, &|| Ok(Box::new(Reference(1)) as Box<dyn HeightField>), threads).unwrap()
}

// spec: TEST-020, TEST-018
#[test]
fn a_settled_still_view_has_lag_zero_in_every_frame_and_a_rotation_has_a_lag_in_every_frame_equal_to_the_post_pass() {
    let mut rig = Rig::new(1_000_000);
    rig.settle(&request(here(), 0.0, -0.4));
    // Settled and still: the synchronous frame, lag 0 and nothing uncovered, every frame, against the frame's own plan.
    let still = run_logged(&mut rig, (0..30).map(|_| request(here(), 0.0, -0.4)).collect());
    for (k, l) in still.inline.iter().enumerate() {
        assert_eq!((l.mean, l.worst, l.uncovered.len()), (0.0, 0, 0), "frame {k}: {l:?}");
    }
    // Rotation (10 degrees a frame, far outside the stale-plan rule, so the frames draw the cover; and 0.2 degrees a frame,
    // inside it): every frame has a lag judged against its own camera's plan, nothing is skipped or "not measured", the post-pass
    // reproduces each of them, and with R resident the lag is 0 and nothing is uncovered (the cover is the plan's own selection).
    for (step_deg, frames) in [(10.0f64, 24u32), (0.2, 24)] {
        let reqs = (0..frames).map(|k| request(here(), f64::from(k) * step_deg.to_radians(), [-1.0, -0.4, 0.2][k as usize % 3])).collect();
        let run = run_logged(&mut rig, reqs);
        assert_eq!(run.inline.len(), frames as usize);
        assert!(
            run.inline.iter().all(|l| l.mean == 0.0 && l.worst == 0 && l.uncovered.is_empty()),
            "{step_deg} degrees a frame: {:?}",
            run.inline
        );
        for threads in [1, 3] {
            assert_eq!(post_pass(&run, threads), run.inline, "{step_deg} degrees a frame, {threads} threads");
        }
    }
}

// spec: TEST-020, TEST-018, CON-14
#[test]
fn the_post_pass_equals_the_inline_lag_frame_for_frame_on_a_dive_and_a_teleport_and_on_any_thread_count() {
    // Dive: a lagging descent, 24 tile jobs a frame.
    let mut rig = Rig::new(1_000_000);
    rig.tile_jobs_per_frame = 24;
    rig.settle(&request(dive_position(0), 0.3, -0.4));
    let dive = run_logged(&mut rig, (0..60).map(|k| request(dive_position(k), 0.3, -0.4)).collect());
    assert!(dive.inline.iter().any(|l| l.mean > 0.0), "the script must lag for the comparison to say anything");
    // A visible node always has a tile, root or ancestor, to show (ADR 0032's hard measure): nothing uncovered in any frame.
    assert!(dive.inline.iter().all(|l| l.uncovered.is_empty()), "dive: {:?}", dive.inline.iter().find(|l| !l.uncovered.is_empty()));
    // Teleport: a cold plan and a long fill.
    let mut rig = Rig::new(1_000_000);
    rig.tile_jobs_per_frame = 16;
    rig.settle(&request(here(), 0.0, -0.4));
    let tp = run_logged(&mut rig, (0..40).map(|_| request(there(), 0.5, -0.3)).collect());
    assert!(tp.inline[0].mean > 0.0 && tp.inline.last().unwrap().mean == 0.0, "lag at the jump, none once the draw set is complete");
    assert!(tp.inline.iter().all(|l| l.uncovered.is_empty()), "teleport: {:?}", tp.inline.iter().find(|l| !l.uncovered.is_empty()));
    for run in [&dive, &tp] {
        for threads in [1, 2, 8] {
            let got = post_pass(run, threads);
            assert_eq!(got.len(), run.inline.len());
            for (k, (a, b)) in got.iter().zip(&run.inline).enumerate() {
                assert_eq!(a, b, "{threads} threads, frame {k}");
            }
        }
    }
}

// spec: TEST-020
#[test]
fn a_frame_without_a_recorded_pose_fails_the_post_pass_naming_the_frame() {
    let mut rig = Rig::new(1_000_000);
    rig.settle(&request(here(), 0.0, -0.4));
    let mut run = run_logged(&mut rig, (0..6).map(|k| request(here(), f64::from(k), -0.4)).collect());
    run.draws[4].camera = None;
    let req_of = |camera: Camera| TerrainRequest { camera, ..run.reqs[0].clone() };
    let e = lag_post_pass(&run.draws, &req_of, &|| Ok(Box::new(Reference(1)) as Box<dyn HeightField>), 2).unwrap_err();
    assert!(e.contains("frame 4") && e.contains("no recorded camera pose"), "{e}");
    // The pure function agrees with the plan-versus-plan one on a hand-built case (same code path).
    let fresh = plan_terrain_field(&run.reqs[0], &Reference(1)).unwrap();
    assert_eq!(detail_lag(&fresh, &DrawnSet::of_plan(&fresh), &run.reqs[0].lod).mean, 0.0);
}

// spec: STRM-002, STRM-016, TEST-018, TEST-020
#[test]
fn rotation_at_the_settled_state_has_lag_zero_and_nothing_uncovered_in_every_frame_at_any_turn_rate() {
    // ADR 0031 point 3: rotation never fetches and never blurs. With R resident a camera that only turns draws what the plan of
    // its own camera selects, whether the stale-plan rule lets the frame draw a plan (0.2 degrees a frame) or not (the rest:
    // the frame draws the cover, which is the plan's own selection with "resident" as the condition for descending).
    let mut rig = Rig::new(1_000_000);
    rig.settle(&request(here(), 0.0, -0.4));
    // (degrees a frame, frames): the last is a half turn in 0.2 s at 60 frames a second.
    for (step_deg, frames) in [(0.2f64, 40u32), (0.7, 40), (3.0, 40), (10.0, 36), (15.0, 12)] {
        let pitches = [-0.4, -0.4, -1.0, 0.2];
        for pitch_script in [false, true] {
            let reqs = (0..frames)
                .map(|k| request(here(), f64::from(k) * step_deg.to_radians(), if pitch_script { pitches[k as usize % 4] } else { -0.4 }))
                .collect();
            let run = run_logged(&mut rig, reqs);
            for (k, l) in run.inline.iter().enumerate() {
                assert_eq!(
                    (l.mean, l.worst, l.uncovered.len()),
                    (0.0, 0, 0),
                    "{step_deg} degrees a frame (pitch script {pitch_script}), frame {k}: {l:?}"
                );
            }
        }
    }
    // The turn fetched nothing and cancelled nothing: R was resident and stays the wanted set.
    let st = rig.lp.manager().streamer().stats();
    assert_eq!((st.queued, st.in_flight), (0, 0));
}

// ---- batched tile writes (STRM-006, REND-016) ----

/// Bytes of one compact tile at cells 32: its slot of 9,168 B (GEN-025; what the renderer uploads).
const COMPACT_TILE_BYTES: u64 = 9_168;

/// Run the ground fill with the budget `budget` and return, per frame that wrote tiles, `(counts, slots written)`.
fn ground_fill(budget: UploadBudget, coalesce: bool, frames: usize) -> (Rig, Vec<(FrameCounts, Vec<SlotRef>)>) {
    // Up to 256 jobs finish a frame, so a frame can have the 114 tiles of the 1 MiB cap to write.
    let mut rig = Rig::build_full(1_000_000, false, COMPACT_TILE_BYTES, budget, 256);
    rig.sink.coalesce = coalesce;
    let ground = track(0.0, 2.0);
    let mut out = Vec::new();
    for _ in 0..frames {
        let before = rig.sink.slots_written.len();
        let f = rig.begin(&request(ground, 0.3, -0.4));
        rig.finish();
        let c = f.counts;
        let slots = rig.sink.slots_written[before..].to_vec();
        assert_eq!(c.uploaded as usize, slots.len());
        out.push((c, slots));
    }
    (rig, out)
}

// spec: REND-018, STRM-006, REND-016
#[test]
fn a_frames_tiles_reach_the_sink_as_one_batch_in_upload_order_and_the_calls_are_counted() {
    for n in [1u64, 10, 118] {
        let budget = UploadBudget { tile_bytes_per_frame: n * COMPACT_TILE_BYTES, node_records_in_cap: false };
        for coalesce in [false, true] {
            let (rig, frames) = ground_fill(budget, coalesce, 60);
            let full: Vec<_> = frames.iter().filter(|(c, _)| c.uploaded == n).collect();
            assert!(!full.is_empty(), "N={n}: a frame must reach the cap, or the test proves nothing");
            // One batch per frame that wrote, holding exactly the frame's tiles, in the order they were written.
            let written: Vec<usize> = frames.iter().map(|(c, _)| c.uploaded as usize).filter(|u| *u > 0).collect();
            assert_eq!(rig.sink.batches, written, "N={n}: one batch of the frame's tiles per frame");
            let mut total = 0u64;
            for (c, slots) in &frames {
                total += c.upload_calls;
                if slots.is_empty() {
                    assert_eq!(c.upload_calls, 0, "N={n}: a frame that writes nothing makes no call");
                } else if coalesce {
                    // The bound: calls per frame are the runs of adjacent slots, never more than the tiles.
                    assert_eq!(c.upload_calls, runs_of(slots), "N={n}");
                    assert!(c.upload_calls <= c.uploaded, "N={n}");
                } else {
                    assert_eq!(c.upload_calls, c.uploaded, "N={n}: the default sink makes one call per tile");
                }
            }
            if coalesce {
                let tiles: u64 = frames.iter().map(|(c, _)| c.uploaded).sum();
                eprintln!("N={n}: {} calls for {tiles} tiles in {} upload frames", total, written.len());
                assert!(
                    n == 1 || total < tiles,
                    "N={n}: adjacent slots must coalesce (the lowest-free-slot placement fills slots in order)"
                );
            }
        }
    }
}

// spec: REND-018, STRM-006, REND-016
#[test]
fn batching_changes_neither_the_order_nor_the_placement_nor_the_bytes_of_the_upload() {
    let budget = UploadBudget { tile_bytes_per_frame: 20 * COMPACT_TILE_BYTES, node_records_in_cap: false };
    let (a, fa) = ground_fill(budget, false, 40);
    let (b, fb) = ground_fill(budget, true, 40);
    assert_eq!(a.sink.order, b.sink.order, "the same tiles in the same order");
    assert_eq!(a.sink.slots_written, b.sink.slots_written, "in the same slots");
    let key = |f: &[(FrameCounts, Vec<SlotRef>)]| f.iter().map(|(c, _)| (c.uploaded, c.upload_bytes, c.deferred)).collect::<Vec<_>>();
    assert_eq!(key(&fa), key(&fb), "the same counts per frame, bytes and the cap unchanged");
}

// spec: STRM-026, REND-032
#[test]
fn an_edit_refreshes_the_tiles_it_touches_while_their_old_versions_stay_drawn_and_a_still_camera_gets_a_new_plan() {
    let rev = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let req = request(here(), 0.0, -0.4);
    let region = TileId::from_direction(&TangentWarp, here().0.normalized(), 6);
    let field = Arc::new(EditField { inner: Reference(1), rev: Arc::clone(&rev), region });
    let mut rig = Rig::build_edited(1_000_000, false, 0, UploadBudget::default(), 64, Some(Arc::clone(&field)));
    rig.settle(&req);
    let drawn_before: BTreeSet<TileId> = rig.step(&req).pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect();
    let affected: Vec<TileId> = rig.lp.resident().keys().filter(|t| field.touches(**t)).copied().collect();
    assert!(affected.len() > 3 && affected.iter().any(|t| drawn_before.contains(t)), "the edit must touch drawn tiles: {affected:?}");
    let (completed_before, sink_uploads) = (rig.lp.pipeline().worker().counts().completed, rig.sink.uploads);

    // The edit. Tile jobs wait (the machine is busy) for a few frames: what is drawn meanwhile.
    rev.store(1, Ordering::SeqCst);
    let queued = rig.lp.edit(&affected);
    assert!(queued > 0, "resident tiles in a class are requested again");
    rig.tile_jobs_per_frame = 0;
    let mut saw_stale = false;
    for k in 0..4 {
        let f = rig.step(&req);
        assert_eq!(f.pipeline.draw.blank, 0, "frame {k}: never blank");
        let drawn: BTreeSet<TileId> = f.pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect();
        assert_eq!(drawn, drawn_before, "frame {k}: the edited place is not replaced by coarser tiles while it reloads");
        if f.pipeline.draw.kind == DrawKind::Stale {
            saw_stale = true;
            let want: HashMap<TileId, u64> =
                plan_terrain_field(&req, field.as_ref()).unwrap().nodes.iter().map(|n| (n.tile, n.mesh_key)).collect();
            assert!(
                f.pipeline.draw.plan.nodes.iter().any(|n| want.get(&n.tile).is_some_and(|k| *k != n.mesh_key)),
                "frame {k}: an old version is what is drawn"
            );
        }
    }
    assert!(saw_stale, "the new plan (asked for although the camera did not move) was delivered and the old tiles drawn under it");
    assert!(rig.lp.pipeline().worker().counts().completed > completed_before, "a still camera got a new plan from the edit event");
    assert_eq!(rig.sink.uploads, sink_uploads, "nothing was uploaded while the jobs waited");

    // The jobs finish: every refreshed tile is replaced in place, the frame is the synchronous frame of the new field again.
    rig.tile_jobs_per_frame = 10_000;
    rig.settle_view(&req);
    let f = rig.step(&req);
    assert_eq!(f.pipeline.draw.kind, DrawKind::Exact);
    assert_eq!(f.pipeline.draw.plan, plan_terrain_field(&req, field.as_ref()).unwrap());
    let stats = rig.lp.manager().streamer().stats();
    assert_eq!(stats.refreshed, queued as u64, "every requested refresh replaced its tile");
    assert!(rig.sink.uploads > sink_uploads, "the new versions were uploaded");
}

// spec: STRM-031, STRM-026
#[test]
fn twenty_quick_edits_of_the_same_tiles_start_at_most_two_refresh_jobs_for_each_through_the_managed_loop_and_the_last_edit_wins() {
    let rev = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let req = request(here(), 0.0, -0.4);
    let region = TileId::from_direction(&TangentWarp, here().0.normalized(), 6);
    let field = Arc::new(EditField { inner: Reference(1), rev: Arc::clone(&rev), region });
    let mut rig = Rig::build_edited(1_000_000, false, 0, UploadBudget::default(), 64, Some(Arc::clone(&field)));
    rig.settle(&req);
    let affected: Vec<TileId> = rig.lp.resident().keys().filter(|t| field.touches(**t)).copied().collect();
    assert!(affected.len() > 3);
    // The editor's setting: a frame is 33 ms here, so 20 edits one frame apart are quicker than the 40 ms window.
    rig.lp.set_refresh_settle(40);
    let mark = rig.lp.manager().streamer().dispatch_log.len();
    for edit in 1..=20u64 {
        rev.store(edit, Ordering::SeqCst);
        rig.lp.edit(&affected);
        rig.step(&req);
    }
    rig.settle_view(&req);
    let stats = rig.lp.manager().streamer().stats();
    let log = &rig.lp.manager().streamer().dispatch_log[mark..];
    for t in &affected {
        let jobs = log.iter().filter(|x| *x == t).count();
        assert!(jobs <= 2, "{t:?}: {jobs} refresh jobs for 20 edits");
    }
    assert!(stats.refresh_coalesced >= 19, "the edits that found a refresh waiting are counted: {stats:?}");
    assert_eq!(rig.lp.manager().streamer().held_refreshes(), 0, "nothing is left waiting for the window");
    let f = rig.step(&req);
    assert_eq!(f.pipeline.draw.kind, DrawKind::Exact);
    assert_eq!(f.pipeline.draw.plan, plan_terrain_field(&req, field.as_ref()).unwrap(), "the frame is the one of the last edit's field");
}

// spec: REND-033, STRM-026
#[test]
fn the_edited_tiles_are_previewed_in_the_frame_after_the_edit_before_any_new_plan_and_the_exact_tiles_end_it() {
    let rev = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let req = request(here(), 0.0, -0.4);
    let region = TileId::from_direction(&TangentWarp, here().0.normalized(), 6);
    let field = Arc::new(EditField { inner: Reference(1), rev: Arc::clone(&rev), region });
    let mut rig = Rig::build_edited(1_000_000, false, 0, UploadBudget::default(), 64, Some(Arc::clone(&field)));
    rig.settle(&req);
    let affected: Vec<TileId> = rig.lp.resident().keys().filter(|t| field.touches(**t)).copied().collect();
    let drawn_before: BTreeSet<TileId> = rig.step(&req).pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect();
    let under: Vec<TileId> = affected.iter().copied().filter(|t| drawn_before.contains(t)).collect();
    assert!(!under.is_empty());

    // The edit, and a stamp over the place; the jobs wait.
    rev.store(1, Ordering::SeqCst);
    rig.lp.edit(&affected);
    rig.lp.set_preview(vec![planet_frame::preview::PreviewStamp {
        sign: 1,
        kind: planet_world_def::stamp::StampKind::Hill,
        amount_m: 30.0,
        dir: region.center_direction(&TangentWarp),
        reach_m: 5_000_000.0,
        max_delta_m: 60.0,
    }]);
    rig.tile_jobs_per_frame = 0;
    // The very next frame has no plan made after the edit (the worker is only asked for it in this frame), and its tiles still have the
    // key of the unedited plan: the edited tiles are drawn as stale and previewed all the same.
    let f = rig.step(&req);
    assert!(f.pipeline.delivered.is_none() || f.pipeline.delivered.is_some_and(|(seq, _)| seq < rig.lp.pipeline().frame()));
    let flagged: BTreeSet<TileId> = f.pipeline.draw.plan.nodes.iter().filter(|n| n.preview).map(|n| n.tile).collect();
    assert!(!flagged.is_empty(), "the first frame after the edit previews the edited tiles");
    assert!(flagged.iter().all(|t| affected.contains(t)), "and only tiles the edit named: {flagged:?}");
    assert_eq!(f.pipeline.draw.kind, DrawKind::Stale, "the frame is not exact while they are out of date");
    assert!(f.pipeline.draw.substituted >= flagged.len());

    // The jobs finish: the exact tiles are resident, nothing is previewed, the frame is exact.
    rig.tile_jobs_per_frame = 10_000;
    rig.settle_view(&req);
    let f = rig.step(&req);
    assert_eq!(f.pipeline.draw.kind, DrawKind::Exact);
    assert!(f.pipeline.draw.plan.nodes.iter().all(|n| !n.preview), "the exact tiles end the preview");
}

// spec: STRM-030
#[test]
fn the_roots_are_written_to_the_sink_before_frame_zero_and_the_first_frame_uploads_none_of_them() {
    let mut t = torn_rig(None);
    let mut sink = CountingSink { bytes: 9_168, ..Default::default() };
    let roots = t.lp.upload_roots(&mut sink).unwrap();
    assert_eq!(roots.uploaded, 6, "the six roots of floor 0");
    assert_eq!(roots.bytes, 6 * 9_168, "outside the per-frame cap, whatever their bytes");
    assert_eq!((sink.uploads, sink.by_tile.len()), (6, 6), "every root is in a slot of the sink");
    let first = t.lp.begin_frame(&request(here(), 0.0, -0.4), &mut sink).unwrap();
    assert_eq!((first.counts.uploaded, first.counts.upload_bytes), (0, 0), "frame 0 carries no root upload");
    assert_eq!(sink.uploads, 6, "and wrote nothing more");
    assert_eq!(t.lp.resident().len(), 6, "the loop knows the roots as resident: the first plan draws them");
}

// spec: STRM-030
#[test]
fn without_the_pre_frame_call_the_first_frame_uploads_the_roots_as_it_always_did() {
    let mut t = torn_rig(None);
    let mut sink = CountingSink { bytes: 9_168, ..Default::default() };
    let first = t.lp.begin_frame(&request(here(), 0.0, -0.4), &mut sink).unwrap();
    assert_eq!(first.counts.uploaded, 6, "the exemption of the cap (ADR 0064) is unchanged for a caller that does not call upload_roots");
}

// spec: VIEW-038
#[test]
fn a_refused_plan_makes_a_measured_run_invalid_and_the_window_loop_degrades() {
    let mut rig = Rig::new(1_000_000);
    let good = request(here(), 0.0, -0.4);
    rig.settle(&good);
    let mut bad = good.clone();
    // Far too coarse for 16 cells: the morph interval is empty and the worker refuses the plan.
    bad.lod.tau_px = 200.0;
    assert!(plan_terrain_field(&bad, &Reference(1)).is_err());
    // The window's loop goes on and counts it.
    rig.lp.begin_frame(&bad, &mut rig.sink).unwrap();
    rig.finish();
    rig.lp.begin_frame(&bad, &mut rig.sink).expect("the window's loop degrades");
    assert_eq!(rig.lp.pipeline().refused_plans(), 1);
    // A measured run is invalid at the next frame, naming the count and the first refusal.
    let e = rig.lp.begin_frame_strict(&bad, &mut rig.sink).unwrap_err();
    assert!(e.starts_with("run invalid: the plan was refused 1 time(s)") && e.contains("invalid LOD parameters"), "{e}");
}

// spec: VIEW-038
#[test]
fn a_run_without_a_refusal_is_not_invalid() {
    let mut rig = Rig::new(1_000_000);
    let good = request(here(), 0.0, -0.4);
    for _ in 0..4 {
        rig.clock.advance(FRAME_MS);
        rig.lp.begin_frame_strict(&good, &mut rig.sink).expect("no refusal, no invalid run");
        rig.finish();
    }
}

/// A chunk view whose tiles of level 6 and finer read one chunk each (their ancestor at level 6), ready all at once when `ready` is set.
struct StubView {
    ready: std::sync::atomic::AtomicBool,
}

impl planet_frame::chunkshare::ChunkView for StubView {
    fn top_level(&self) -> u8 {
        6
    }
    fn reads(&self, tile: TileId) -> Vec<planet_generators::chunkfield::ChunkKey> {
        if tile.level() < 6 {
            return Vec::new();
        }
        let shift = tile.level() - 6;
        vec![(tile.face().0, tile.x() >> shift, tile.y() >> shift)]
    }
    fn is_ready(&self, _key: planet_generators::chunkfield::ChunkKey) -> bool {
        self.ready.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// spec: STRM-039
#[test]
fn the_loops_frame_counts_carry_the_stand_in_share_of_its_plan() {
    let mut rig = Rig::new(1_000_000);
    let req = request(here(), 0.0, -0.4);
    rig.settle(&req);
    assert_eq!(rig.step(&req).counts.chunk, planet_frame::chunkshare::ChunkShare::default(), "without a chunk view nothing is counted");
    let view = Arc::new(StubView { ready: std::sync::atomic::AtomicBool::new(false) });
    rig.lp.set_chunk_view(Some(view.clone()));
    let f = rig.step(&req);
    let fine = f.pipeline.draw.plan.nodes.iter().filter(|n| n.tile.level() >= 6).count();
    assert!(fine > 0, "the view sees nodes that read chunk detail");
    assert_eq!((f.counts.chunk.chunk_nodes, f.counts.chunk.chunk_stand_in), (fine, fine), "nothing ready: all of them are the stand-in");
    view.ready.store(true, std::sync::atomic::Ordering::SeqCst);
    let f = rig.step(&req);
    assert_eq!((f.counts.chunk.chunk_nodes, f.counts.chunk.chunk_stand_in), (fine, 0));
    assert_eq!(f.counts.chunk.values(), [fine as u64, 0]);
}

// spec: STRM-040
#[test]
fn a_landed_chunk_refreshes_exactly_the_resident_tiles_that_read_it() {
    let mut rig = Rig::new(1_000_000);
    let req = request(here(), 0.0, -0.4);
    rig.settle(&req);
    let view = Arc::new(StubView { ready: std::sync::atomic::AtomicBool::new(true) });
    assert_eq!(rig.lp.chunk_landed(&[(0, 0, 0)]), 0, "without a view nothing is refreshed");
    rig.lp.set_chunk_view(Some(view));
    assert_eq!(rig.lp.chunk_landed(&[]), 0);
    let resident: Vec<TileId> = rig.lp.resident().keys().copied().collect();
    // The chunk (face, x, y at level 6) of some resident tile of level 6 or finer: its readers are all the resident tiles under it.
    let target = resident.iter().find(|t| t.level() >= 6).map(|t| {
        let s = t.level() - 6;
        (t.face().0, t.x() >> s, t.y() >> s)
    });
    let target = target.expect("the settled view has fine tiles");
    let readers = rig
        .lp
        .resident()
        .keys()
        .copied()
        .filter(|t| t.level() >= 6 && (t.face().0, t.x() >> (t.level() - 6), t.y() >> (t.level() - 6)) == target)
        .count();
    assert!(readers > 0);
    let queued = rig.lp.chunk_landed(&[target]);
    assert_eq!(queued, readers, "exactly the resident tiles that read the chunk are queued for a refresh");
    assert_eq!(rig.lp.chunk_landed(&[(5, 63, 63)]), 0, "a chunk nobody resident reads refreshes nothing");
}

// spec: STRM-047
#[test]
fn the_loop_gives_every_node_the_fade_weight_of_the_chunks_it_reads() {
    let mut rig = Rig::new(1_000_000);
    let req = request(here(), 0.0, -0.4);
    rig.settle(&req);
    let view = Arc::new(StubView { ready: std::sync::atomic::AtomicBool::new(true) });
    let clock = Arc::new(planet_streaming::FakeClock::default());
    // Without a fade every node is at weight 1: the plan is what it was.
    assert!(rig.step(&req).pipeline.draw.plan.nodes.iter().all(|n| n.chunk_weight == 1.0));
    rig.lp.set_chunk_view(Some(view));
    rig.lp.set_chunk_fade(1_500, clock.clone());
    // The chunk of the first drawn node that reads one (the plan's order is fixed, a hash map's is not).
    let drawn = rig.step(&req).pipeline.draw.plan.nodes.iter().map(|n| n.tile).find(|t| t.level() >= 6).expect("the plan draws fine tiles");
    let target = (drawn.face().0, drawn.x() >> (drawn.level() - 6), drawn.y() >> (drawn.level() - 6));
    rig.lp.chunk_landed(&[target]);
    // The fade of a landed chunk starts when the rebuilt tiles reach the renderer (STRM-047), not at the landing: let the refresh finish first.
    rig.settle(&req);
    let reads_target = |t: TileId| t.level() >= 6 && (t.face().0, t.x() >> (t.level() - 6), t.y() >> (t.level() - 6)) == target;
    let mut now = 0u64;
    for (ms, want) in [(0u64, 0.0f32), (750, 0.5), (1_500, 1.0), (4_000, 1.0)] {
        clock.advance(ms - now);
        now = ms;
        let f = rig.step(&req);
        let mut seen = 0;
        for n in &f.pipeline.draw.plan.nodes {
            if reads_target(n.tile) {
                assert!((n.chunk_weight - want).abs() < 1e-6, "{ms} ms: {:?} weighs {}", n.tile, n.chunk_weight);
                seen += 1;
            } else {
                assert_eq!(n.chunk_weight, 1.0, "{ms} ms: {:?} reads no chunk that landed", n.tile);
            }
        }
        assert!(seen > 0, "the plan draws tiles that read the chunk that landed");
    }
}

// spec: STRM-052
#[test]
fn a_rebuild_slower_than_the_fade_still_fades_and_neither_a_neighbour_nor_a_new_version_drops_the_tiles_under_a_faded_chunk() {
    let mut rig = Rig::new(1_000_000);
    let req = request(here(), 0.0, -0.4);
    rig.settle(&req);
    let view = Arc::new(StubView { ready: std::sync::atomic::AtomicBool::new(true) });
    let clock = Arc::new(planet_streaming::FakeClock::default());
    rig.lp.set_chunk_view(Some(view));
    rig.lp.set_chunk_fade(1_500, clock.clone());
    let drawn = rig.step(&req).pipeline.draw.plan.nodes.iter().map(|n| n.tile).find(|t| t.level() >= 6).expect("the plan draws fine tiles");
    let key_of = |t: TileId| (t.face().0, t.x() >> (t.level() - 6), t.y() >> (t.level() - 6));
    let target = key_of(drawn);
    let reads_target = |t: TileId| t.level() >= 6 && key_of(t) == target;
    let weights = |rig: &mut Rig| -> Vec<f32> {
        let f = rig.step(&req);
        f.pipeline.draw.plan.nodes.iter().filter(|n| reads_target(n.tile)).map(|n| n.chunk_weight).collect()
    };
    // The chunk lands and the rebuild takes 2 s, longer than the fade: nothing of the change is drawn before the rebuilt tiles arrive, and
    // the first frame that draws them starts at the base, not at the whole change.
    rig.lp.chunk_landed(&[target]);
    // The frames after the landing draw the tiles built before it as stale tiles, whose node carries the resident key: no clock starts.
    // The stale tiles keep the weight they had (1: nothing of the landed chunk was in them) and the clock does not run.
    assert!(weights(&mut rig).iter().all(|&w| w == 1.0), "the frame after the landing");
    // The rebuild is delivered 2 s after the landing, longer than the fade: the first frame that draws the rebuilt tiles starts at weight 0.
    clock.advance(2_000);
    {
        // A tile still waiting for its rebuild keeps the weight it had; one that has been rebuilt starts at the base, not at the whole change.
        let f = rig.step(&req);
        for n in f.pipeline.draw.plan.nodes.iter().filter(|n| reads_target(n.tile)) {
            let want = if n.state == planet_frame::TileState::Stale { 1.0 } else { 0.0 };
            assert_eq!(n.chunk_weight, want, "2 s after the landing: {:?} is {:?}", n.tile, n.state);
        }
    }
    rig.settle(&req);
    let w0 = weights(&mut rig);
    assert!(!w0.is_empty() && w0.iter().all(|&w| w == 0.0), "the first frame that draws the rebuilt tiles: {w0:?}");
    clock.advance(750);
    assert!(weights(&mut rig).iter().all(|&w| (w - 0.5).abs() < 1e-6));
    clock.advance(1_000);
    assert!(weights(&mut rig).iter().all(|&w| w == 1.0));
    // A chunk that nobody under the target reads lands: the tiles under the target keep weight 1 through the refresh of the others.
    let other = resident_chunk_other_than(&rig, target);
    rig.lp.chunk_landed(&[other]);
    rig.settle(&req);
    assert!(weights(&mut rig).iter().all(|&w| w == 1.0), "a neighbour landing leaves the tiles under the faded chunk alone");
    // The target lands again as a new version: its tiles are rebuilt and keep their weight (no drop to the base, no second fade).
    rig.lp.chunk_landed(&[target]);
    assert!(weights(&mut rig).iter().all(|&w| w == 1.0), "before the rebuilt tiles arrive");
    rig.settle(&req);
    assert!(weights(&mut rig).iter().all(|&w| w == 1.0), "after");
}

/// The key of another chunk some resident tile of level 6 or finer lies in.
fn resident_chunk_other_than(rig: &Rig, target: (u8, u32, u32)) -> (u8, u32, u32) {
    rig.lp
        .resident()
        .keys()
        .filter(|t| t.level() >= 6)
        .map(|t| (t.face().0, t.x() >> (t.level() - 6), t.y() >> (t.level() - 6)))
        .find(|k| *k != target)
        .expect("the settled view reads more than one chunk")
}

// spec: STRM-053
#[test]
fn a_drawn_tile_that_reads_a_chunk_is_never_stale_while_that_chunks_clock_runs() {
    use planet_frame::TileState;
    let mut rig = Rig::new(1_000_000);
    let req = request(here(), 0.0, -0.4);
    rig.settle(&req);
    let view = Arc::new(StubView { ready: std::sync::atomic::AtomicBool::new(true) });
    let clock = Arc::new(planet_streaming::FakeClock::default());
    rig.lp.set_chunk_view(Some(view));
    rig.lp.set_chunk_fade(1_500, clock.clone());
    let drawn = rig.step(&req).pipeline.draw.plan.nodes.iter().map(|n| n.tile).find(|t| t.level() >= 6).expect("the plan draws fine tiles");
    let key_of = |t: TileId| (t.face().0, t.x() >> (t.level() - 6), t.y() >> (t.level() - 6));
    let target = key_of(drawn);
    rig.lp.chunk_landed(&[target]);
    // The rule that makes the increment planes right: a rebuilt tile is drawn at weight 0 while any tile that reads the chunk is still the
    // stale one, because the clock of the chunk waits for it. Frame by frame, 100 ms apart, through the refresh of the landing.
    let (mut saw_stale, mut saw_clock) = (false, false);
    let mut last = 0.0f32;
    for _ in 0..40 {
        let f = rig.step(&req);
        let readers: Vec<_> = f.pipeline.draw.plan.nodes.iter().filter(|n| n.tile.level() >= 6 && key_of(n.tile) == target).collect();
        if readers.iter().any(|n| n.state == TileState::Stale) {
            saw_stale = true;
            assert!(
                readers.iter().filter(|n| n.state == TileState::Exact).all(|n| n.chunk_weight == 0.0),
                "a rebuilt tile moved while a stale tile that reads the chunk was still drawn"
            );
        } else if readers.iter().all(|n| n.state == TileState::Exact) {
            let w = readers.iter().map(|n| n.chunk_weight).fold(1.0, f32::min);
            if w > 0.0 {
                saw_clock = true;
            }
            assert!(w >= last, "the weight fell from {last} to {w}");
            last = w;
        }
        clock.advance(100);
    }
    assert!(saw_clock, "the fade ran once the stale tiles were rebuilt (stale seen: {saw_stale})");
}

/// A chunk view with the publish gate on (STRM-054): every chunk is ready; what the loop publishes is recorded.
struct GatedView {
    published: std::sync::Mutex<Vec<planet_generators::chunkfield::ChunkKey>>,
}

impl planet_frame::chunkshare::ChunkView for GatedView {
    fn top_level(&self) -> u8 {
        6
    }
    fn reads(&self, tile: TileId) -> Vec<planet_generators::chunkfield::ChunkKey> {
        StubView { ready: std::sync::atomic::AtomicBool::new(true) }.reads(tile)
    }
    fn is_ready(&self, _key: planet_generators::chunkfield::ChunkKey) -> bool {
        true
    }
    fn gated(&self) -> bool {
        true
    }
    fn publish(&self, keys: &[planet_generators::chunkfield::ChunkKey]) {
        self.published.lock().unwrap().extend_from_slice(keys);
    }
}

// spec: STRM-054
#[test]
fn a_landing_waits_for_the_running_fade_and_is_published_when_it_ends_without_the_timeout() {
    let mut rig = Rig::new(1_000_000);
    let req = request(here(), 0.0, -0.4);
    rig.settle(&req);
    let view = Arc::new(GatedView { published: Default::default() });
    let clock = Arc::new(planet_streaming::FakeClock::default());
    rig.lp.set_chunk_view(Some(view.clone()));
    rig.lp.set_chunk_fade(1_500, clock.clone());
    let key_of = |t: TileId| (t.face().0, t.x() >> (t.level() - 6), t.y() >> (t.level() - 6));
    let drawn = rig.step(&req).pipeline.draw.plan.nodes.iter().map(|n| n.tile).find(|t| t.level() >= 6).expect("the plan draws fine tiles");
    let (a, b) = (key_of(drawn), resident_chunk_other_than(&rig, key_of(drawn)));
    // Nothing fades yet: the first landing is published at once, and its stale tiles are rebuilt before its clock starts.
    rig.lp.chunk_landed(&[a]);
    assert_eq!(*view.published.lock().unwrap(), vec![a]);
    rig.settle(&req);
    rig.step(&req);
    // 500 ms into the fade another chunk lands: it waits.
    clock.advance(500);
    rig.lp.chunk_landed(&[b]);
    assert_eq!(*view.published.lock().unwrap(), vec![a], "the second landing waits for the fade of the first");
    // Frame by frame to the end of the fade (100 ms apart): published at the first frame at or after the end, never forced.
    let mut at = None;
    for _ in 0..30 {
        clock.advance(100);
        rig.step(&req);
        rig.lp.chunk_landed(&[]);
        if at.is_none() && view.published.lock().unwrap().contains(&b) {
            at = Some(planet_streaming::Clock::now_ms(&*clock));
        }
    }
    let at = at.expect("the waiting chunk is published");
    assert!((1_500..=1_800).contains(&at), "published at {at} ms: the fade of the first lasts 1.5 s and the rebuild a few frames");
    assert_eq!(rig.lp.forced_publications(), 0);
}

// spec: STRM-054
#[test]
fn a_landing_held_longer_than_the_timeout_is_published_anyway_and_counted() {
    let mut rig = Rig::new(1_000_000);
    let req = request(here(), 0.0, -0.4);
    rig.settle(&req);
    let view = Arc::new(GatedView { published: Default::default() });
    let clock = Arc::new(planet_streaming::FakeClock::default());
    rig.lp.set_chunk_view(Some(view.clone()));
    // A fade that outlasts the timeout stands for a stale tile that never gets its rebuild: the clock is not idle for 20 s.
    rig.lp.set_chunk_fade(20_000, clock.clone());
    let key_of = |t: TileId| (t.face().0, t.x() >> (t.level() - 6), t.y() >> (t.level() - 6));
    let drawn = rig.step(&req).pipeline.draw.plan.nodes.iter().map(|n| n.tile).find(|t| t.level() >= 6).expect("the plan draws fine tiles");
    let (a, b) = (key_of(drawn), resident_chunk_other_than(&rig, key_of(drawn)));
    rig.lp.chunk_landed(&[a]);
    rig.settle(&req);
    rig.step(&req);
    rig.lp.chunk_landed(&[b]);
    for _ in 0..4 {
        clock.advance(1_000);
        rig.step(&req);
        rig.lp.chunk_landed(&[]);
        assert!(!view.published.lock().unwrap().contains(&b), "{} ms: still waiting", planet_streaming::Clock::now_ms(&*clock));
    }
    assert_eq!(rig.lp.forced_publications(), 0);
    clock.advance(1_000);
    rig.step(&req);
    rig.lp.chunk_landed(&[]);
    assert!(view.published.lock().unwrap().contains(&b), "5 s after it landed it is published anyway");
    assert_eq!(rig.lp.forced_publications(), 1);
}

// spec: STRM-054
#[test]
fn chunks_landing_along_a_stream_are_each_published_within_one_fade_and_a_rebuild_and_none_is_forced() {
    let mut rig = Rig::new(1_000_000);
    let req = request(here(), 0.0, -0.4);
    rig.settle(&req);
    let view = Arc::new(GatedView { published: Default::default() });
    let clock = Arc::new(planet_streaming::FakeClock::default());
    rig.lp.set_chunk_view(Some(view.clone()));
    rig.lp.set_chunk_fade(1_500, clock.clone());
    let key_of = |t: TileId| (t.face().0, t.x() >> (t.level() - 6), t.y() >> (t.level() - 6));
    rig.step(&req);
    let mut keys: Vec<_> = rig.lp.resident().keys().filter(|t| t.level() >= 6).map(|t| key_of(*t)).collect();
    keys.sort_unstable();
    keys.dedup();
    assert!(keys.len() >= 3, "the settled view reads several chunks: {keys:?}");
    // One chunk lands every 300 ms (a flight along a river), frames 100 ms apart, then 4 s without landings; when each was published.
    let mut landed_at = std::collections::HashMap::new();
    let mut published_at = std::collections::HashMap::new();
    let mut next = 0;
    for frame in 0..120u64 {
        let now = planet_streaming::Clock::now_ms(&*clock);
        let mut landed = Vec::new();
        if frame % 3 == 0 && frame < 80 && next < keys.len() {
            landed.push(keys[next]);
            landed_at.insert(keys[next], now);
            next += 1;
        }
        rig.lp.chunk_landed(&landed);
        for k in view.published.lock().unwrap().iter() {
            published_at.entry(*k).or_insert(now);
        }
        rig.step(&req);
        clock.advance(100);
    }
    // A chunk waits for the fade of the epoch before it (1.5 s) and the rebuild of that epoch's tiles (a few frames): never more than 1.9 s.
    let worst = landed_at.iter().map(|(k, at)| published_at.get(k).map(|p| p - at).unwrap_or(u64::MAX)).max().unwrap();
    assert!(worst <= 1_900, "the longest wait for publication was {worst} ms");
    assert_eq!(published_at.len(), landed_at.len(), "every landed chunk is published");
    assert_eq!(rig.lp.forced_publications(), 0, "none needed the timeout");
}
