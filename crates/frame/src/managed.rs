//! The managed frame loop (STRM-005, STRM-006, STRM-007, STRM-015, STRM-016): the pipelined loop of [`crate::pipeline`] with the
//! residency manager and the scheduler's classes in it, GPU-free (CON-03). One call per iteration:
//!
//! 1. the manager fits the wants (the newest plan's draw set D, the newest residency set R from the worker, and the prefetch
//!    set of the camera's translation, empty without [`ManagedLoop::with_prefetch`], STRM-018) to the pool, evicts and requests by class ([`ResidencyManager::begin_frame`]);
//! 2. the changes of the resident set are executed on the renderer through a [`TileSink`]: a tile that became resident is
//!    placed in the slot the manager chose, an evicted tile is forgotten (the renderer decides nothing). Placement is
//!    capped per frame by the [`UploadBudget`] (STRM-006): finished tiles wait in class order and are written while their
//!    bytes fit; the rest stay finished-but-not-placed for the next frame and the view is drawn from what the sink holds;
//! 3. the pipeline decides what to draw from the tiles the sink holds ([`PlanPipeline::begin_frame`]) and delivers the
//!    plan and R the worker finished since the last frame; they are the wants of the next call.
//!
//! R comes from the worker, computed after the plan of the same job from the camera position only ([`PlanField::residency`]),
//! so no frame waits for it and a rotation at a fixed position reuses the previous R without selecting anything.
//! A frame's wants are those of the plan and R delivered up to the frame before (the same one-iteration lag the bare
//! streamer loop had).

use crate::draw::indirect_frame_bytes;
use crate::pipeline::{PipelineFrame, PlanPipeline, ResidentTile};
use crate::prefetch::{cap_to_free, PrefetchConfig, TravelEstimator};
use crate::residency::ResidencySet;
use crate::terrain::TerrainRequest;
use crate::upload::{sort_waiting, upload_count, UploadBudget, UploadClass, Waiting};
use planet_core::TileId;
use planet_generators::compact::RenderTile;
use planet_generators::mesh::MeshVertex;
use planet_streaming::{Clock, ResidencyManager, ResidentChange, SchedWork, SlotRef, StreamStats, Wants};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

/// Where the managed loop puts tiles: the renderer, or a counter in tests.
pub trait TileSink {
    /// Write `tile` (either format) into `slot` (the slot the manager chose). Whatever tile the slot held before is gone.
    fn upload_at(&mut self, tile: &RenderTile, slot: SlotRef) -> Result<(), String>;
    /// Bytes `upload_at` writes for `tile`: the tile's size in the sink's format (the per-frame upload budget counts in
    /// them, STRM-006). The default is what the renderer writes: a vertex tile at its vertex size, a compact tile at its
    /// texels plus its record (8,840 B at cells 32); a sink that writes something else says so.
    fn upload_bytes(&self, tile: &RenderTile) -> u64 {
        match tile {
            RenderTile::Vertex(m) => m.vertices.len() as u64 * MeshVertex::STRIDE,
            RenderTile::Compact(c) => c.resident_bytes(),
        }
    }
    /// The tile was evicted; its slot is the manager's again (no pool is touched).
    fn forget(&mut self, tile: TileId);
}

/// What the scheduler and the manager did in one frame (counts only, CON-20). The `*_total` fields of the streamer's
/// [`StreamStats`] are cumulative; these are per frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCounts {
    /// Tiles resident in slots after the frame, and slots of the main pool in use (resident plus jobs in flight).
    pub resident: usize,
    pub slots_in_use: usize,
    /// Jobs running and requests waiting after the frame.
    pub in_flight: usize,
    pub queued: usize,
    /// Jobs handed to the workers, requests cancelled, dispatches that found no slot, tiles evicted, tiles written to a slot,
    /// during the frame.
    pub dispatched: u64,
    pub cancelled: u64,
    pub blocked: u64,
    pub evicted: u64,
    pub uploaded: u64,
    /// Tile bytes written to slots this frame, finished tiles that did not fit the budget and wait for the next frame, and
    /// the node-record bytes of the frame's plan (an own line against CON-21's 2 ms, not part of the tile cap unless
    /// [`UploadBudget::node_records_in_cap`]; the indirect path's figure).
    pub upload_bytes: u64,
    pub deferred: usize,
    pub node_bytes: u64,
    /// Tiles of R replaced by their ancestors this frame (ADR 0031 point 4b), draw tiles asked for as ancestors (point 4c)
    /// and whether the draw set with its ancestors exceeds the share (a budget failure).
    pub degraded: usize,
    pub draw_replaced: usize,
    pub budget_failure: bool,
    /// Tiles in the prefetch wants after the cap by free share, and prefetch requests cancelled this frame (STRM-018). Not
    /// among the logged columns of [`FrameCounts::NAMES`].
    pub prefetch: usize,
    pub cancelled_prefetch: u64,
}

