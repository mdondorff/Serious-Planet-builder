//! The scheduler's frame-thread work follows what changed, not the size of R (STRM-005, STRM-019, TEST-018), tier A, fakes only.
//!
//! Two kinds of test:
//! - **Equivalence** (STRM-005, CON-14): the request order, the cancellations, the evictions, the resident sets and the counts of
//!   every frame of scripted runs are folded into one digest per script. The digests were taken from the implementation that
//!   rebuilt every derived set each frame (before the change `scheduler-incremental`, PR #161's parent a8d7fb0) and stay
//!   pinned: the incremental scheduler must reproduce each of them bit for bit, on the streamer alone (wants handed over every
//!   frame, failures and retries, a pool too small for R, prefetch) and through the managed loop (plans and R from the worker,
//!   upload budget, prefetch, eviction).
//! - **Cost** (STRM-019, TEST-018): count-based (CON-20), no clock. The streamer counts the elements its frame touches
//!   ([`planet_streaming::SchedWork`]); a settled frame touches none, a rotation touches none of R, and c changes touch O(c).

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::managed::{ManagedLoop, TileSink};
use planet_frame::pipeline::{PlanWorker, PlanPipeline, ResidentRecords};
use planet_frame::prefetch::{prefetch_set, PrefetchConfig};
use planet_frame::upload::UploadBudget;
use planet_frame::{
    residency_set_field, select_nodes_in_view_field, Camera, LodParams, SelectionCache, TerrainRequest, TerrainView,
};
use planet_generators::compact::RenderTile;
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::TileMesh;
use planet_streaming::{FakeClock, ManualSpawner, ResidencyManager, SlotRef, Spawner, StreamConfig, Streamer, TileSource, Wants};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (160, 90);
const CELLS: u32 = 16;
const FRAME_MS: u64 = 33;

/// FNV-1a over u64 words: the digest of a run.
struct Digest(u64);

impl Digest {
    fn new() -> Self {
        Digest(0xcbf2_9ce4_8422_2325)
    }
    fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn tile(&mut self, t: TileId) {
        self.word(t.raw());
    }
}

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
    request_with(lod(), pos, yaw, pitch)
}

fn request_with(lod: LodParams, pos: PlanetFixed, yaw: f64, pitch: f64) -> TerrainRequest {
    TerrainRequest {
        camera: camera(pos, yaw, pitch),
        size: SIZE,
        view: TerrainView::Level,
        lod,
        face_mapping: TangentWarp.id().to_string(),
        seed: 1,
    }
}

/// A camera on a straight ground track east of the start, `metres` along it, `height` above the sphere.
fn track(metres: f64, height: f64) -> PlanetFixed {
    let start = Vec3::new(0.3, 0.5, 0.8).normalized();
    let east = start.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
    position((start + east * (metres / R)).normalized(), height)
}

fn here() -> PlanetFixed {
    position(Vec3::new(0.3, 0.5, 0.8), 800.0)
}

fn there() -> PlanetFixed {
    position(Vec3::new(-0.6, -0.2, -0.5), 300.0)
}

// ---- the streamer alone ----

/// Tiles the streamer asks for; some fail twice first (deterministically, by tile), to exercise retries.
struct FlakySource {
    attempts: Mutex<HashMap<TileId, u32>>,
    flaky_modulus: u64,
}

impl TileSource<u64> for FlakySource {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        if self.flaky_modulus > 0 && tile.level() > 0 && tile.raw() % self.flaky_modulus == 0 {
            let mut a = self.attempts.lock().unwrap();
            let n = a.entry(tile).or_insert(0);
            *n += 1;
            if *n <= 2 {
                return Err(format!("flaky {tile:?} attempt {n}"));
            }
        }
        Ok(tile.raw())
    }
}

struct StreamRig {
    s: Streamer<u64>,
    sp: Arc<ManualSpawner>,
    clock: Arc<FakeClock>,
}

fn stream_rig(capacity: usize, vertex_pool: usize, max_in_flight: usize, flaky_modulus: u64) -> StreamRig {
    let sp = Arc::new(ManualSpawner::default());
    let clock = Arc::new(FakeClock::default());
    let config = StreamConfig {
        max_in_flight,
        capacity,
        vertex_pool_slots: vertex_pool,
        retire_frames: 2,
        retry_after_ms: 100,
        record_dispatches: true,
        record_changes: true,
    };
    let src = FlakySource { attempts: Mutex::default(), flaky_modulus };
    let mut s = Streamer::new(Arc::new(src), sp.clone() as Arc<dyn Spawner>, clock.clone(), config);
    s.load_roots_blocking().unwrap();
    StreamRig { s, sp, clock }
}

