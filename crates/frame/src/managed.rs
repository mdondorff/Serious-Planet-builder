//! The managed frame loop (STRM-005, STRM-006, STRM-007, STRM-015, STRM-016): the pipelined loop of [`crate::pipeline`] with the
//! residency manager and the scheduler's classes in it, GPU-free (CON-03). One call per iteration:
//!
//! 1. the manager fits the wants (the newest plan's draw set D, the newest residency set R from the worker, and the prefetch
//!    set of the camera's translation, empty without [`ManagedLoop::with_prefetch`], STRM-018) to the pool, evicts and requests by class ([`ResidencyManager::begin_frame`]);
//! 2. the changes of the resident set are executed on the renderer through a [`TileSink`]: a tile that became resident is
//!    placed in the slot the manager chose, an evicted tile is forgotten (the renderer decides nothing);
//! 3. the pipeline decides what to draw from the tiles the sink holds ([`PlanPipeline::begin_frame`]) and delivers the
//!    plan and R the worker finished since the last frame; they are the wants of the next call.
//!
//! R comes from the worker, computed after the plan of the same job from the camera position only ([`PlanField::residency`]),
//! so no frame waits for it and a rotation at a fixed position reuses the previous R without selecting anything.
//! A frame's wants are those of the plan and R delivered up to the frame before (the same one-iteration lag the bare
//! streamer loop had).

use crate::pipeline::{PipelineFrame, PlanPipeline, ResidentTile};
use crate::prefetch::{cap_to_free, PrefetchConfig, TravelEstimator};
use crate::residency::ResidencySet;
use crate::terrain::TerrainRequest;
use planet_core::TileId;
use planet_generators::mesh::TileMesh;
use planet_streaming::{Clock, ResidencyManager, ResidentChange, SlotRef, StreamStats, Wants};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Where the managed loop puts tiles: the renderer, or a counter in tests.
pub trait TileSink {
    /// Write `mesh` into `slot` (the slot the manager chose). Whatever tile the slot held before is gone.
    fn upload_at(&mut self, mesh: &TileMesh, slot: SlotRef) -> Result<(), String>;
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
    pub const NAMES: [&'static str; 9] =
        ["resident", "in_flight", "queued", "dispatched", "cancelled", "blocked", "evicted", "uploaded", "degraded"];

    pub fn values(&self) -> [u64; 9] {
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
    manager: ResidencyManager<TileMesh>,
    /// What the sink holds, with the slot of each tile.
    resident: HashMap<TileId, ResidentTile>,
    slots: HashMap<TileId, SlotRef>,
    wants: Wants,
    wants_plan: Option<u64>,
    wants_residency: Option<Arc<ResidencySet>>,
    /// The prefetch source (STRM-018): the camera path on the injected clock; `None` is the loop without prefetch.
    travel: Option<(TravelEstimator, Arc<dyn Clock>)>,
    last: StreamStats,
}

impl ManagedLoop {
    /// `manager`'s streamer must have been built with `StreamConfig::record_changes` and have its roots loaded
    /// ([`planet_streaming::Streamer::load_roots_blocking`], the only wait); they are put on the sink by the first frame.
    pub fn new(pipe: PlanPipeline, manager: ResidencyManager<TileMesh>) -> Self {
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
        }
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

    pub fn manager(&self) -> &ResidencyManager<TileMesh> {
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

    /// Begin the next frame for `req`; never waits. See the module documentation for the order of the three steps.
    pub fn begin_frame(&mut self, req: &TerrainRequest, sink: &mut dyn TileSink) -> Result<ManagedFrame, String> {
        let t = Instant::now();
        let managed = self.manager.begin_frame(&self.wants);
        let scheduler_ms = t.elapsed().as_secs_f64() * 1000.0;

        let t = Instant::now();
        let mut uploaded = 0u64;
        let streamer = self.manager.streamer_mut();
        for change in streamer.take_changes() {
            match change {
                ResidentChange::Evicted(tile) => {
                    if self.resident.remove(&tile).is_some() {
                        self.slots.remove(&tile);
                        sink.forget(tile);
                    }
                }
                ResidentChange::Added(tile) => {
                    // Act on the state now: a tile added and evicted since the last frame is not resident any more.
                    let (Some(mesh), Some(slot)) = (streamer.get(tile), streamer.slot_of(tile)) else { continue };
                    if self.slots.get(&tile) == Some(&slot) && self.resident.get(&tile).is_some_and(|r| r.mesh_key == mesh.key) {
                        continue;
                    }
                    sink.upload_at(&mesh, slot).map_err(|e| format!("placing {tile:?} in {slot:?}: {e}"))?;
                    self.resident.insert(tile, ResidentTile { mesh_key: mesh.key, origin: mesh.origin });
                    self.slots.insert(tile, slot);
                    uploaded += 1;
                }
            }
        }
        let upload_ms = t.elapsed().as_secs_f64() * 1000.0;

        // The camera path: a pure function of the positions on the clock (rotation never enters).
        let predicted = self.travel.as_mut().and_then(|(est, clock)| {
            est.observe(clock.now_ms(), req.camera.position);
            est.predict()
        });
        let pipeline = self.pipe.begin_frame_predicted(req, predicted, &self.resident)?;
        let stats = self.manager.streamer().stats();
        self.refresh_wants(stats.main_free);
        // No predicted position now (a teleport, a stop, a rotation): the set of the old direction is dropped at once, not when
        // the worker's next result arrives, so its queued requests are cancelled this frame.
        if predicted.is_none() {
            self.wants.prefetch.clear();
        }

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
            degraded: managed.report.dropped,
            draw_replaced: managed.report.draw_replaced.len(),
            budget_failure: managed.report.budget_failure,
            prefetch: self.wants.prefetch.len(),
            cancelled_prefetch: stats.cancelled_prefetch - self.last.cancelled_prefetch,
        };
        self.last = stats;
        Ok(ManagedFrame { pipeline, counts, scheduler_ms, upload_ms })
    }

    /// The wants follow the newest delivered plan (D, in plan order) and R (distance); rebuilt only when either changed.
    /// The prefetch wants are the worker's set for the newest R, capped by the free share (`free_slots` of the main pool).
    fn refresh_wants(&mut self, free_slots: usize) {
        let plan_seq = self.pipe.current().map(|c| c.seq);
        if plan_seq != self.wants_plan {
            self.wants.draw =
                self.pipe.current().map(|c| c.plan.nodes.iter().enumerate().map(|(i, n)| (n.tile, i as f64)).collect()).unwrap_or_default();
            self.wants_plan = plan_seq;
        }
        let residency = self.pipe.residency().map(|r| Arc::clone(&r.residency));
        let same = match (&residency, &self.wants_residency) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.wants.residency = residency.as_ref().map(|r| r.wants()).unwrap_or_default();
            self.wants_residency = residency;
        }
        let set = self.pipe.residency().map(|r| Arc::clone(&r.prefetch));
        self.wants.prefetch = match set {
            Some(set) if !set.is_empty() => cap_to_free(&set, free_slots, |t| self.resident.contains_key(&t)),
            _ => Vec::new(),
        };
    }
}