impl FrameCounts {
    /// Column names of the per-frame log (TEST-012), in [`FrameCounts::values`] order.
    pub const NAMES: [&'static str; 12] = [
        "resident",
        "in_flight",
        "queued",
        "dispatched",
        "cancelled",
        "blocked",
        "evicted",
        "uploaded",
        "degraded",
        "upload_bytes",
        "deferred",
        "node_bytes",
    ];

    pub fn values(&self) -> [u64; 12] {
        [
            self.resident as u64,
            self.in_flight as u64,
            self.queued as u64,
            self.dispatched,
            self.cancelled,
            self.blocked,
            self.evicted,
            self.uploaded,
            self.degraded as u64,
            self.upload_bytes,
            self.deferred as u64,
            self.node_bytes,
        ]
    }
}

/// One iteration of the managed loop.
#[derive(Clone, Debug)]
pub struct ManagedFrame {
    pub pipeline: PipelineFrame,
    pub counts: FrameCounts,
    /// Time of the manager's frame (collect, cancel, request, evict) and of executing the changes on the sink: report
    /// columns (TEST-012), they feed no decision.
    pub scheduler_ms: f64,
    pub upload_ms: f64,
}

pub struct ManagedLoop {
    pipe: PlanPipeline,
    manager: ResidencyManager<RenderTile>,
    /// What the sink holds, with the slot of each tile.
    resident: HashMap<TileId, ResidentTile>,
    slots: HashMap<TileId, SlotRef>,
    wants: Wants,
    wants_plan: Option<u64>,
    wants_residency: Option<Arc<ResidencySet>>,
    /// The prefetch source (STRM-018): the camera path on the injected clock; `None` is the loop without prefetch.
    travel: Option<(TravelEstimator, Arc<dyn Clock>)>,
    last: StreamStats,
    /// The per-frame upload budget (STRM-006) and what it waits on: tiles the streamer finished that the sink does not hold
    /// yet, the class sets of the wants they are ranked by (rebuilt when the wants changed) and the node-record bytes of the
    /// last drawn plan.
    budget: UploadBudget,
    pending: HashSet<TileId>,
    classes: Option<Classes>,
    classes_stale: bool,
    last_node_bytes: u64,
    /// The lists of `wants` that changed since the manager was last given them.
    draw_changed: bool,
    residency_changed: bool,
    prefetch_changed: bool,
    /// The prefetch set the wants were capped from, and whether the whole set fitted the free share.
    prefetch_basis: Option<(Arc<Vec<TileWant>>, bool)>,
    /// The loop's own passes over its wants (rebuilding D and R from a delivery, comparing a delivered R with the held one, capping
    /// the prefetch set), counted like the scheduler's (STRM-019).
    extra: SchedWork,
}

/// A tile with its priority, as in the wants.
type TileWant = (TileId, f64);

/// The tiles of the draw set and of R with their ancestors (an ancestor is requested, and so placed, in its descendants'
/// class), and the plan order of the draw tiles.
struct Classes {
    draw: HashMap<TileId, f64>,
    residency: HashSet<TileId>,
}

