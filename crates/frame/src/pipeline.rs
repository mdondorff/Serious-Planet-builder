//! Frame pipelining (STRM-016, ADR 0032 point 5, optimization review item A): the loop never waits for a plan or for the
//! GPU of the frame it just submitted. Everything that decides *what* a pipelined frame draws is here, GPU-free (CON-03):
//!
//! - [`FramesInFlight`]: the bound on submitted-but-unfinished frames and the count the slot reuse rule follows.
//! - [`PlanWorker`]: plans are computed on a spawner (a thread pool, or a manual spawner in tests), one at a time, newest
//!   request wins; a plan is a pure function of its request, so the plan made for a camera does not depend on timing or on
//!   the thread count (CON-14).
//! - [`draw_plan`]: the plan a frame draws, a pure function of the frame's camera, the newest completed plan and the
//!   resident set: the plan rebased to the camera, every node that is not resident replaced by its deepest resident
//!   ancestor, or, when there is no usable plan that covers the frame's view (the first frame, after a jump, a turn that outruns
//!   the plan), the cover of the view ([`cover_selection_bounded`]: the plan's own selection with "resident" as the condition
//!   for descending, so with R resident it equals the plan of the camera).
//! - [`PlanPipeline`]: the per-frame glue of the three.
//!
//! The stale-plan rule: frame N draws the newest completed plan if it is usable for frame N's camera ([`plan_usable`]:
//! the camera moved at most [`STALE_MOVE_FRACTION`] of the finest node's split distance and turned at most
//! [`STALE_ROTATION_RAD`] since the plan's camera, same frame size, view and grid) and it covers the frame's view
//! ([`plan_covers_view`]), otherwise the cover. A plan that is usable but made for an earlier camera is drawn with its nodes and
//! the camera of frame N (origins and matrix rebased).
//! Once the camera has been still for as long as a plan takes, the frame draws the synchronous frame's plan, node for
//! node.

use crate::lod::{
    child_index_toward, cover_selection_masked, plan_covers_view, quadrant_bit, LodParams, SelectionCache, TileBounds, ALL_QUADRANTS,
};
use crate::plan::{face_mapping_by_id, PlanError, CLEAR_COLOR, HEIGHT_RANGE};
use crate::prefetch::prefetch_set;
use crate::preview::PreviewStamp;
use crate::residency::{residency_set_field, ResidencySet};
use crate::terrain::{node_color, plan_terrain_threaded, TerrainDraw, TerrainPlan, TerrainRequest};
use planet_core::cube::FaceMapping;
use planet_core::{PlanetFixed, TileId};
use planet_generators::height::HeightField;
use planet_streaming::Spawner;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

/// Frames the loop may have submitted and not yet seen finish. The slot pools of the renderer retire a released slot this
/// many frames later ([`FramesInFlight::slot_retire_frames`]), so a slot is never reused while a frame that read it runs.
pub const MAX_FRAMES_IN_FLIGHT: u32 = 2;
/// A plan stays usable while the camera is within this fraction of the finest node's split distance of the plan's camera.
pub const STALE_MOVE_FRACTION: f64 = 0.25;
/// ... and while the view and up directions are within this angle (0.5 degrees) of the plan's.
pub const STALE_ROTATION_RAD: f64 = 0.008_726_646_259_971_648;

/// The count of frames in flight (STRM-016): submitted minus finished, never above the bound.
#[derive(Clone, Debug)]
pub struct FramesInFlight {
    max: u32,
    submitted: u64,
    retired: u64,
    peak: u32,
    waits: u64,
}

impl FramesInFlight {
    pub fn new(max: u32) -> Self {
        assert!(max >= 1, "at least one frame in flight");
        Self { max, submitted: 0, retired: 0, peak: 0, waits: 0 }
    }

    pub fn max(&self) -> u32 {
        self.max
    }

    pub fn in_flight(&self) -> u32 {
        (self.submitted - self.retired) as u32
    }

    pub fn is_full(&self) -> bool {
        self.in_flight() >= self.max
    }

    /// Frames submitted so far.
    pub fn submitted(&self) -> u64 {
        self.submitted
    }

    /// The most frames that were in flight at once.
    pub fn peak(&self) -> u32 {
        self.peak
    }

    /// Times the loop had to wait for the GPU because the bound was reached (the only wait there is).
    pub fn waits(&self) -> u64 {
        self.waits
    }

    /// Record a submission; the caller has made room first. Panics when the bound would be exceeded: that is a loop bug.
    pub fn submit(&mut self) -> u64 {
        assert!(!self.is_full(), "a frame was submitted with {} frames in flight (bound {})", self.in_flight(), self.max);
        self.submitted += 1;
        self.peak = self.peak.max(self.in_flight());
        self.submitted
    }

    /// The GPU reports `retired_total` frames finished (a running total; never more than were submitted).
    pub fn retire_to(&mut self, retired_total: u64) {
        self.retired = self.retired.max(retired_total.min(self.submitted));
    }

    /// The loop waited for the GPU because the bound was reached.
    pub fn note_wait(&mut self) {
        self.waits += 1;
    }

    /// Frames after its release a slot may be reused: the bound (the renderer's pools take it as `retire_frames`).
    pub fn slot_retire_frames(&self) -> u64 {
        u64::from(self.max)
    }
}

/// What the loop knows of a tile it has uploaded: the key of its mesh and its origin (the mesh's `origin`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResidentTile {
    pub mesh_key: u64,
    pub origin: PlanetFixed,
}

