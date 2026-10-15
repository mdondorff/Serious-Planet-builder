//! The managed loop on fakes (STRM-005, STRM-006, STRM-007, STRM-015, STRM-016, TEST-012): the residency manager and the
//! scheduler's classes driven through the pipelined loop, with R computed on the plan worker. Fake clock, fake I/O (a source
//! that makes meshes without vertices), manual spawners for plans and tiles, a counting sink for the renderer: counts only
//! (CON-20), no GPU, no wall time.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::managed::{FrameCounts, ManagedFrame, ManagedLoop, TileSink};
use planet_frame::pipeline::{DrawKind, PlanField, PlanPipeline, PlanWorker, ResidentRecords};
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

/// The renderer's side as counts: which tile sits in which slot; a slot never holds two tiles.
#[derive(Default)]
struct CountingSink {
    by_tile: HashMap<TileId, SlotRef>,
    owner: HashMap<SlotRef, TileId>,
    uploads: u64,
    forgets: u64,
}

impl TileSink for CountingSink {
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
    tile_spawner: Arc<ManualSpawner>,
    sink: CountingSink,
    capacity: usize,
}

impl Rig {
    fn new(capacity: usize) -> Rig {
        let plan_spawner = Arc::new(ManualSpawner::default());
        let tile_spawner = Arc::new(ManualSpawner::default());
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
        let mut streamer = Streamer::new(
            Arc::new(Source { lod: lod() }),
            tile_spawner.clone() as Arc<dyn Spawner>,
            Arc::new(FakeClock::default()),
            config,
        );
        streamer.load_roots_blocking().unwrap();
        Rig {
            lp: ManagedLoop::new(pipe, ResidencyManager::new(streamer)),
            plan_spawner,
            tile_spawner,
            sink: CountingSink::default(),
            capacity,
        }
    }

    /// One iteration, then the workers finish what they have (a plan, the tiles dispatched): the next frame sees the results.
    fn step(&mut self, req: &TerrainRequest) -> ManagedFrame {
        let f = self.lp.begin_frame(req, &mut self.sink).unwrap();
        self.check_mirror();
        self.plan_spawner.run(4);
        self.tile_spawner.run(10_000);
        f
    }

    /// The sink holds exactly the streamer's resident tiles, each in the slot the manager chose, no slot twice, and the
    /// pool's accounting adds up (STRM-006, STRM-007).
    fn check_mirror(&self) {
        let streamer = self.lp.manager().streamer();
        let st = streamer.stats();
        assert_eq!(self.sink.by_tile.len(), st.resident, "the sink holds the streamer's resident tiles");
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