impl Classes {
    /// Built only after the wants changed and only when a tile waits.
    fn of(wants: &Wants) -> Classes {
        let mut draw: HashMap<TileId, f64> = HashMap::new();
        for &(tile, priority) in &wants.draw {
            let mut t = Some(tile);
            while let Some(x) = t {
                let e = draw.entry(x).or_insert(priority);
                if *e > priority {
                    *e = priority;
                }
                t = x.parent();
            }
        }
        let mut residency: HashSet<TileId> = HashSet::new();
        for &(tile, _) in &wants.residency {
            let mut t = Some(tile);
            while let Some(x) = t {
                if !residency.insert(x) {
                    break;
                }
                t = x.parent();
            }
        }
        Classes { draw, residency }
    }
}

impl ManagedLoop {
    /// `manager`'s streamer must have been built with `StreamConfig::record_changes` and have its roots loaded
    /// ([`planet_streaming::Streamer::load_roots_blocking`], the only wait); they are put on the sink by the first frame.
    pub fn new(pipe: PlanPipeline, manager: ResidencyManager<RenderTile>) -> Self {
        assert!(manager.streamer().records_changes(), "the managed loop follows the streamer through StreamConfig::record_changes");
        let last = manager.streamer().stats();
        Self {
            pipe,
            manager,
            resident: HashMap::new(),
            slots: HashMap::new(),
            wants: Wants::default(),
            wants_plan: None,
            wants_residency: None,
            travel: None,
            last,
            budget: UploadBudget::default(),
            pending: HashSet::new(),
            classes: None,
            classes_stale: true,
            last_node_bytes: 0,
            draw_changed: false,
            residency_changed: false,
            prefetch_changed: false,
            prefetch_basis: None,
            extra: SchedWork::default(),
        }
    }

    /// Use another per-frame upload budget than [`UploadBudget::default`] (the provisional 1 MiB of ADR 0033 point 9).
    pub fn with_upload_budget(mut self, budget: UploadBudget) -> Self {
        self.budget = budget;
        self
    }

    pub fn upload_budget(&self) -> UploadBudget {
        self.budget
    }

    /// Finished tiles the sink does not hold yet (deferred by the budget), in tile order.
    pub fn pending(&self) -> Vec<TileId> {
        let mut v: Vec<TileId> = self.pending.iter().copied().collect();
        v.sort();
        v
    }

    /// Turn the prefetch source on (STRM-018): the camera's translation on `clock` predicts a position `config.look_ahead_ms`
    /// ahead, the worker computes R there minus R here, and the free share takes it. `radius_m` is the LOD parameters'
    /// reference radius (altitudes are measured from it).
    pub fn with_prefetch(mut self, config: PrefetchConfig, radius_m: f64, clock: Arc<dyn Clock>) -> Self {
        self.travel = Some((TravelEstimator::new(config, radius_m), clock));
        self
    }

    /// The prefetch source's estimator, if prefetch is on.
    pub fn travel(&self) -> Option<&TravelEstimator> {
        self.travel.as_ref().map(|(e, _)| e)
    }

    pub fn pipeline(&self) -> &PlanPipeline {
        &self.pipe
    }

    pub fn manager(&self) -> &ResidencyManager<RenderTile> {
        &self.manager
    }

    /// The tiles the sink holds (what a frame may draw).
    pub fn resident(&self) -> &HashMap<TileId, ResidentTile> {
        &self.resident
    }

    /// The slot the sink holds `tile` in.
    pub fn slot_of(&self, tile: TileId) -> Option<SlotRef> {
        self.slots.get(&tile).copied()
    }

    /// The wants the next frame hands the manager.
    pub fn wants(&self) -> &Wants {
        &self.wants
    }

    /// What the loop's scheduling touched so far, the streamer's work and the loop's own passes over its wants (STRM-019,
    /// CON-20): the difference of two readings is a frame's work. The upload ranking of waiting tiles is not part of it (it
    /// is the upload column's).
    pub fn work(&self) -> SchedWork {
        let w = self.manager.streamer().work();
        SchedWork { scanned: w.scanned + self.extra.scanned, updated: w.updated + self.extra.updated, layer_updates: w.layer_updates }
    }