/// The tiles a frame may draw.
pub trait ResidentSet {
    fn resident(&self, tile: TileId) -> Option<ResidentTile>;
}

impl ResidentSet for HashMap<TileId, ResidentTile> {
    fn resident(&self, tile: TileId) -> Option<ResidentTile> {
        self.get(&tile).copied()
    }
}

/// How a drawn plan relates to the plan of the synchronous loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawKind {
    /// The plan made for exactly this request, every node resident: the synchronous frame.
    Exact,
    /// A plan made for an earlier camera (rebased to this one) and/or with nodes replaced by resident ancestors.
    Stale,
    /// No usable plan: the cover of the view from the resident set.
    Cover,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrawPlan {
    pub plan: TerrainPlan,
    pub kind: DrawKind,
    /// Plan nodes drawn as an ancestor instead (their tile was not resident), and nodes drawn from a resident version of another key
    /// (stale while revalidating, REND-032).
    pub substituted: usize,
    /// Plan nodes with nothing resident to show, not even a root: zero whenever the roots are resident (STRM-002).
    pub blank: usize,
    /// The preview stamps the frame draws (REND-033): the list given to [`draw_plan_preview`], which the renderer evaluates on the nodes
    /// flagged `preview`. Empty for a frame without an edit in flight.
    pub preview: Vec<PreviewStamp>,
    /// How many of the first stamps of `preview` are the delta of the edit in flight; the rest are the transient preview (VIEW-036).
    pub preview_delta_len: usize,
    /// The radius of the body the preview stamps lie on (the request's), for the renderer to place their anchors.
    pub preview_radius_m: f64,
}

fn draw_of(req: &TerrainRequest, tile: TileId, r: ResidentTile) -> TerrainDraw {
    let rel = r.origin.0 - req.camera.position.0;
    TerrainDraw {
        tile,
        level: tile.level(),
        origin: r.origin,
        camera_relative_origin: [rel.x as f32, rel.y as f32, rel.z as f32],
        morph: 0.0,
        // A tile drawn for its own level or as a stand-in for finer ones does not morph (like a root): its geometry is its own.
        morph_start_m: 1.0e30,
        morph_end_m: 1.0e30,
        color: node_color(req.view, tile),
        mesh_key: r.mesh_key,
        preview: false,
        transient_only: false,
        quadrants: ALL_QUADRANTS,
    }
}

/// Whether `plan`, made for another camera, still serves `req` (the stale-plan rule of the module documentation).
pub fn plan_usable(plan: &TerrainPlan, req: &TerrainRequest) -> bool {
    let Some(finest) = plan.nodes.iter().map(|n| n.level).max() else { return false };
    let (a, b) = (&plan.camera, &req.camera);
    let moved = (b.position.0 - a.position.0).length();
    let cos_limit = libm::cos(STALE_ROTATION_RAD);
    plan.size == req.size
        && plan.view == req.view
        && plan.cells == req.lod.cells
        && a.fov_y_rad == b.fov_y_rad
        && a.aspect == b.aspect
        && a.near_m == b.near_m
        && moved <= STALE_MOVE_FRACTION * req.lod.split_distance(finest)
        && a.forward.normalized().dot(b.forward.normalized()) >= cos_limit
        && a.up.normalized().dot(b.up.normalized()) >= cos_limit
}

/// The plan frame `req` draws (STRM-016) with no cached tile bounds: [`draw_plan_with`] over empty bounds.
pub fn draw_plan(
    req: &TerrainRequest,
    base: Option<(&TerrainRequest, &TerrainPlan)>,
    resident: &dyn ResidentSet,
) -> Result<DrawPlan, PlanError> {
    draw_plan_with(req, base, resident, &TileBounds::default())
}

/// The plan frame `req` draws (STRM-016). `base` is the newest completed plan with the request it was made for, if any is
/// usable; `resident` is what the renderer holds; `bounds` the tile bounds of the newest residency set (the cover reads them
/// instead of evaluating a field). A pure function of its arguments, and no height field is evaluated:
/// - with a `base` made for exactly `req` and all its nodes resident, the result is that plan unchanged;
/// - with a `base`, each node whose tile is resident with another mesh key is drawn from that resident version (REND-032, an edit's
///   refresh is on the way) and each node whose tile is not resident is replaced by its deepest resident ancestor
///   (the coarser tile wins over any resident descendant it covers, so no two drawn tiles overlap) and the camera of `req` is
///   put in (origins and matrix), so the plan's nodes follow the camera; a `base` made for another camera is drawn only if it
///   covers the frame's view ([`plan_covers_view`]: every tile of the frame's own cover is drawn by it, or filled by finer
///   tiles, and no node is blank), otherwise the frame draws the cover;
/// - without a `base`, the nodes are [`cover_selection_bounded`] of the resident set: the plan's own selection for the frame's
///   camera with "resident" as the condition for descending.
pub fn draw_plan_with(
    req: &TerrainRequest,
    base: Option<(&TerrainRequest, &TerrainPlan)>,
    resident: &dyn ResidentSet,
    bounds: &TileBounds,
) -> Result<DrawPlan, PlanError> {
    draw_plan_preview(req, base, resident, bounds, &[])
}