struct Views {
    field: Reference,
    p: LodParams,
    cache: SelectionCache,
}

impl Views {
    fn new() -> Views {
        let field = Reference(1);
        let p = LodParams {
            cells: 16,
            viewport_h_px: 360.0,
            tau_px: 4.0,
            max_level: 16,
            max_height_m: field.max_abs_height_m(),
            ..LodParams::earth_1080p()
        };
        Views { field, p, cache: SelectionCache::new() }
    }

    fn wants(&mut self, pos: PlanetFixed, yaw: f64, pitch: f64, ahead: Option<PlanetFixed>) -> Wants {
        let d = select_nodes_in_view_field(&TangentWarp, &camera(pos, yaw, pitch), &self.field, &self.p);
        let r = residency_set_field(&TangentWarp, pos, &self.field, &self.p, &mut self.cache);
        let prefetch = ahead
            .map(|a| prefetch_set(&r, &residency_set_field(&TangentWarp, a, &self.field, &self.p, &mut self.cache)))
            .unwrap_or_default();
        Wants { draw: d.nodes.iter().map(|n| (n.tile, n.distance_m)).collect(), residency: r.wants(), prefetch }
    }
}

/// One frame of the streamer rig, folded into the digest: what it answered, what it counted, what it dispatched and changed.
fn stream_frame(rig: &mut StreamRig, w: &Wants, jobs: usize, d: &mut Digest) {
    rig.clock.advance(FRAME_MS);
    let log_before = rig.s.dispatch_log.len();
    let resolved = rig.s.begin_frame_wants(w);
    for r in &resolved {
        d.tile(r.wanted);
        d.word(r.shown.map_or(u64::MAX, TileId::raw));
    }
    let st = rig.s.stats();
    for v in [
        st.resident as u64,
        st.queued as u64,
        st.in_flight as u64,
        st.completed,
        st.failed,
        st.cancelled,
        st.cancelled_prefetch,
        st.dropped,
        st.evicted,
        st.dispatched,
        st.blocked,
        st.main_in_use as u64,
        st.main_retiring as u64,
        st.main_free as u64,
        st.queued_by_class[0] as u64,
        st.queued_by_class[1] as u64,
        st.queued_by_class[2] as u64,
    ] {
        d.word(v);
    }
    for t in &rig.s.dispatch_log[log_before..] {
        d.tile(*t);
    }
    d.word(u64::MAX - 1);
    for c in rig.s.take_changes() {
        match c {
            planet_streaming::ResidentChange::Added(t) => {
                d.word(1);
                d.tile(t);
            }
            planet_streaming::ResidentChange::Evicted(t) => {
                d.word(2);
                d.tile(t);
            }
        }
    }
    rig.sp.run(jobs);
}

/// The wants of the streamer script, one per frame: settle, rotate, travel (with prefetch), turn, teleport, rotate again. Built
/// once and replayed against every pool and source variant.
fn streamer_wants() -> Vec<Wants> {
    let mut views = Views::new();
    let mut out = vec![views.wants(here(), 0.0, -0.4, None); 30];
    for k in 0..24 {
        out.push(views.wants(here(), f64::from(k) * 0.25, [-1.0, -0.4, 0.2][k as usize % 3], None));
    }
    for k in 0..40 {
        let pos = track(k as f64 * 60.0, 800.0);
        let ahead = track(k as f64 * 60.0 + 240.0, 800.0);
        out.push(views.wants(pos, 0.3, -0.4, (k % 7 != 6).then_some(ahead)));
    }
    for k in 0..12 {
        out.push(views.wants(there(), 1.0 + f64::from(k) * 0.1, -0.3, None));
    }
    for k in 0..16 {
        out.push(views.wants(there(), f64::from(k) * 0.4, -0.2, None));
    }
    out
}

