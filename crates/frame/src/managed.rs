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
use planet_generators::compact::CompactTile;
use planet_streaming::{Clock, ResidencyManager, ResidentChange, SchedWork, SlotRef, StreamStats, Wants};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

/// Where the managed loop puts tiles: the renderer, or a counter in tests.
pub trait TileSink {
    /// Write `tile` into `slot` (the slot the manager chose). Whatever tile the slot held before is gone.
    fn upload_at(&mut self, tile: &CompactTile, slot: SlotRef) -> Result<(), String>;
    /// Bytes `upload_at` writes for `tile`: the tile's size in the sink's format (the per-frame upload budget counts in
    /// them, STRM-006). The default is what the renderer writes: a compact tile at its
    /// slot (header, climate block and texels, 9,168 B at cells 32); a sink that writes something else says so.
    fn upload_bytes(&self, tile: &CompactTile) -> u64 {
        tile.resident_bytes()
    }
    /// Write a frame's tiles, in this order, each into its slot: the same result as `upload_at` for each (STRM-006). A sink that
    /// can write runs of adjacent slots in one go overrides this and says what it cost; the default is the loop over
    /// `upload_at` and counts one call and `upload_bytes` per tile. An error names the tile it failed on.
    fn upload_batch(&mut self, tiles: &[(&CompactTile, SlotRef)]) -> Result<UploadStats, String> {
        let mut stats = UploadStats::default();
        for (tile, slot) in tiles {
            self.upload_at(tile, *slot).map_err(|e| format!("placing {:?} in {slot:?}: {e}", tile.id()))?;
            stats.calls += 1;
            stats.staging_bytes += self.upload_bytes(tile);
        }
        Ok(stats)
    }
    /// The tile was evicted; its slot is the manager's again (no pool is touched).
    fn forget(&mut self, tile: TileId);
}

/// What writing a frame's tiles cost the queue (CON-20, count-based): calls (each a staging allocation and a copy command) and
/// the bytes they carried.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadStats {
    pub calls: u64,
    pub staging_bytes: u64,
}

/// What [`ManagedLoop::upload_roots`] wrote before frame 0: the tiles, their bytes in the sink's format and the sink's calls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RootUpload {
    pub uploaded: u64,
    pub bytes: u64,
    pub calls: u64,
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
    /// Queue calls (`write_buffer`, `write_texture`) the frame's tile writes made (REND-016): at most one per run of adjacent
    /// slots when batched, one or two per tile when written one by one.
    pub upload_calls: u64,
    /// Tiles of R replaced by their ancestors this frame (ADR 0031 point 4b), draw tiles asked for as ancestors (point 4c)
    /// and whether the draw set with its ancestors exceeds the share (a budget failure).
    pub degraded: usize,
    pub draw_replaced: usize,
    pub budget_failure: bool,
    /// Tiles in the prefetch wants after the cap by free share, and prefetch requests cancelled this frame (STRM-018). Not
    /// among the logged columns of [`FrameCounts::NAMES`].
    pub prefetch: usize,
    pub cancelled_prefetch: u64,
    /// The plan's nodes that read chunk detail and those still on the base because their chunk is not ready (STRM-039); all zero without a
    /// chunk view. Logged under [`crate::chunkshare::CHUNK_COLUMNS`], not among [`FrameCounts::NAMES`] (the scheduler digests keep their width).
    pub chunk: crate::chunkshare::ChunkShare,
}

impl FrameCounts {
    /// Column names of the per-frame log (TEST-012), in [`FrameCounts::values`] order.
    pub const NAMES: [&'static str; 13] = [
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
        "upload_calls",
    ];

    /// The columns before `upload_calls`: the width the scheduler-equivalence digests were taken with (a figure of the sink is
    /// not a scheduler decision, REND-018).
    pub const DIGESTED_COLUMNS: usize = 12;