/// [`draw_plan_with`] for a frame with an edit in flight (REND-033): a node drawn from a resident version of another key (stale while
/// revalidating) is flagged `preview` when the reach of a stamp in `preview` touches the tile, and the list goes to the draw plan. Only
/// those nodes: a stand-in ancestor, a node whose resident key is the plan's, and a node away from every stamp are not flagged. An empty
/// list is [`draw_plan_with`] exactly.
pub fn draw_plan_preview(
    req: &TerrainRequest,
    base: Option<(&TerrainRequest, &TerrainPlan)>,
    resident: &dyn ResidentSet,
    bounds: &TileBounds,
    preview: &[PreviewStamp],
) -> Result<DrawPlan, PlanError> {
    draw_plan_edited(req, base, resident, bounds, preview, &BTreeSet::new())
}

/// [`draw_plan_preview`] for the frames between an edit and the first plan made after it (REND-033): the plan in hand still carries the
/// mesh keys of the unedited world, so a resident tile that `edited` names (the footprint find of the edit) matches its key although its
/// content is out of date. Such a tile is drawn as it is like a stale one, counts as substituted (the frame is not exact) and is flagged
/// when a stamp touches it. An empty set is [`draw_plan_preview`] exactly.
pub fn draw_plan_edited(
    req: &TerrainRequest,
    base: Option<(&TerrainRequest, &TerrainPlan)>,
    resident: &dyn ResidentSet,
    bounds: &TileBounds,
    preview: &[PreviewStamp],
    edited: &BTreeSet<TileId>,
) -> Result<DrawPlan, PlanError> {
    draw_plan_transient(req, base, resident, bounds, preview, edited, &[])
}

/// [`draw_plan_edited`] with a transient preview (VIEW-036): stamps that are not the delta of an edit in flight but something being made
/// (a brush stroke under the pointer), so that no tile is stale for them. Every node drawn from a resident tile, stale or exact, whose
/// cap a transient stamp touches is flagged `preview`, and the draw plan carries the delta's stamps and then the transient ones, at most
/// [`crate::preview::MAX_PREVIEW_STAMPS`] (the transient ones are kept first). The frame's kind is not changed by them: its tiles are what
/// they were. An empty transient list is [`draw_plan_edited`] exactly.
pub fn draw_plan_transient(
    req: &TerrainRequest,
    base: Option<(&TerrainRequest, &TerrainPlan)>,
    resident: &dyn ResidentSet,
    bounds: &TileBounds,
    preview: &[PreviewStamp],
    edited: &BTreeSet<TileId>,
    transient: &[PreviewStamp],
) -> Result<DrawPlan, PlanError> {
    // The delta's stamps are at most `MAX_DELTA_STAMPS` (`preview_delta` keeps whole units to that) and a transient preview at most
    // `BRUSH_CHAIN_STAMPS`, so both fit the table whole; a longer list from another caller is cut at the end, the transient ones kept.
    let max = crate::preview::MAX_PREVIEW_STAMPS;
    let transient = &transient[..transient.len().min(max)];
    let preview = &preview[..preview.len().min(max - transient.len())];
    let all: Vec<PreviewStamp> = preview.iter().chain(transient).copied().collect();
    let mut transient_flags = false;
    let map = face_mapping_by_id(&req.face_mapping).ok_or_else(|| PlanError::UnknownFaceMapping(req.face_mapping.clone()))?;
    let mut shown: BTreeMap<TileId, TerrainDraw> = BTreeMap::new();
    let (mut substituted, mut blank) = (0usize, 0usize);
    let mut kind = DrawKind::Cover;
    let mut cover: Option<Vec<(TileId, u8)>> = None;
    // The quadrants each stand-in ancestor draws, from the plan nodes it stands in for.
    let mut stand_in: BTreeMap<TileId, u8> = BTreeMap::new();
    if let Some((base_req, plan)) = base {
        for d in &plan.nodes {
            if resident.resident(d.tile).is_some_and(|r| r.mesh_key == d.mesh_key) && !edited.contains(&d.tile) {
                let rel = d.origin.0 - req.camera.position.0;
                let flagged = previewed(transient, map.as_ref(), d.tile, req.lod.radius_m);
                transient_flags |= flagged;
                // An exact tile already holds the edit in flight: only the transient stamps are applied to it.
                shown.insert(
                    d.tile,
                    TerrainDraw {
                        camera_relative_origin: [rel.x as f32, rel.y as f32, rel.z as f32],
                        preview: flagged,
                        transient_only: flagged,
                        ..*d
                    },
                );
                continue;
            }
            // Stale while revalidating (REND-032): a resident tile of another version (an edit changed what it reads and its refresh is
            // on the way) is drawn as it is, not replaced by an ancestor, so an edited place never goes coarse while it reloads.
            if let Some(r) = resident.resident(d.tile) {
                substituted += 1;
                let mut node = draw_of(req, d.tile, r);
                let by_delta = previewed(preview, map.as_ref(), d.tile, req.lod.radius_m);
                let by_transient = previewed(transient, map.as_ref(), d.tile, req.lod.radius_m);
                node.preview = by_delta || by_transient;
                // A stale tile under both gets the delta's stamps and the transient ones; under the transient ones alone, only those.
                node.transient_only = by_transient && !by_delta;
                transient_flags |= by_transient;
                shown.insert(d.tile, node);
                continue;
            }
            let mut up = d.tile.parent();
            let found = loop {
                match up {
                    Some(a) => match resident.resident(a) {
                        Some(r) => break Some((a, r)),
                        None => up = a.parent(),
                    },
                    None => break None,
                }
            };
            match found {
                Some((a, r)) => {
                    substituted += 1;
                    shown.insert(a, draw_of(req, a, r));
                    // The stand-in draws the quadrant that holds the missing node (STRM-016): the resident nodes beside it stay.
                    *stand_in.entry(a).or_insert(0) |= quadrant_bit(child_index_toward(a, d.tile));
                }
                None => blank += 1,
            }
        }
        // A coarser stand-in covers the finer tiles of the plan that lie under the quadrants it draws.
        for (a, mask) in &stand_in {
            if let Some(node) = shown.get_mut(a) {
                node.quadrants = *mask;
            }
        }
        let tiles: Vec<TileId> = shown.keys().copied().collect();
        for t in tiles {
            let mut up = t.parent();
            while let Some(a) = up {
                let covers = match stand_in.get(&a) {
                    Some(mask) => shown.contains_key(&a) && mask & quadrant_bit(child_index_toward(a, t)) != 0,
                    None => shown.contains_key(&a),
                };
                if covers {
                    shown.remove(&t);
                    break;
                }
                up = a.parent();
            }
        }
        // The plan of exactly this camera serves it by construction; a plan of another camera must cover the frame's view.
        let serves = base_req == req || {
            let c = cover.insert(view_cover(req, map.as_ref(), resident, bounds));
            let tiles: Vec<TileId> = c.iter().map(|(t, _)| *t).collect();
            // A tile drawn over some quadrants only draws those children, not the whole tile (STRM-016): the test sees its children.
            let drawn: BTreeSet<TileId> = shown
                .iter()
                .flat_map(|(t, n)| {
                    if n.quadrants == ALL_QUADRANTS {
                        vec![*t]
                    } else {
                        let kids = t.children().expect("a tile drawn over some quadrants has children");
                        (0..4).filter(|k| n.quadrants & quadrant_bit(*k) != 0).map(|k| kids[k]).collect()
                    }
                })
                .collect();
            blank == 0 && plan_covers_view(map.as_ref(), &req.camera, &req.lod, bounds, &tiles, &drawn)
        };
        if serves {
            kind = if base_req == req && substituted == 0 && blank == 0 { DrawKind::Exact } else { DrawKind::Stale };
        } else {
            shown.clear();
            (substituted, blank) = (0, 0);
        }
    }
    if kind == DrawKind::Cover {
        for (t, quadrants) in cover.unwrap_or_else(|| view_cover(req, map.as_ref(), resident, bounds)) {
            let r = resident.resident(t).expect("the cover holds resident tiles only");
            shown.insert(t, TerrainDraw { quadrants, ..draw_of(req, t, r) });
        }
    }
    let nodes: Vec<TerrainDraw> = shown.into_values().collect();
    let plan = match (kind, base) {
        // The synchronous frame's plan, untouched (and so equal to it bit for bit).
        (DrawKind::Exact, Some((_, p))) if !transient_flags => p.clone(),
        _ => TerrainPlan {
            size: req.size,
            view: req.view,
            camera: req.camera,
            view_projection: req.camera.view_projection(),
            clear_color: CLEAR_COLOR,
            height_range: HEIGHT_RANGE,
            cells: req.lod.cells,
            shore_ramp_per_m: req.lod.shore_ramp_per_m() as f32,
            nodes,
            lit: None,
            water: None,
        },
    };
    Ok(DrawPlan { plan, kind, substituted, blank, preview: all, preview_delta_len: preview.len(), preview_radius_m: req.lod.radius_m })
}