/// The script on the streamer alone; a few tiles fail twice.
fn streamer_script(wants: &[Wants], capacity: usize, vertex_pool: usize, max_in_flight: usize, flaky_modulus: u64, jobs: usize) -> u64 {
    let mut rig = stream_rig(capacity, vertex_pool, max_in_flight, flaky_modulus);
    let mut d = Digest::new();
    for w in wants {
        stream_frame(&mut rig, w, jobs, &mut d);
    }
    let st = rig.s.stats();
    eprintln!(
        "  streamer script cap {capacity}: resident {} completed {} failed {} cancelled {} (prefetch {}) dropped {} evicted {} dispatched {} blocked {}",
        st.resident, st.completed, st.failed, st.cancelled, st.cancelled_prefetch, st.dropped, st.evicted, st.dispatched, st.blocked
    );
    d.0
}

// ---- the managed loop ----

struct Source {
    lod: LodParams,
}

impl TileSource<RenderTile> for Source {
    fn load(&self, tile: TileId) -> Result<RenderTile, String> {
        let field = Reference(1);
        let rec = ResidentRecords::new(&field, &TangentWarp, &self.lod).record(tile);
        Ok(RenderTile::Vertex(TileMesh { id: tile, cells: CELLS, key: rec.mesh_key, origin: rec.origin, vertices: Vec::new() }))
    }
}

#[derive(Default)]
struct CountingSink {
    order: Vec<TileId>,
    forgets: Vec<TileId>,
    bytes: u64,
}

impl TileSink for CountingSink {
    fn upload_bytes(&self, _tile: &RenderTile) -> u64 {
        self.bytes
    }

    fn upload_at(&mut self, tile: &RenderTile, _slot: SlotRef) -> Result<(), String> {
        self.order.push(tile.id());
        Ok(())
    }

    fn forget(&mut self, tile: TileId) {
        self.forgets.push(tile);
    }
}

struct LoopRig {
    lp: ManagedLoop,
    plan_spawner: Arc<ManualSpawner>,
    tile_spawner: Arc<ManualSpawner>,
    sink: CountingSink,
    clock: Arc<FakeClock>,
}

fn loop_rig(capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget) -> LoopRig {
    loop_rig_for(lod(), capacity, prefetch, tile_bytes, budget)
}

fn loop_rig_for(lod: LodParams, capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget) -> LoopRig {
    let clock = Arc::new(FakeClock::default());
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
    let mut streamer = Streamer::new(Arc::new(Source { lod }), tile_spawner.clone() as Arc<dyn Spawner>, clock.clone(), config);
    streamer.load_roots_blocking().unwrap();
    let mut lp = ManagedLoop::new(pipe, ResidencyManager::new(streamer)).with_upload_budget(budget);
    if prefetch {
        lp = lp.with_prefetch(PrefetchConfig::default(), R, clock.clone());
    }
    LoopRig { lp, plan_spawner, tile_spawner, sink: CountingSink { bytes: tile_bytes, ..Default::default() }, clock }
}

impl LoopRig {
    /// One iteration, folded into `d`; then the workers finish what they have (`jobs` tile jobs, the plans).
    fn step(&mut self, req: &TerrainRequest, jobs: usize, d: &mut Digest) {
        self.clock.advance(FRAME_MS);
        let log_before = self.lp.manager().streamer().dispatch_log.len();
        let f = self.lp.begin_frame(req, &mut self.sink).unwrap();
        for v in f.counts.values() {
            d.word(v);
        }
        d.word(f.counts.prefetch as u64);
        d.word(f.counts.cancelled_prefetch);
        d.word(f.counts.draw_replaced as u64);
        d.word(u64::from(f.counts.budget_failure));
        d.word(f.counts.slots_in_use as u64);
        d.word(f.pipeline.draw.plan.nodes.len() as u64);
        for t in &self.lp.manager().streamer().dispatch_log[log_before..] {
            d.tile(*t);
        }
        d.word(u64::MAX - 1);
        self.plan_spawner.run(4);
        self.tile_spawner.run(jobs);
    }

    fn finish(&self, d: &mut Digest) {
        for t in &self.sink.order {
            d.tile(*t);
        }
        d.word(u64::MAX - 2);
        for t in &self.sink.forgets {
            d.tile(*t);
        }
        d.word(u64::MAX - 3);
        let resident: BTreeSet<TileId> = self.lp.resident().keys().copied().collect();
        for t in resident {
            d.tile(t);
        }
        for t in self.lp.pending() {
            d.tile(t);
        }
    }
}