    /// Begin the next frame for `req`; never waits. See the module documentation for the order of the three steps.
    pub fn begin_frame(&mut self, req: &TerrainRequest, sink: &mut dyn TileSink) -> Result<ManagedFrame, String> {
        let t = Instant::now();
        // Only the lists that changed are handed over (STRM-019); a frame after which nothing changed does no work per tile.
        if std::mem::take(&mut self.draw_changed) {
            self.manager.set_draw(&self.wants.draw);
        }
        if std::mem::take(&mut self.residency_changed) {
            self.manager.set_residency(&self.wants.residency);
        }
        if std::mem::take(&mut self.prefetch_changed) {
            self.manager.set_prefetch(&self.wants.prefetch);
        }
        let report = self.manager.step();
        let (degraded, draw_replaced, budget_failure) = (report.dropped, report.draw_replaced.len(), report.budget_failure);
        let scheduler_ms = t.elapsed().as_secs_f64() * 1000.0;

        let t = Instant::now();
        let (mut uploaded, mut upload_bytes) = (0u64, 0u64);
        let streamer = self.manager.streamer_mut();
        for change in streamer.take_changes() {
            match change {
                ResidentChange::Evicted(tile) => {
                    self.pending.remove(&tile);
                    if self.resident.remove(&tile).is_some() {
                        self.slots.remove(&tile);
                        sink.forget(tile);
                    }
                }
                ResidentChange::Added(tile) => {
                    // Act on the state now: a tile added and evicted since the last frame is not resident any more.
                    let (Some(mesh), Some(slot)) = (streamer.get(tile), streamer.slot_of(tile)) else { continue };
                    if self.slots.get(&tile) == Some(&slot) && self.resident.get(&tile).is_some_and(|r| r.mesh_key == mesh.key()) {
                        continue;
                    }
                    self.pending.insert(tile);
                }
            }
        }
        // The budget (STRM-006): the finished tiles in class order while their bytes fit, at least one.
        if !self.pending.is_empty() {
            if self.classes_stale || self.classes.is_none() {
                self.classes = Some(Classes::of(&self.wants));
                self.classes_stale = false;
            }
            let classes = self.classes.as_ref().expect("classes");
            let mut prefetch: HashSet<TileId> = HashSet::new();
            for &(tile, _) in &self.wants.prefetch {
                let mut t = Some(tile);
                while let Some(x) = t {
                    if !prefetch.insert(x) {
                        break;
                    }
                    t = x.parent();
                }
            }
            let mut waiting = Vec::with_capacity(self.pending.len());
            self.pending.retain(|t| streamer.is_resident(*t));
            for &tile in &self.pending {
                let Some(mesh) = streamer.get(tile) else { continue };
                let (class, priority) = if let Some(p) = classes.draw.get(&tile) {
                    (UploadClass::Draw, *p)
                } else if tile.level() == 0 {
                    (UploadClass::Draw, 0.0)
                } else if classes.residency.contains(&tile) {
                    (UploadClass::Residency, 0.0)
                } else if prefetch.contains(&tile) {
                    (UploadClass::Prefetch, 0.0)
                } else {
                    (UploadClass::Other, 0.0)
                };
                waiting.push(Waiting { tile, class, priority, bytes: sink.upload_bytes(&mesh) });
            }
            sort_waiting(&mut waiting);
            let take = upload_count(&waiting, self.budget.allowance(self.last_node_bytes));
            for w in &waiting[..take] {
                self.pending.remove(&w.tile);
                let (Some(mesh), Some(slot)) = (streamer.get(w.tile), streamer.slot_of(w.tile)) else { continue };
                sink.upload_at(&mesh, slot).map_err(|e| format!("placing {:?} in {slot:?}: {e}", w.tile))?;
                self.resident.insert(w.tile, ResidentTile { mesh_key: mesh.key(), origin: mesh.origin() });
                self.slots.insert(w.tile, slot);
                uploaded += 1;
                upload_bytes += w.bytes;
            }
        }
        let deferred = self.pending.len();
        let upload_ms = t.elapsed().as_secs_f64() * 1000.0;

        // The camera path: a pure function of the positions on the clock (rotation never enters).
        let predicted = self.travel.as_mut().and_then(|(est, clock)| {
            est.observe(clock.now_ms(), req.camera.position);
            est.predict()
        });
        let pipeline = self.pipe.begin_frame_predicted(req, predicted, &self.resident)?;
        let node_bytes = indirect_frame_bytes(pipeline.draw.plan.nodes.len());
        self.last_node_bytes = node_bytes;
        let stats = self.manager.streamer().stats();
        self.refresh_wants(stats.main_free, predicted.is_some());

        let counts = FrameCounts {
            resident: stats.resident,
            slots_in_use: stats.main_in_use,
            in_flight: stats.in_flight,
            queued: stats.queued,
            dispatched: stats.dispatched - self.last.dispatched,
            cancelled: stats.cancelled - self.last.cancelled,
            blocked: stats.blocked - self.last.blocked,
            evicted: stats.evicted - self.last.evicted,
            uploaded,
            upload_bytes,
            deferred,
            node_bytes,
            degraded,
            draw_replaced,
            budget_failure,
            prefetch: self.wants.prefetch.len(),
            cancelled_prefetch: stats.cancelled_prefetch - self.last.cancelled_prefetch,
        };
        self.last = stats;
        Ok(ManagedFrame { pipeline, counts, scheduler_ms, upload_ms })
    }