/// Whether the reach of a preview stamp touches the cap of `tile` (REND-033), with the halo the stamp stage declares its items with.
fn previewed(preview: &[PreviewStamp], map: &dyn FaceMapping, tile: TileId, radius_m: f64) -> bool {
    if preview.is_empty() {
        return false;
    }
    let cap = planet_world_def::footprint::tile_cap(map, tile, planet_generators::stamp::STAGE_HALO);
    preview.iter().any(|s| planet_world_def::footprint::touches(cap, s.dir, s.reach_m, radius_m))
}

/// The cover of the frame's view over the resident set (the tiles of [`cover_selection_bounded`]).
fn view_cover(req: &TerrainRequest, map: &dyn FaceMapping, resident: &dyn ResidentSet, bounds: &TileBounds) -> Vec<(TileId, u8)> {
    cover_selection_masked(map, &req.camera, &req.lod, &|t| resident.resident(t).map(|r| r.origin), bounds)
}

/// What a worker plans over: a height field that is `Send + Sync + 'static`, or anything that can build one inside the job
/// (a world whose field borrows its bake). `plan` is [`plan_terrain_threaded`] of the request over that field.
pub trait PlanField: Send + Sync + 'static {
    fn plan(&self, req: &TerrainRequest, cache: &mut SelectionCache, threads: usize) -> Result<TerrainPlan, PlanError>;

    /// The residency set R of `req` (STRM-015): the position-only selection of the request's camera position, with the
    /// request's LOD parameters, over the same field. `cache` is the one the plan used, so every tile geometry and height
    /// volume the plan already computed is a hit and R needs no origin height or mesh key at all (a resident tile carries its
    /// own origin and key); what R adds is the volumes of the tiles outside the frustum, once per tile.
    fn residency(&self, req: &TerrainRequest, cache: &mut SelectionCache) -> Result<ResidencySet, PlanError>;

    /// The revision of the authored items the field reads (STRM-027), 0 when it tracks none: R is selected again when it changes.
    fn item_revision(&self) -> u64 {
        0
    }
}