fn dive_position(k: usize) -> PlanetFixed {
    const DESCENT: usize = 45;
    if k < DESCENT {
        track(0.0, 20_000.0 * libm::pow(200.0 / 20_000.0, k as f64 / DESCENT as f64))
    } else {
        track((k - DESCENT) as f64 * 20.0, 200.0)
    }
}

/// Settle, dive (descent then ground segment), teleport, hold, look around.
fn loop_script(capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget, jobs: usize) -> u64 {
    let mut rig = loop_rig(capacity, prefetch, tile_bytes, budget);
    let mut d = Digest::new();
    let req = request(dive_position(0), 0.3, -0.4);
    for _ in 0..40 {
        rig.step(&req, jobs, &mut d);
    }
    for k in 0..105 {
        rig.step(&request(dive_position(k), 0.3, -0.4), jobs, &mut d);
    }
    for _ in 0..40 {
        rig.step(&request(there(), 1.0, -0.3), jobs, &mut d);
    }
    for k in 0..30 {
        rig.step(&request(there(), f64::from(k) * 0.3, [-1.0, -0.4, 0.2][k as usize % 3]), jobs, &mut d);
    }
    rig.finish(&mut d);
    let st = rig.lp.manager().streamer().stats();
    eprintln!(
        "  loop script cap {capacity}: resident {} completed {} cancelled {} (prefetch {}) dropped {} evicted {} dispatched {} blocked {} uploads {} forgets {} pending {}",
        st.resident, st.completed, st.cancelled, st.cancelled_prefetch, st.dropped, st.evicted, st.dispatched, st.blocked, rig.sink.order.len(), rig.sink.forgets.len(), rig.lp.pending().len()
    );
    d.0
}

// spec: STRM-005, STRM-007, CON-14
#[test]
fn the_streamer_alone_requests_cancels_and_evicts_exactly_as_before() {
    // A pool that holds everything, a pool smaller than R (eviction by class, degradation is the manager's), failures with
    // retries, a small in-flight limit (a long queue), a vertex pool for levels 0-2.
    let w = streamer_wants();
    let digests = [
        streamer_script(&w, 1_000_000, 0, 16, 0, 1000),
        streamer_script(&w, 1_000_000, 0, 6, 13, 5),
        streamer_script(&w, 700, 0, 16, 0, 1000),
        streamer_script(&w, 700, 0, 4, 11, 3),
        streamer_script(&w, 900, 64, 8, 0, 20),
    ];
    eprintln!("streamer digests: {digests:?}");
    assert_eq!(digests, STREAMER_DIGESTS);
}

// spec: STRM-005, STRM-006, STRM-007, STRM-018, CON-14
#[test]
fn the_managed_loop_requests_cancels_evicts_and_uploads_exactly_as_before() {
    let budgeted = UploadBudget { tile_bytes_per_frame: 40_000, node_records_in_cap: false };
    let digests = [
        loop_script(1_000_000, false, 0, UploadBudget::default(), 24),
        loop_script(1_000_000, true, 0, UploadBudget::default(), 24),
        loop_script(1_000_000, true, 8_840, budgeted, 12),
        loop_script(500, true, 0, UploadBudget::default(), 24),
        loop_script(500, false, 8_840, budgeted, 7),
    ];
    eprintln!("loop digests: {digests:?}");
    assert_eq!(digests, LOOP_DIGESTS);
}

const STREAMER_DIGESTS: [u64; 5] = [5821096178887817403, 7956851253523423290, 2013283050162853493, 17756168894569450947, 4887772297078303956];
const LOOP_DIGESTS: [u64; 5] = [4794488177596797991, 399955434248795404, 17886947815659735396, 13908339924811075598, 8970155066444902107];

// ---- cost through the managed loop ----

/// Frames at `req` until the view is the exact frame and nothing is queued, running or waiting; returns the sizes of D and R.
fn settle(rig: &mut LoopRig, req: &TerrainRequest) -> (usize, usize) {
    let mut d = Digest::new();
    for k in 0..600 {
        rig.step(req, 10_000, &mut d);
        let counts = rig.lp.manager().streamer().stats();
        let r = &rig.lp.wants().residency;
        if k > 3
            && counts.queued == 0
            && counts.in_flight == 0
            && rig.lp.pending().is_empty()
            && !r.is_empty()
            && r.iter().all(|(t, _)| rig.lp.resident().contains_key(t))
        {
            return (rig.lp.wants().draw.len(), r.len());
        }
    }
    panic!("did not settle");
}