    pub fn values(&self) -> [u64; 13] {
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
            self.upload_calls,
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
    manager: ResidencyManager<CompactTile>,
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
    /// The prefetch wants with their ancestors (an ancestor is requested in its descendants' class), rebuilt only when the
    /// prefetch wants changed.
    prefetch_closure: HashSet<TileId>,
    prefetch_closure_stale: bool,
    /// The waiting tiles and the frame's batch, kept between frames so that a frame that uploads allocates no list for them.
    waiting_scratch: Vec<Waiting>,
    batch_scratch: Vec<(TileId, Arc<CompactTile>, SlotRef, u64)>,
    /// How often the class sets and the prefetch closure were rebuilt (O(wants) each): a frame whose wants did not change
    /// rebuilds neither (REND-018, STRM-019).
    class_rebuilds: u64,
    prefetch_rebuilds: u64,
    last_node_bytes: u64,
    /// What the frame knows of the chunk field (STRM-039): set by the window that has one.
    chunk_view: Option<Arc<dyn crate::chunkshare::ChunkView>>,
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

/// The tiles of the draw set with their ancestors (an ancestor is requested, and so placed, in its descendants' class) and the
/// plan order of the draw tiles. The residency class needs no set of its own: R is closed under parents
/// ([`ResidencySet`]), so its closure is the set, and a tile is looked up in it.
#[derive(Default)]
struct Classes {
    draw: HashMap<TileId, f64>,
}

impl Classes {
    #[cfg(test)]
    fn of(wants: &Wants) -> Classes {
        let mut c = Classes::default();
        c.fill(wants, 0);
        c
    }

    /// Rebuilt after the draw wants changed and only when a tile waits, in the map the last build left (its memory is kept: a
    /// fresh map of a few thousand entries is a megabyte the OS must hand out and zero every time, which is where a frame shared
    /// with the generation threads loses milliseconds). The walks end at the render `floor` (LOD-011): no tile above it is a member.
    fn fill(&mut self, wants: &Wants, floor: u8) {
        // An entry's ancestors always hold a priority at most its own (every walk lowers them all), so a walk that meets a tile
        // already at or below its priority has nothing left to change above it: the draw set's tiles share their ancestors, and
        // the walks stop there instead of running to the root (the priorities are plan indices, never NaN).
        let draw = &mut self.draw;
        draw.clear();
        for &(tile, priority) in &wants.draw {
            let mut t = Some(tile);
            while let Some(x) = t {
                match draw.entry(x) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        if *e.get() <= priority {
                            break;
                        }
                        e.insert(priority);
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(priority);
                    }
                }
                t = x.parent().filter(|_| x.level() > floor);
            }
        }
    }
}