impl<T: HeightField + Send + Sync + 'static> PlanField for T {
    fn plan(&self, req: &TerrainRequest, cache: &mut SelectionCache, threads: usize) -> Result<TerrainPlan, PlanError> {
        plan_terrain_threaded(req, self, cache, threads)
    }

    fn residency(&self, req: &TerrainRequest, cache: &mut SelectionCache) -> Result<ResidencySet, PlanError> {
        residency_of(req, self, cache)
    }

    fn item_revision(&self) -> u64 {
        HeightField::item_revision(self)
    }
}

/// R of `req` over `field` (what [`PlanField::residency`] computes; for fields that build their `HeightField` inside the job).
pub fn residency_of(req: &TerrainRequest, field: &dyn HeightField, cache: &mut SelectionCache) -> Result<ResidencySet, PlanError> {
    let map = face_mapping_by_id(&req.face_mapping).ok_or_else(|| PlanError::UnknownFaceMapping(req.face_mapping.clone()))?;
    Ok(residency_set_field(map.as_ref(), req.camera.position, field, &req.lod, cache))
}

/// What R depends on: the camera position, the LOD parameters, the face mapping and the seed. A request that differs from the
/// last one only in its view direction (or frame setup, which R never reads) has the same key, and R is reused.
#[derive(Clone, Debug, PartialEq)]
struct ResidencyKey {
    position: PlanetFixed,
    lod: LodParams,
    face_mapping: String,
    seed: u64,
    /// The items the field reads: an edit changes the height bounds that R holds (STRM-027).
    item_revision: u64,
}

impl ResidencyKey {
    fn of(req: &TerrainRequest, item_revision: u64) -> Self {
        Self { position: req.camera.position, lod: req.lod.clone(), face_mapping: req.face_mapping.clone(), seed: req.seed, item_revision }
    }
}

/// R made for the request of frame `seq`.
#[derive(Clone, Debug)]
pub struct ResidencyOutcome {
    pub seq: u64,
    pub position: PlanetFixed,
    pub residency: Arc<ResidencySet>,
    /// Worker time spent on it (0 when reused).
    pub ms: f64,
    /// The position and parameters equal the previous R's: nothing was selected (a rotation).
    pub reused: bool,
    /// The prefetch set made with this R (STRM-018): R at the predicted position minus this R, coarse first, nearer first.
    /// Empty when no position was predicted (a stationary or rotating camera, a teleport).
    pub prefetch: Arc<Vec<(TileId, f64)>>,
}

/// A completed plan: the request it was made for (`seq` is the frame that asked), the plan and the time it took on its worker.
#[derive(Clone, Debug)]
pub struct PlanOutcome {
    pub seq: u64,
    pub request: TerrainRequest,
    pub plan: Arc<TerrainPlan>,
    pub plan_ms: f64,
}

/// A plan the worker could not make: why, and the request it was asked for (so a failure of an older request than the frame's can be told
/// from one of the frame's own).
#[derive(Clone, Debug, PartialEq)]
pub struct PlanFailure {
    pub request: TerrainRequest,
    pub message: String,
}

impl std::fmt::Display for PlanFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Default)]
struct WorkerState {
    running: bool,
    /// Set when the pipeline is dropped: a job that has not started does not run, and a running one does not start its
    /// residency set, its prefetch set or the next pending plan (STRM-005: teardown waits only for the job in flight).
    closed: bool,
    pending: Option<(u64, TerrainRequest, Option<PlanetFixed>)>,
    done: Option<Result<PlanOutcome, PlanFailure>>,
    /// The newest R, delivered after the plan of the same job (or with it when the position did not change).
    residency_done: Option<Result<ResidencyOutcome, String>>,
    last_residency: Option<(ResidencyKey, Arc<ResidencySet>)>,
    submitted: u64,
    completed: u64,
    superseded: u64,
    residencies: u64,
    residencies_reused: u64,
}

/// What the worker counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkerCounts {
    pub submitted: u64,
    pub completed: u64,
    /// Requests replaced by a newer one before they started.
    pub superseded: u64,
    /// R sets selected (a changed position or parameters) and R sets reused (the same position: a rotation).
    pub residencies: u64,
    pub residencies_reused: u64,
}

/// Computes plans off the frame path (STRM-016): one plan at a time on `spawner`; a request that arrives while one runs
/// waits in a single slot where a newer request replaces it. The origin heights of nodes the cache has not seen are
/// evaluated on `threads` threads inside the job. Each plan is [`plan_terrain_threaded`] of its request: identical for any
/// thread count, spawner and timing.
pub struct PlanWorker {
    spawner: Arc<dyn Spawner>,
    field: Arc<dyn PlanField>,
    cache: Arc<Mutex<SelectionCache>>,
    threads: usize,
    state: Arc<Mutex<WorkerState>>,
}

impl PlanWorker {
    pub fn new(spawner: Arc<dyn Spawner>, field: Arc<dyn PlanField>, threads: usize) -> Self {
        Self { spawner, field, cache: Arc::new(Mutex::new(SelectionCache::new())), threads, state: Arc::default() }
    }

    /// Forget the cached origin heights, mesh keys and volumes of `tiles` (an item edit changed what they read, STRM-026), or of every
    /// tile with `None`, so the next plan computes them again.
    pub fn invalidate_cache(&self, tiles: Option<&[TileId]>) {
        let mut c = self.cache.lock().expect("selection cache");
        match tiles {
            Some(t) => {
                c.invalidate_tiles(t);
            }
            None => c.clear(),
        }
    }