fn work_of(rig: &mut LoopRig, req: &TerrainRequest) -> planet_streaming::SchedWork {
    let before = rig.lp.work();
    let mut d = Digest::new();
    rig.step(req, 10_000, &mut d);
    rig.lp.work().since(&before)
}

fn finer(tau: f64) -> LodParams {
    LodParams { tau_px: tau, ..lod() }
}

// spec: STRM-019, STRM-005, STRM-001, TEST-018
#[test]
fn a_settled_still_view_through_the_loop_does_no_scheduler_work_and_a_rotation_touches_no_tile_of_r_at_any_size_of_r() {
    let mut sizes = Vec::new();
    for tau in [8.0, 2.0] {
        let lp = finer(tau);
        let mut rig = loop_rig_for(lp.clone(), 1_000_000, true, 8_840, UploadBudget::default());
        let first = request_with(lp.clone(), here(), 0.0, -0.4);
        let (_, r_len) = settle(&mut rig, &first);
        sizes.push(r_len);
        // Still: the loop's wants do not change, the manager and the streamer have nothing to do.
        for _ in 0..30 {
            let w = work_of(&mut rig, &first);
            assert_eq!(w, planet_streaming::SchedWork::default(), "tau {tau}: R {r_len} tiles, a settled frame touches nothing");
        }
        // Turning: the plan changes with the view, R does not (the worker hands the same set), nothing is requested or cancelled.
        let mut turned_d = 0u64;
        let mut d_applied = rig.lp.wants().draw.len();
        for k in 0..24 {
            let req = request_with(lp.clone(), here(), f64::from(k) * 0.26, [-1.0, -0.4, 0.2][k as usize % 3]);
            let before = rig.lp.manager().streamer().stats();
            for _ in 0..3 {
                let d_before = rig.lp.wants().draw.len();
                let w = work_of(&mut rig, &req);
                // The list handed over this frame is the one the wants held before it, the layer held the one before that, and the loop
                // rebuilds the wants from the plan it received.
                let d_len = d_before.max(d_applied).max(rig.lp.wants().draw.len());
                d_applied = d_before;
                assert_eq!(w.layer_updates[1], 0, "tau {tau} yaw {k}: R is untouched by a turn");
                assert_eq!(w.layer_updates[2], 0, "no prefetch");
                // The draw set is compared with the last (a sequential pass over D, a few times), the tiles that differ are written.
                assert!(w.scanned <= 6 * d_len as u64 + 64, "tau {tau} yaw {k}: {w:?} with D of {d_len}");
                turned_d += w.updated;
            }
            let after = rig.lp.manager().streamer().stats();
            assert_eq!((after.dispatched, after.cancelled), (before.dispatched, before.cancelled), "yaw {k}: a turn requests and cancels nothing");
        }
        eprintln!("tau {tau}: R {r_len}, a turn wrote {turned_d} elements in 72 frames");
    }
    assert!(sizes[1] >= 3 * sizes[0], "the second scene has a much larger R: {sizes:?}");
}

// spec: STRM-019, STRM-005, TEST-018
#[test]
fn a_step_of_travel_through_the_loop_touches_far_fewer_elements_than_r_has_tiles() {
    let lp = finer(2.0);
    let mut rig = loop_rig_for(lp.clone(), 1_000_000, false, 0, UploadBudget::default());
    let start = request_with(lp.clone(), track(0.0, 200.0), 0.3, -0.4);
    let (_, r_len) = settle(&mut rig, &start);
    // A 3 m step on the ground: the plan and R are asked for, delivered, and the wants follow.
    let before = rig.lp.work();
    let mut d = Digest::new();
    let step = request_with(lp, track(3.0, 200.0), 0.3, -0.4);
    for _ in 0..4 {
        rig.step(&step, 0, &mut d);
    }
    let w = rig.lp.work().since(&before);
    eprintln!("R {r_len}: a 3 m step over 4 frames: {w:?}");
    assert!(w.updated * 5 < r_len as u64, "the writes of a small step are a small part of R: {w:?} against {r_len}");
}