    /// The wants follow the newest delivered plan (D, in plan order) and R (distance); rebuilt only when either changed.
    /// The prefetch wants are the worker's set for the newest R, capped by the free share (`free_slots` of the main pool).
    fn refresh_wants(&mut self, free_slots: usize, predicted: bool) {
        let plan_seq = self.pipe.current().map(|c| c.seq);
        if plan_seq != self.wants_plan {
            self.wants.draw =
                self.pipe.current().map(|c| c.plan.nodes.iter().enumerate().map(|(i, n)| (n.tile, i as f64)).collect()).unwrap_or_default();
            self.extra.scanned += self.wants.draw.len() as u64;
            self.wants_plan = plan_seq;
            self.classes_stale = true;
            self.draw_changed = true;
        }
        let residency = self.pipe.residency().map(|r| Arc::clone(&r.residency));
        // The worker hands a new set for every request, a turn included; its tiles and distances are the old ones then, and the
        // wants (and the manager's) stay as they are.
        let same = match (&residency, &self.wants_residency) {
            (Some(a), Some(b)) => {
                Arc::ptr_eq(a, b) || {
                    self.extra.scanned += a.tiles.len() as u64;
                    a.tiles == b.tiles
                }
            }
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.wants.residency = residency.as_ref().map(|r| r.wants()).unwrap_or_default();
            self.extra.scanned += self.wants.residency.len() as u64;
            self.wants_residency = residency;
            self.classes_stale = true;
            self.residency_changed = true;
        }
        // No predicted position now (a teleport, a stop, a rotation): the set of the old direction is dropped at once, not when
        // the worker's next result arrives, so its queued requests are cancelled this frame.
        let set = if predicted { self.pipe.residency().map(|r| Arc::clone(&r.prefetch)) } else { None };
        let Some(set) = set.filter(|s| !s.is_empty()) else {
            self.prefetch_basis = None;
            if !self.wants.prefetch.is_empty() {
                self.wants.prefetch.clear();
                self.prefetch_changed = true;
            }
            return;
        };
        // The same set with room for all of it is the same wants: nothing to look at (the cap only binds when the share is nearly
        // full, then the set is capped again every frame).
        if matches!(&self.prefetch_basis, Some((basis, true)) if Arc::ptr_eq(basis, &set)) && free_slots >= set.len() {
            return;
        }
        let prefetch = cap_to_free(&set, free_slots, |t| self.resident.contains_key(&t));
        self.extra.scanned += set.len() as u64;
        self.prefetch_basis = Some((Arc::clone(&set), free_slots >= set.len()));
        if prefetch != self.wants.prefetch {
            self.wants.prefetch = prefetch;
            self.prefetch_changed = true;
        }
    }
}