    /// Ask for the plan of `req`. Never blocks and never plans on the calling thread.
    pub fn submit(&self, seq: u64, req: TerrainRequest) {
        self.submit_predicted(seq, req, None);
    }

    /// As [`PlanWorker::submit`], and with `predicted` (the position the camera will reach, STRM-018) the worker also
    /// computes the prefetch set after R: R at `predicted` minus R of the request's position. `None` yields an empty set.
    pub fn submit_predicted(&self, seq: u64, req: TerrainRequest, predicted: Option<PlanetFixed>) {
        let start = {
            let mut s = self.state.lock().expect("worker state");
            s.submitted += 1;
            if s.running {
                if s.pending.is_some() {
                    s.superseded += 1;
                }
                s.pending = Some((seq, req, predicted));
                None
            } else {
                s.running = true;
                Some((seq, req, predicted))
            }
        };
        if let Some((seq, req, predicted)) = start {
            self.spawn_job(seq, req, predicted);
        }
    }

    fn spawn_job(&self, seq: u64, req: TerrainRequest, predicted: Option<PlanetFixed>) {
        let (state, cache, field, threads, spawner) =
            (Arc::clone(&self.state), Arc::clone(&self.cache), Arc::clone(&self.field), self.threads, Arc::clone(&self.spawner));
        let spawner_again = Arc::clone(&spawner);
        spawner.spawn(Box::new(move || {
            if state.lock().expect("worker state").closed {
                let mut s = state.lock().expect("worker state");
                s.pending = None;
                s.running = false;
                return;
            }
            let started = std::time::Instant::now();
            // A panicking field is an error outcome, not a worker that stays "running" for ever.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut cache = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                field.plan(&req, &mut cache, threads)
            }))
            .unwrap_or_else(|_| Err(PlanError::HeightField("the plan job panicked".into())));
            let plan_ms = started.elapsed().as_secs_f64() * 1000.0;
            let planned = result.is_ok();
            let outcome = result
                .map(|p| PlanOutcome { seq, request: req.clone(), plan: Arc::new(p), plan_ms })
                .map_err(|e| PlanFailure { request: req.clone(), message: e.to_string() });
            {
                let mut s = state.lock().expect("worker state");
                s.completed += 1;
                // Keep the newest completed plan only: the loop wants the newest, and an older one is useless to it.
                // Jobs run one after the other, so a later plan is the newer; an unread error is not overwritten.
                if !matches!(s.done, Some(Err(_))) {
                    s.done = Some(outcome);
                }
            }
            // R follows the plan in the same job, after the plan is out (a frame never waits for it). It shares the plan's
            // cache, and when the position and parameters equal the previous R's (a rotation) nothing is selected at all.
            let residency = (planned && !state.lock().expect("worker state").closed).then(|| {
                let key = ResidencyKey::of(&req, field.item_revision());
                let reuse =
                    state.lock().expect("worker state").last_residency.as_ref().filter(|(k, _)| *k == key).map(|(_, r)| Arc::clone(r));
                let started = std::time::Instant::now();
                let made = match reuse {
                    Some(r) => Ok((r, true)),
                    None => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut cache = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                        field.residency(&req, &mut cache)
                    }))
                    .unwrap_or_else(|_| Err(PlanError::HeightField("the residency job panicked".into())))
                    .map(|r| (Arc::new(r), false)),
                };
                let ms = if matches!(made, Ok((_, false))) { started.elapsed().as_secs_f64() * 1000.0 } else { 0.0 };
                // The prefetch set follows R in the same job, on the same cache: R at the predicted position (the same function
                // of a position that R is), minus R. A tile the cache has seen costs nothing; the rest costs its volume once.
                let made = made.and_then(|(residency, reused)| {
                    let prefetch = match predicted {
                        Some(p) => {
                            if state.lock().expect("worker state").closed {
                                return Err(PlanError::HeightField("closed".into()));
                            }
                            let mut ahead_req = req.clone();
                            ahead_req.camera.position = p;
                            let ahead = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                let mut cache = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                                field.residency(&ahead_req, &mut cache)
                            }))
                            .unwrap_or_else(|_| Err(PlanError::HeightField("the prefetch job panicked".into())))?;
                            prefetch_set(&residency, &ahead)
                        }
                        None => Vec::new(),
                    };
                    Ok(ResidencyOutcome { seq, position: req.camera.position, residency, ms, reused, prefetch: Arc::new(prefetch) })
                });
                (key, made)
            });
            let next = {
                let mut s = state.lock().expect("worker state");
                if let Some((key, made)) = residency {
                    match made {
                        Ok(o) => {
                            if o.reused {
                                s.residencies_reused += 1;
                            } else {
                                s.residencies += 1;
                                s.last_residency = Some((key, Arc::clone(&o.residency)));
                            }
                            if !matches!(s.residency_done, Some(Err(_))) {
                                s.residency_done = Some(Ok(o));
                            }
                        }
                        Err(e) => s.residency_done = Some(Err(e.to_string())),
                    }
                }
                let next = if s.closed { None } else { s.pending.take() };
                if next.is_none() {
                    s.running = false;
                }
                next
            };
            if let Some((seq, req, predicted)) = next {
                let worker = PlanWorker { spawner: spawner_again, field, cache, threads, state };
                worker.spawn_job(seq, req, predicted);
            }
        }));
    }

    /// The newest plan completed since the last call, if any.
    pub fn take(&self) -> Option<Result<PlanOutcome, PlanFailure>> {
        self.state.lock().expect("worker state").done.take()
    }

    /// The newest residency set completed since the last call, if any. It follows the plan of its job (a reused one is
    /// delivered in the same job).
    pub fn take_residency(&self) -> Option<Result<ResidencyOutcome, String>> {
        self.state.lock().expect("worker state").residency_done.take()
    }

    pub fn counts(&self) -> WorkerCounts {
        let s = self.state.lock().expect("worker state");
        WorkerCounts {
            submitted: s.submitted,
            completed: s.completed,
            superseded: s.superseded,
            residencies: s.residencies,
            residencies_reused: s.residencies_reused,
        }
    }

    /// Stop the worker: jobs that have not started do not run and a running job ends after its current computation.
    pub fn close(&self) {
        self.state.lock().expect("worker state").closed = true;
    }

    /// A plan is being computed or waits for its turn.
    pub fn busy(&self) -> bool {
        self.state.lock().expect("worker state").running
    }
}