impl ManagedLoop {
    /// `manager`'s streamer must have been built with `StreamConfig::record_changes` and have its roots loaded
    /// ([`planet_streaming::Streamer::load_roots_blocking`], the only wait); they are put on the sink by the first frame.
    pub fn new(pipe: PlanPipeline, manager: ResidencyManager<CompactTile>) -> Self {
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
            prefetch_closure: HashSet::new(),
            prefetch_closure_stale: true,
            waiting_scratch: Vec::new(),
            batch_scratch: Vec::new(),
            class_rebuilds: 0,
            prefetch_rebuilds: 0,
            last_node_bytes: 0,
            chunk_view: None,
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

    /// How often the loop rebuilt the class sets of the upload ranking and the prefetch closure so far: each is O(wants) and
    /// happens when the wants changed and a tile waits, never in a frame whose wants stayed (REND-018).
    pub fn ranking_rebuilds(&self) -> (u64, u64) {
        (self.class_rebuilds, self.prefetch_rebuilds)
    }

    /// The prefetch source's estimator, if prefetch is on.
    pub fn travel(&self) -> Option<&TravelEstimator> {
        self.travel.as_ref().map(|(e, _)| e)
    }

    pub fn pipeline(&self) -> &PlanPipeline {
        &self.pipe
    }

    /// [`ManagedLoop::begin_frame`] for a measured or checked run, which a refused plan makes invalid: the first frame after the worker
    /// refused the plan returns an error naming the count and the refusal, so the scene ends with it (VIEW-038). The window uses
    /// [`ManagedLoop::begin_frame`], which degrades instead (VIEW-032).
    pub fn begin_frame_strict(&mut self, req: &TerrainRequest, sink: &mut dyn TileSink) -> Result<ManagedFrame, String> {
        let frame = self.begin_frame(req, sink)?;
        let refused = self.pipe.refused_plans();
        if refused > 0 {
            let first = self.pipe.take_refusal().unwrap_or_else(|| "(reported before)".to_string());
            return Err(format!("run invalid: the plan was refused {refused} time(s); first refusal: {first}"));
        }
        Ok(frame)
    }

    /// A refusal of the plan not reported yet, once (see [`PlanPipeline::take_refusal`]).
    pub fn take_plan_refusal(&mut self) -> Option<String> {
        self.pipe.take_refusal()
    }

    pub fn manager(&self) -> &ResidencyManager<CompactTile> {
        &self.manager
    }

    /// Announce the item change of the next [`ManagedLoop::edit`] to the plan cache (STRM-034): the field's item revision went from `from` to `to`,
    /// and `changes` is where the items that differ were and are (`WorldDefinition::item_changes_from`). Call it before `edit`.
    pub fn note_item_change(&mut self, from: u64, to: u64, changes: Vec<crate::lod::ItemChange>) {
        self.pipe.worker().note_item_change(from, to, changes);
    }

    /// An item edit changed what `tiles` read (STRM-026, E1): the plan cache forgets them, the resident ones are refreshed (drawn from
    /// their old version until the new one is uploaded, REND-032), and a new plan is asked for at the next frame although the camera
    /// did not move. `tiles` is the footprint find of the edit (WDEF-007). Returns the refresh requests queued.
    pub fn edit(&mut self, tiles: &[TileId]) -> usize {
        self.pipe.worker().invalidate_cache(Some(tiles));
        self.pipe.mark_edited(tiles);
        let queued = self.manager.streamer_mut().refresh(tiles);
        self.pipe.force_replan();
        self.classes_stale = true;
        self.prefetch_closure_stale = true;
        queued
    }

    /// The chunk field the frame counts its stand-in share against (STRM-039); `None` counts nothing.
    pub fn set_chunk_view(&mut self, view: Option<Arc<dyn crate::chunkshare::ChunkView>>) {
        self.chunk_view = view;
    }

    /// Set the settle window of the refresh pre-emption in milliseconds of the injected clock (STRM-031, ADR 0072): a tile that edits keep
    /// re-keying is refreshed once, after the last of them has been quiet for this long. 0 (the default) holds nothing.
    pub fn set_refresh_settle(&mut self, ms: u64) {
        self.manager.streamer_mut().set_refresh_settle(ms);
    }

    /// Set the preview of the edit in flight (REND-033, from `preview_delta`); an empty list ends it. Call after [`ManagedLoop::edit`].
    pub fn set_preview(&mut self, stamps: Vec<crate::preview::PreviewStamp>) {
        self.pipe.set_preview(stamps);
    }

    /// Set the transient preview (VIEW-036, REND-033): the stamps of something being made, such as a brush stroke under the pointer, shown on
    /// every node they touch although no tile is stale. An empty list ends it.
    pub fn set_transient_preview(&mut self, stamps: Vec<crate::preview::PreviewStamp>) {
        self.pipe.set_transient_preview(stamps);
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

    /// Write the roots (the tiles of the render floor, loaded by [`planet_streaming::Streamer::load_roots_blocking`]) to the sink now,
    /// before frame 0, outside the per-frame cap (STRM-001, ADR 0064): the start-up load is a step of the body and not a measured frame, so
    /// a harness that calls this finds no root upload in its first frame. A caller that does not call it gets them in the first frame as
    /// before. Only tiles at or above the floor are written; any other change stays for the first frame.
    pub fn upload_roots(&mut self, sink: &mut dyn TileSink) -> Result<RootUpload, String> {
        let floor = self.manager.streamer().floor();
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
                    let (Some(mesh), Some(slot)) = (streamer.get(tile), streamer.slot_of(tile)) else { continue };
                    if self.slots.get(&tile) == Some(&slot) && self.resident.get(&tile).is_some_and(|r| r.mesh_key == mesh.key()) {
                        continue;
                    }
                    self.pending.insert(tile);
                }
            }
        }
        let mut roots: Vec<TileId> = self.pending.iter().copied().filter(|t| t.level() <= floor && streamer.is_resident(*t)).collect();
        roots.sort();
        let mut batch = Vec::with_capacity(roots.len());
        for tile in &roots {
            let (Some(mesh), Some(slot)) = (streamer.get(*tile), streamer.slot_of(*tile)) else { continue };
            batch.push((*tile, mesh, slot));
        }
        let refs: Vec<(&CompactTile, SlotRef)> = batch.iter().map(|(_, mesh, slot)| (&**mesh, *slot)).collect();
        let stats = sink.upload_batch(&refs)?;
        let mut bytes = 0u64;
        for (tile, mesh, slot) in &batch {
            self.pending.remove(tile);
            self.resident.insert(*tile, ResidentTile { mesh_key: mesh.key(), origin: mesh.origin() });
            self.slots.insert(*tile, *slot);
            bytes += sink.upload_bytes(mesh);
        }
        Ok(RootUpload { uploaded: batch.len() as u64, bytes, calls: stats.calls })
    }

    /// Begin the next frame for `req`; never waits. See the module documentation for the order of the three steps.
    pub fn begin_frame(&mut self, req: &TerrainRequest, sink: &mut dyn TileSink) -> Result<ManagedFrame, String> {
        let t = Instant::now();
        // The roots of the draw set are the tiles of the streamer's render floor (LOD-011): no ancestor above it is requested.
        let floor = self.manager.streamer().floor();
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
        let (mut uploaded, mut upload_bytes, mut upload_calls) = (0u64, 0u64, 0u64);
        let streamer = self.manager.streamer_mut();
        let changes = streamer.take_changes();
        for change in changes {
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
                self.classes.get_or_insert_with(Classes::default).fill(&self.wants, floor);
                self.classes_stale = false;
                self.class_rebuilds += 1;
            }
            if self.prefetch_closure_stale {
                self.prefetch_closure.clear();
                for &(tile, _) in &self.wants.prefetch {
                    let mut t = Some(tile);
                    while let Some(x) = t {
                        if !self.prefetch_closure.insert(x) {
                            break;
                        }
                        t = x.parent().filter(|_| x.level() > floor);
                    }
                }
                self.prefetch_closure_stale = false;
                self.prefetch_rebuilds += 1;
            }
            let classes = self.classes.as_ref().expect("classes");
            let residency = self.wants_residency.as_deref();
            let prefetch = &self.prefetch_closure;
            let mut waiting = std::mem::take(&mut self.waiting_scratch);
            waiting.clear();
            self.pending.retain(|t| streamer.is_resident(*t));
            for &tile in &self.pending {
                let Some(mesh) = streamer.get(tile) else { continue };
                let (class, priority) = if let Some(p) = classes.draw.get(&tile) {
                    (UploadClass::Draw, *p)
                } else if tile.level() <= floor {
                    (UploadClass::Draw, 0.0)
                } else if residency.is_some_and(|r| r.contains(tile)) {
                    (UploadClass::Residency, 0.0)
                } else if prefetch.contains(&tile) {
                    (UploadClass::Prefetch, 0.0)
                } else {
                    (UploadClass::Other, 0.0)
                };
                waiting.push(Waiting { tile, class, priority, bytes: sink.upload_bytes(&mesh) });
            }
            sort_waiting(&mut waiting);
            // The start-up load of the roots is not a steady frame (STRM-001, a declared exception to the cap of STRM-006): every
            // root that waits is written before the frame is drawn, whatever its bytes. They sort first (the draw class, the
            // lowest level); the cap and the one-tile rule then govern the tiles behind them, the first steady frame's.
            let roots = waiting.iter().take_while(|w| w.tile.level() <= floor).count();
            let take = roots + upload_count(&waiting[roots..], self.budget.allowance(self.last_node_bytes));
            // The frame's tiles in upload order, written as one batch (a sink that coalesces adjacent slots makes one call per run).
            let mut batch = std::mem::take(&mut self.batch_scratch);
            batch.clear();
            for w in &waiting[..take] {
                self.pending.remove(&w.tile);
                let (Some(mesh), Some(slot)) = (streamer.get(w.tile), streamer.slot_of(w.tile)) else { continue };
                batch.push((w.tile, mesh, slot, w.bytes));
            }
            let refs: Vec<(&CompactTile, SlotRef)> = batch.iter().map(|(_, mesh, slot, _)| (&**mesh, *slot)).collect();
            let stats = sink.upload_batch(&refs)?;
            upload_calls += stats.calls;
            for (tile, mesh, slot, bytes) in &batch {
                self.resident.insert(*tile, ResidentTile { mesh_key: mesh.key(), origin: mesh.origin() });
                self.slots.insert(*tile, *slot);
                uploaded += 1;
                upload_bytes += bytes;
            }
            batch.clear();
            self.batch_scratch = batch;
            self.waiting_scratch = waiting;
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
            upload_calls,
            degraded,
            draw_replaced,
            budget_failure,
            prefetch: self.wants.prefetch.len(),
            cancelled_prefetch: stats.cancelled_prefetch - self.last.cancelled_prefetch,
            chunk: self
                .chunk_view
                .as_deref()
                .map(|v| crate::chunkshare::ChunkShare::of(pipeline.draw.plan.nodes.iter().map(|n| n.tile), v))
                .unwrap_or_default(),
        };
        self.last = stats;
        Ok(ManagedFrame { pipeline, counts, scheduler_ms, upload_ms })
    }

    /// The wants follow the newest delivered plan (D, in plan order) and R (distance); rebuilt only when either changed.
    /// The prefetch wants are the worker's set for the newest R, capped by the free share (`free_slots` of the main pool).
    fn refresh_wants(&mut self, free_slots: usize, predicted: bool) {
        let plan_seq = self.pipe.current().map(|c| c.seq);
        if plan_seq != self.wants_plan {
            // In the list the last delivery left: no allocation of a draw set's size per delivered plan.
            self.wants.draw.clear();
            if let Some(c) = self.pipe.current() {
                self.wants.draw.extend(c.plan.nodes.iter().enumerate().map(|(i, n)| (n.tile, i as f64)));
            }
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
                    a.tiles.len() == b.tiles.len()
                        && a.tiles
                            .iter()
                            .zip(&b.tiles)
                            .all(|(x, y)| x.tile == y.tile && x.leaf == y.leaf && x.distance_m.to_bits() == y.distance_m.to_bits())
                }
            }
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.wants.residency = residency.as_ref().map(|r| r.wants()).unwrap_or_default();
            self.extra.scanned += self.wants.residency.len() as u64;
            debug_assert!(
                residency.as_ref().is_none_or(|r| r.is_sorted_and_closed()),
                "the residency class is ranked by lookup in R, which must be sorted and closed under parents"
            );
            self.wants_residency = residency;
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
                self.prefetch_closure_stale = true;
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
            self.prefetch_closure_stale = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::Face;

    /// The draw class as the definition says: every draw tile with all its ancestors, a tile with the lowest priority of the draw
    /// tiles at or below it.
    fn reference(wants: &Wants) -> HashMap<TileId, f64> {
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
        draw
    }

    // spec: REND-018, STRM-006
    #[test]
    fn the_draw_class_with_early_stopping_walks_equals_the_definition_for_any_order_of_priorities() {
        struct Rng(u64);
        impl Rng {
            fn next(&mut self, n: u64) -> u64 {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                self.0 % n
            }
            fn tile(&mut self, level: u8) -> TileId {
                let side = 1u64 << level;
                TileId::new(Face::ALL[self.next(6) as usize], level, self.next(side) as u32, self.next(side) as u32).unwrap()
            }
        }
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        for round in 0..40 {
            let level = 3 + (round % 6) as u8;
            let mut wants = Wants::default();
            for k in 0..200 {
                // Ascending priorities (the plan order), shuffled ones, and ties.
                let p = match round % 3 {
                    0 => k as f64,
                    1 => rng.next(1000) as f64,
                    _ => rng.next(5) as f64,
                };
                wants.draw.push((rng.tile(level), p));
            }
            assert_eq!(Classes::of(&wants).draw, reference(&wants), "round {round}");
        }
    }
}