/// What one pipelined frame found out.
#[derive(Clone, Debug)]
pub struct PipelineFrame {
    pub frame: u64,
    pub draw: DrawPlan,
    /// The frame whose camera the drawn plan was made for (`None` for a cover).
    pub base_seq: Option<u64>,
    /// A plan was delivered by the worker this frame: its request frame and its worker time.
    pub delivered: Option<(u64, f64)>,
    /// R was delivered this frame: its request frame, its worker time and whether it was the previous one reused.
    pub residency_delivered: Option<(u64, f64, bool)>,
    /// A plan was requested for this frame's camera.
    pub requested: bool,
    /// The newest completed plan's nodes with priorities (what the streamer is asked for), usable or not.
    pub wanted: Vec<(TileId, f64)>,
}

/// The per-frame glue (STRM-016): deliver what the worker finished, ask for the plan of this camera, decide what to draw.
pub struct PlanPipeline {
    worker: PlanWorker,
    current: Option<PlanOutcome>,
    residency: Option<ResidencyOutcome>,
    last_requested: Option<TerrainRequest>,
    last_predicted: Option<PlanetFixed>,
    frame: u64,
    /// The preview of the edit in flight (REND-033); empty when there is none.
    preview: Vec<PreviewStamp>,
    /// The transient preview of something being made (VIEW-036); empty when there is none.
    transient: Vec<PreviewStamp>,
    /// The tiles named by the edits since the last plan made after one was delivered, and the request number of the first plan that is
    /// made after the latest edit (a plan with a lower number predates it).
    edited: BTreeSet<TileId>,
    edit_seq: u64,
    /// The plan failures dropped because the request they were made for carried other LOD parameters than the frame's.
    dropped_plan_errors: u64,
}

impl PlanPipeline {
    pub fn new(worker: PlanWorker) -> Self {
        Self {
            worker,
            current: None,
            residency: None,
            last_requested: None,
            last_predicted: None,
            frame: 0,
            preview: Vec::new(),
            transient: Vec::new(),
            edited: BTreeSet::new(),
            edit_seq: 0,
            dropped_plan_errors: 0,
        }
    }

    /// How many plan failures were dropped, not returned, because they were for a request with other LOD parameters than the frame
    /// that received them (the coarse view switched off while its plan was made).
    pub fn dropped_plan_errors(&self) -> u64 {
        self.dropped_plan_errors
    }

    /// An edit named `tiles` (the footprint find, WDEF-007): until a plan made after it is delivered, the frames draw them as stale and
    /// preview them (REND-033), so the preview does not wait for the plan worker.
    pub fn mark_edited(&mut self, tiles: &[TileId]) {
        self.edited.extend(tiles.iter().copied());
        self.edit_seq = self.frame + 1;
    }

    /// Set the preview of the edit in flight (REND-033, from `preview_delta`): the frames that follow flag the nodes drawn from a stale
    /// tile under a stamp and carry the list to the renderer. An empty list ends the preview; the flags end by themselves when the exact
    /// tiles are resident.
    pub fn set_preview(&mut self, preview: Vec<PreviewStamp>) {
        self.preview = preview;
    }

    /// Set the transient preview (VIEW-036): stamps of something being made, flagged on every node they touch whether or not its tile is
    /// stale. An empty list ends it.
    pub fn set_transient_preview(&mut self, stamps: Vec<PreviewStamp>) {
        self.transient = stamps;
    }

    /// An edit changed what the world draws: ask for a new plan at the next frame although the camera did not move (STRM-026).
    pub fn force_replan(&mut self) {
        self.last_requested = None;
    }

    pub fn worker(&self) -> &PlanWorker {
        &self.worker
    }

    /// Frames begun.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// The newest completed plan delivered so far.
    pub fn current(&self) -> Option<&PlanOutcome> {
        self.current.as_ref()
    }

    /// The newest residency set delivered so far (made for the camera position of an earlier or the current plan).
    pub fn residency(&self) -> Option<&ResidencyOutcome> {
        self.residency.as_ref()
    }

    /// Begin frame `self.frame() + 1` for `req`. Never waits: the plan of this camera is only asked for.
    pub fn begin_frame(&mut self, req: &TerrainRequest, resident: &dyn ResidentSet) -> Result<PipelineFrame, String> {
        self.begin_frame_predicted(req, None, resident)
    }

    /// As [`PlanPipeline::begin_frame`], with the position the camera is predicted to reach (STRM-018): the worker computes
    /// the prefetch set for it after R. A changed prediction asks the worker again, as a changed request does.
    pub fn begin_frame_predicted(
        &mut self,
        req: &TerrainRequest,
        predicted: Option<PlanetFixed>,
        resident: &dyn ResidentSet,
    ) -> Result<PipelineFrame, String> {
        self.frame += 1;
        let mut delivered = None;
        match self.worker.take() {
            Some(Ok(o)) => {
                delivered = Some((o.seq, o.plan_ms));
                if o.seq >= self.edit_seq {
                    self.edited.clear();
                }
                if self.current.as_ref().is_none_or(|c| c.seq < o.seq) {
                    self.current = Some(o);
                }
            }
            // A refusal of a request made with other LOD parameters than this frame's (the coarse view was switched off while its plan was
            // being made) is of no use to this frame: it is dropped and counted, not returned. The frame's own refusal is returned.
            Some(Err(f)) if f.request.lod != req.lod => self.dropped_plan_errors += 1,
            Some(Err(f)) => return Err(format!("plan for frame {}: {f}", self.frame)),
            None => {}
        }
        let mut residency_delivered = None;
        match self.worker.take_residency() {
            Some(Ok(o)) => {
                residency_delivered = Some((o.seq, o.ms, o.reused));
                if self.residency.as_ref().is_none_or(|c| c.seq < o.seq) {
                    self.residency = Some(o);
                }
            }
            Some(Err(e)) => return Err(format!("residency set for frame {}: {e}", self.frame)),
            None => {}
        }
        let requested = self.last_requested.as_ref() != Some(req) || self.last_predicted != predicted;
        if requested {
            self.worker.submit_predicted(self.frame, req.clone(), predicted);
            self.last_requested = Some(req.clone());
            self.last_predicted = predicted;
        }
        let base = self.current.as_ref().filter(|c| c.request == *req || plan_usable(&c.plan, req));
        let empty = TileBounds::default();
        let bounds = self.residency.as_ref().map_or(&empty, |r| &r.residency.bounds);
        let draw = draw_plan_transient(
            req,
            base.map(|c| (&c.request, c.plan.as_ref())),
            resident,
            bounds,
            &self.preview,
            &self.edited,
            &self.transient,
        )
        .map_err(|e| e.to_string())?;
        let wanted =
            self.current.as_ref().map(|c| c.plan.nodes.iter().enumerate().map(|(i, n)| (n.tile, i as f64)).collect()).unwrap_or_default();
        Ok(PipelineFrame { frame: self.frame, base_seq: base.map(|c| c.seq), draw, delivered, residency_delivered, requested, wanted })
    }
}

/// The per-frame time columns (TEST-012, ADR 0032 point 5): reported apart so a frame over budget is attributed to its
/// largest part.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameColumns {
    /// Time the worker took for the plan delivered this frame (0 when none was; it ran off the frame path).
    pub plan_ms: f64,
    pub scheduler_ms: f64,
    pub upload_ms: f64,
    pub record_ms: f64,
    pub gpu_ms: Option<f64>,
    pub wall_ms: f64,
}

impl FrameColumns {
    pub const NAMES: [&'static str; 6] = ["plan_ms", "scheduler_ms", "upload_ms", "record_ms", "gpu_ms", "wall_ms"];

    /// The values in [`FrameColumns::NAMES`] order (a missing GPU time is NaN).
    pub fn values(&self) -> [f64; 6] {
        [self.plan_ms, self.scheduler_ms, self.upload_ms, self.record_ms, self.gpu_ms.unwrap_or(f64::NAN), self.wall_ms]
    }

    /// The column of the frame-path parts (scheduler, upload, record, GPU) that is largest: where a slow frame went. The plan
    /// column is not on the path (a worker computes it) and wall time is the sum they are part of, so neither competes.
    pub fn largest(&self) -> &'static str {
        let parts = [
            ("scheduler_ms", self.scheduler_ms),
            ("upload_ms", self.upload_ms),
            ("record_ms", self.record_ms),
            ("gpu_ms", self.gpu_ms.unwrap_or(0.0)),
        ];
        parts.into_iter().fold(("scheduler_ms", f64::MIN), |best, p| if p.1 > best.1 { p } else { best }).0
    }
}

/// Resident records from the field: the origin and key a mesh of the tile would carry (tests and tools without meshes). One
/// keyer serves all tiles.
pub struct ResidentRecords<'a> {
    field: &'a dyn HeightField,
    map: &'a dyn planet_core::cube::FaceMapping,
    keyer: planet_generators::mesh::MeshKeyer<'a>,
    radius_m: f64,
}

impl<'a> ResidentRecords<'a> {
    pub fn new(field: &'a dyn HeightField, map: &'a dyn planet_core::cube::FaceMapping, lod: &LodParams) -> Self {
        Self { field, map, keyer: planet_generators::mesh::MeshKeyer::new(field, map, lod.cells, lod.radius_m), radius_m: lod.radius_m }
    }

    pub fn record(&self, tile: TileId) -> ResidentTile {
        let centre = tile.center_direction(self.map);
        ResidentTile { mesh_key: self.keyer.key(tile), origin: PlanetFixed(centre * (self.radius_m + self.field.height(centre))) }
    }
}

impl Drop for PlanPipeline {
    /// Dropping the pipeline closes its worker: the plan in flight finishes, nothing else starts (STRM-005 teardown).
    fn drop(&mut self) {
        self.worker.close();
    }
}
