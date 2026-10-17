//! SPIKE-13 ONLY (never merged): edit-to-visible latency. A test-only authored item (a raised disc, "stamp") entered through the
//! in-memory cell index (`WorldDefinition::put_item`), a height field that reads it through cell queries, and a harness that
//! drives the real pipelined loop (plan worker, residency manager, streamer, pools, renderer) through an edit and times the stages.

use crate::pipelined::RendererSink;
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::hash::hash_u64s;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::managed::ManagedLoop;
use planet_frame::pipeline::{residency_of, FramesInFlight, PlanField, PlanPipeline, PlanWorker, MAX_FRAMES_IN_FLIGHT};
use planet_frame::{plan_terrain_threaded, Camera, LodParams, PlanError, ResidencySet, SelectionCache, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::climate::ClimateTexture;
use planet_generators::compact::{tile_data_field, RenderTile};
use planet_generators::global::{bake, GridSource};
use planet_generators::height::HeightField;
use planet_generators::height_fields::{build_field, HeightFieldKind};
use planet_render::{GpuContext, TerrainRenderer};
use planet_streaming::{GenWorkers, ResidencyManager, StreamConfig, Streamer, SystemClock, ThreadPool, TileSource};
use planet_world_def::{BodyId, GenContext, ReadRecord, WorldDefinition};
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub const R: f64 = 6_371_000.0;
/// Largest stamp amplitude: the bounds of the field are widened by it always, so an edit never changes the LOD selection's volumes.
pub const MAX_STAMP_AMP_M: f64 = 120.0;

#[derive(Clone, Copy, Debug)]
pub struct Stamp {
    pub id: u64,
    pub dir: Vec3,
    pub radius_m: f64,
    pub amp_m: f64,
}

impl Stamp {
    pub fn content_hash(&self) -> u64 {
        hash_u64s(0x7374_6d70, &[self.radius_m.to_bits(), self.amp_m.to_bits()])
    }

    /// Raised disc: `A (1 - (d / r)^2)^2` inside the radius (smooth at the rim), along the surface distance `d`.
    pub fn bump(&self, dir: Vec3) -> f64 {
        let chord = (dir - self.dir).length();
        let d = R * 2.0 * libm::asin((chord / 2.0).min(1.0));
        if d >= self.radius_m {
            return 0.0;
        }
        let w = 1.0 - (d / self.radius_m) * (d / self.radius_m);
        self.amp_m * w * w
    }
}

/// A snapshot of the authored items: the definition with its cell index and the stamps' parameters.
pub struct StampSet {
    pub def: WorldDefinition,
    pub stamps: Vec<Stamp>,
}

impl StampSet {
    pub fn with(&self, s: Stamp) -> StampSet {
        let mut def = self.def.clone();
        def.put_item(BodyId::PLANET, s.id, s.dir, s.content_hash()).expect("item");
        let mut stamps: Vec<Stamp> = self.stamps.iter().copied().filter(|t| t.id != s.id).collect();
        stamps.push(s);
        StampSet { def, stamps }
    }
}

/// The boxed base field handed to threads: it is read-only after construction (the spike's bake is leaked), so sharing it is sound.
pub struct SendBase(pub Box<dyn HeightField>);
unsafe impl Send for SendBase {}
impl std::ops::Deref for SendBase {
    type Target = dyn HeightField;
    fn deref(&self) -> &(dyn HeightField + 'static) {
        &*self.0
    }
}

/// The base field (eroded over a small in-memory bake, so generation costs what the product's eroded tiles cost) plus the stamps.
pub struct StampField {
    pub set: Arc<StampSet>,
    pub base: Arc<SendBase>,
}

impl HeightField for StampField {
    fn height(&self, d: Vec3) -> f64 {
        self.base.height(d) + self.set.stamps.iter().map(|s| s.bump(d)).sum::<f64>()
    }
    fn seed(&self) -> u64 {
        self.base.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "spike13-stamp"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.base.max_abs_height_m() + MAX_STAMP_AMP_M
    }
    fn inputs_hash(&self) -> u64 {
        self.base.inputs_hash()
    }
    fn read_records(&self) -> Vec<ReadRecord> {
        self.base.read_records()
    }
    fn reads_cells(&self) -> bool {
        true
    }
    fn read_records_for(&self, tile: TileId, map: &dyn FaceMapping) -> Vec<ReadRecord> {
        let mut records = self.base.read_records_for(tile, map);
        let ctx = GenContext::new(&self.set.def);
        let _ = ctx.body_seed(BodyId::PLANET);
        let _ = ctx.query_tile(BodyId::PLANET, tile, 1, 64);
        records.extend(ctx.read_records());
        records
    }
    fn body(&self) -> BodyId {
        self.base.body()
    }
    fn source_key(&self) -> Option<[u8; 16]> {
        self.base.source_key()
    }
    fn surface_id(&self) -> &'static str {
        self.base.surface_id()
    }
    fn climate(&self, map: &dyn FaceMapping, id: TileId) -> ClimateTexture {
        self.base.climate(map, id)
    }
    fn climate_version_word(&self) -> u64 {
        self.base.climate_version_word()
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (lo, hi) = self.base.height_bounds(centre, angular_radius);
        (lo, hi + MAX_STAMP_AMP_M)
    }
}

/// The current field, replaced by an edit (jobs take a snapshot when they start).
pub struct Shared {
    pub field: RwLock<Arc<StampField>>,
}

impl Shared {
    pub fn current(&self) -> Arc<StampField> {
        Arc::clone(&self.field.read().expect("field"))
    }
}

struct StampPlanner(Arc<Shared>);

impl PlanField for StampPlanner {
    fn plan(&self, req: &TerrainRequest, cache: &mut SelectionCache, threads: usize) -> Result<TerrainPlan, PlanError> {
        let f = self.0.current();
        plan_terrain_threaded(req, &*f, cache, threads)
    }
    fn residency(&self, req: &TerrainRequest, cache: &mut SelectionCache) -> Result<ResidencySet, PlanError> {
        let f = self.0.current();
        residency_of(req, &*f, cache)
    }
}

struct StampSource {
    shared: Arc<Shared>,
    max_level: u8,
    cells: u32,
}

impl TileSource<RenderTile> for StampSource {
    fn load(&self, tile: TileId) -> Result<RenderTile, String> {
        let f = self.shared.current();
        Ok(tile_data_field(&*f, &TangentWarp, tile, self.cells, R, self.max_level))
    }
}

/// The base world: a small in-memory bake and the eroded field over it.
pub fn base_world(seed: u64) -> (WorldDefinition, Arc<SendBase>) {
    let grid = bake(seed, &TangentWarp, 16, 2).expect("bake");
    let def = WorldDefinition::single_planet(R, seed);
    let src = Box::leak(Box::new(GridSource(Box::leak(Box::new(grid)))));
    let field = build_field(HeightFieldKind::Eroded, &GenContext::new(&def), BodyId::PLANET, src, [1; 16]).expect("eroded field");
    (def, Arc::new(SendBase(field)))
}

pub struct Rig {
    pub renderer: TerrainRenderer,
    pub managed: ManagedLoop,
    pub fi: FramesInFlight,
    pub shared: Arc<Shared>,
}

impl Rig {
    pub fn new(ctx: &GpuContext, shared: Arc<Shared>, first: &TerrainRequest, workers: usize) -> Result<Rig, String> {
        let mut renderer = TerrainRenderer::for_context(ctx, first.size, first.lod.cells, u64::from(MAX_FRAMES_IN_FLIGHT));
        renderer.warm_up(ctx);
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let plan_pool = Arc::new(ThreadPool::new(1));
        let pipe = PlanPipeline::new(PlanWorker::new(plan_pool, Arc::new(StampPlanner(Arc::clone(&shared))), threads));
        let pool = *renderer.pool_config();
        let clock: Arc<dyn planet_streaming::Clock> = Arc::new(SystemClock::new());
        let gen = GenWorkers::new(workers);
        let mut streamer = Streamer::new(
            Arc::new(StampSource { shared: Arc::clone(&shared), max_level: first.lod.max_level, cells: first.lod.cells }),
            gen.pool(),
            Arc::clone(&clock),
            gen.apply(StreamConfig {
                capacity: pool.main_slots() as usize,
                vertex_pool_slots: pool.managed_vertex_slots() as usize,
                retire_frames: u64::from(MAX_FRAMES_IN_FLIGHT),
                retry_after_ms: 1000,
                record_dispatches: false,
                record_changes: true,
                ..Default::default()
            }),
        );
        streamer.load_roots_blocking()?;
        let managed = ManagedLoop::new(pipe, ResidencyManager::new(streamer)).with_prefetch(planet_frame::prefetch::PrefetchConfig::default(), R, clock);
        Ok(Rig { renderer, managed, fi: FramesInFlight::new(MAX_FRAMES_IN_FLIGHT), shared })
    }
}

/// A camera at `h` metres above the ground at ground distance `rho` from the stamp, looking at the stamp's centre, at angle
/// `theta` (radians) around it.
pub fn camera_around(stamp: &Stamp, field: &dyn HeightField, rho: f64, h: f64, theta: f64, aspect: f64) -> Camera {
    let c = stamp.dir;
    let north = Vec3::new(0.0, 0.0, 1.0);
    let t1 = (north - c * north.dot(c)).normalized();
    let t2 = c.cross(t1);
    let tang = t1 * libm::cos(theta) + t2 * libm::sin(theta);
    let a = rho / R;
    let dir = (c * libm::cos(a) + tang * libm::sin(a)).normalized();
    let pos = PlanetFixed(dir * (R + field.height(dir) + h));
    let target = c * (R + field.height(c) + stamp.amp_m * 0.5);
    let forward = (target - pos.0).normalized();
    Camera { position: pos, forward, up: dir, fov_y_rad: 60f64.to_radians(), aspect, near_m: 0.1 }
}

#[derive(Clone, Debug, Default)]
pub struct LevelCounts(pub std::collections::BTreeMap<u8, usize>);

impl LevelCounts {
    pub fn text(&self) -> String {
        self.0.iter().map(|(l, c)| format!("{l}:{c}")).collect::<Vec<_>>().join(" ")
    }
    pub fn total(&self) -> usize {
        self.0.values().sum()
    }
}

/// Whether a tile's footprint (bounding sphere) can touch the stamp's disc.
pub fn touches(tile: TileId, stamp: &Stamp) -> bool {
    let c = tile.center_direction(&TangentWarp);
    let edge = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << tile.level());
    let d = R * 2.0 * libm::asin(((c - stamp.dir).length() / 2.0).min(1.0));
    d < stamp.radius_m + 0.8 * edge
}

pub fn request(camera: Camera, size: (u32, u32), lod: &LodParams) -> TerrainRequest {
    TerrainRequest { camera, size, view: TerrainView::Relief, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 7 }
}

pub fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

pub fn _unused(_: &RendererSink<'_>) {}

// ---------------------------------------------------------------------------------------------------------------------------------
// The case runner.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The naive invalidation: the affected tiles are dropped and requested again; the frames draw ancestors meanwhile.
    Evict,
    /// Stale while revalidating: the affected tiles are requested again, the old versions stay drawn until the new ones arrive.
    Refresh,
}

#[derive(Clone, Copy, Debug)]
pub struct CaseCfg {
    pub altitude_m: f64,
    pub radius_m: f64,
    pub moving: bool,
    pub workers: usize,
    pub mode: Mode,
    pub preview: bool,
    /// 0: the plan cache is not told (stale keys), 1: targeted per tile, 2: cleared.
    pub cache_mode: u8,
    pub size: (u32, u32),
    /// Write the before / preview / exact frames of a still case here.
    pub images: Option<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct GenStats {
    pub per_level: std::collections::BTreeMap<u8, (usize, f64, f64)>,
    pub total_ms: f64,
    pub wait_mean_ms: f64,
}

#[derive(Clone, Debug, Default)]
pub struct CaseResult {
    pub resident_before: usize,
    pub affected: LevelCounts,
    pub affected_cells_method: usize,
    pub target: LevelCounts,
    pub find_keys_ms: f64,
    pub find_cells_ms: f64,
    pub apply_ms: f64,
    pub plan_delivered_ms: Option<f64>,
    pub plan_worker_ms: f64,
    pub first_dispatch_ms: Option<f64>,
    pub preview_ms: Option<f64>,
    pub first_new_ms: Option<f64>,
    pub exact_ms: Option<f64>,
    pub all_ms: Option<f64>,
    pub exact_frame_ms: f64,
    pub gen: GenStats,
    pub upload_ms_total: f64,
    pub upload_frames: usize,
    pub tiles_uploaded: u64,
    pub scheduler_ms_max: f64,
    pub frame_cpu_ms_max: f64,
    pub substituted_max: usize,
    pub blank_max: usize,
    pub frames: usize,
    pub load_line: String,
    pub settle_s: f64,
    pub error: Option<String>,
}

fn stamp_for(radius_m: f64) -> Stamp {
    let (la, lo) = (20f64.to_radians(), 30f64.to_radians());
    let dir = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
    Stamp { id: 1, dir, radius_m, amp_m: (0.2 * radius_m).min(100.0) }
}

pub fn run_case(ctx: &GpuContext, base: &(WorldDefinition, Arc<SendBase>), cfg: CaseCfg) -> CaseResult {
    match run_case_inner(ctx, base, cfg) {
        Ok(r) => r,
        Err(e) => CaseResult { error: Some(e), ..Default::default() },
    }
}

fn run_case_inner(ctx: &GpuContext, base: &(WorldDefinition, Arc<SendBase>), cfg: CaseCfg) -> Result<CaseResult, String> {
    let stamp = stamp_for(cfg.radius_m);
    let aspect = f64::from(cfg.size.0) / f64::from(cfg.size.1);
    let set0 = Arc::new(StampSet { def: base.0.clone(), stamps: Vec::new() });
    let field0 = Arc::new(StampField { set: Arc::clone(&set0), base: Arc::clone(&base.1) });
    let shared = Arc::new(Shared { field: RwLock::new(Arc::clone(&field0)) });
    let lod = LodParams {
        cells: 32,
        viewport_h_px: f64::from(cfg.size.1),
        tau_px: 6.0,
        max_level: planet_frame::production_finest_level(32),
        max_height_m: 10_000.0,
        ..LodParams::earth_1080p()
    };
    let rho = (1.5 * cfg.radius_m).max(cfg.altitude_m);
    let speed = match cfg.altitude_m as u64 {
        0..=10 => 5.0,
        11..=500 => 50.0,
        _ => 250.0,
    };
    let omega = speed / rho;
    let cam_at = |theta: f64, field: &dyn HeightField| camera_around(&stamp, field, rho, cfg.altitude_m, theta, aspect);
    let first = request(cam_at(0.0, &*field0), cfg.size, &lod);
    planet_render::set_stamp_preview(None);
    planet_render::set_stamp_preview_tiles(Default::default());
    planet_frame::pipeline::SPIKE_STALE_OK.store(cfg.mode == Mode::Refresh, std::sync::atomic::Ordering::SeqCst);
    let mut rig = Rig::new(ctx, Arc::clone(&shared), &first, cfg.workers)?;
    let mut last_plan: Option<TerrainPlan> = None;
    let mut iteration = 0u64;

    // One iteration of the loop: the manager's frame (requests, collection, uploads), the plan, a frame submitted.
    let mut step = |rig: &mut Rig, req: &TerrainRequest| -> Result<planet_frame::managed::ManagedFrame, String> {
        iteration += 1;
        let frame = rig.managed.begin_frame(req, &mut RendererSink { ctx, renderer: &mut rig.renderer })?;
        rig.fi.retire_to(rig.renderer.retired_frames(ctx));
        while rig.fi.is_full() {
            rig.fi.note_wait();
            rig.fi.retire_to(rig.renderer.wait_oldest(ctx));
        }
        if let Err(e) = rig.renderer.submit(ctx, &frame.pipeline.draw.plan) {
            let f = rig.shared.current();
            let keyer = planet_generators::mesh::MeshKeyer::new(&*f, &TangentWarp, 32, R).at_finest_level(22);
            let bad: Vec<String> = frame
                .pipeline
                .draw
                .plan
                .nodes
                .iter()
                .filter(|d| rig.renderer.tile_key(d.tile).is_some_and(|k| k != d.mesh_key))
                .take(3)
                .map(|d| format!("level {} plan {:#x} renderer {:#x?} managed {:#x?} true {:#x} kind {:?} subst {}", d.level, d.mesh_key, rig.renderer.tile_key(d.tile), rig.managed.resident().get(&d.tile).map(|r| r.mesh_key), keyer.key(d.tile), frame.pipeline.draw.kind, frame.pipeline.draw.substituted))
                .collect();
            return Err(format!("frame {iteration}: {e} :: {bad:?}"));
        }
        rig.fi.submit();
        last_plan = Some(frame.pipeline.draw.plan.clone());
        Ok(frame)
    };

    // Settle: the camera still until the plan is exact and nothing is generating.
    let settle0 = Instant::now();
    let mut calm = 0;
    loop {
        let frame = step(&mut rig, &first)?;
        let idle = rig.managed.manager().streamer().unfinished_jobs() == 0
            && !rig.managed.pipeline().worker().busy()
            && rig.managed.pending().is_empty();
        if frame.pipeline.draw.kind == planet_frame::pipeline::DrawKind::Exact && idle {
            calm += 1;
        } else {
            calm = 0;
        }
        if calm >= 20 {
            break;
        }
        if settle0.elapsed().as_secs() > 240 {
            return Err(format!("did not settle in 240 s ({:?})", rig.managed.manager().streamer().stats()));
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let settle_s = settle0.elapsed().as_secs_f64();
    let mut out = CaseResult { settle_s, ..Default::default() };
    // The frame before the edit (for the image comparison).
    let before_img = rig.renderer.read_color(ctx).rgba;
    let resident_before: Vec<(TileId, u64)> = rig.managed.resident().iter().map(|(t, r)| (*t, r.mesh_key)).collect();
    out.resident_before = resident_before.len();
    let old_keys: std::collections::HashMap<TileId, u64> = resident_before.iter().copied().collect();

    let sampler = planet_perf::load::LoadSampler::start(1);
    let jobs_before = rig.managed.manager().streamer().job_log().len();
    let dispatched_before = rig.managed.manager().streamer().stats().dispatched;

    // ---- the edit ----
    let t0 = Instant::now();
    let tiles: Vec<TileId> = resident_before.iter().map(|(t, _)| *t).collect();
    let new_set = Arc::new(set0.with(stamp));
    let new_field = Arc::new(StampField { set: Arc::clone(&new_set), base: Arc::clone(&base.1) });
    // Find, by the key of each resident tile (the declared reads of the cell index).
    let tf = Instant::now();
    let map = TangentWarp;
    let affected: Vec<TileId> = tiles.iter().copied().filter(|t| field0.inputs_hash_for(*t, &map) != new_field.inputs_hash_for(*t, &map)).collect();
    out.find_keys_ms = ms(tf);
    // Find, from the cell grid alone (the cells the stamp's cell margin touches).
    let tf = Instant::now();
    let cell_level = planet_world_def::cells::cell_level(R);
    let item_cell = planet_world_def::cells::cell_of(R, stamp.dir);
    let affected_by_cells = tiles
        .iter()
        .filter(|t| match planet_world_def::cells::cells_covering(**t, cell_level, 64) {
            Some(c) => planet_world_def::cells::with_margin(&c, 1).contains(&item_cell),
            None => true,
        })
        .count();
    out.find_cells_ms = ms(tf);
    out.affected_cells_method = affected_by_cells;
    for t in &affected {
        *out.affected.0.entry(t.level()).or_insert(0) += 1;
    }
    let affected_set: std::collections::HashSet<TileId> = affected.iter().copied().collect();
    // Install the new field, then tell the cache, the streamer and the plan.
    let ta = Instant::now();
    *shared.field.write().expect("field") = Arc::clone(&new_field);
    let edit_seq = rig.managed.pipeline().frame() + 1;
    rig.managed.spike_edit(&affected, cfg.cache_mode, cfg.mode == Mode::Refresh);
    out.apply_ms = ms(ta);
    let center = stamp.dir * (R + new_field.base.height(stamp.dir));
    if cfg.preview {
        planet_render::set_stamp_preview(Some(([center.x, center.y, center.z], stamp.radius_m, stamp.amp_m)));
        planet_render::set_stamp_preview_tiles(affected_set.clone());
    }
    let mut preview_img: Option<Vec<u8>> = None;
    let mut first_dispatch_seen = false;
    let mut preview_done = !cfg.preview;
    let mut new_key_seen = false;
    let mut calm = 0;
    loop {
        let now = t0.elapsed().as_secs_f64() * 1000.0;
        let cam = if cfg.moving { cam_at(omega * now / 1000.0, &*new_field) } else { first.camera };
        let req = request(cam, cfg.size, &lod);
        let frame = step(&mut rig, &req)?;
        out.frames += 1;
        out.upload_ms_total += frame.upload_ms;
        out.upload_frames += usize::from(frame.counts.uploaded > 0);
        out.tiles_uploaded += frame.counts.uploaded;
        out.scheduler_ms_max = out.scheduler_ms_max.max(frame.scheduler_ms);
        out.substituted_max = out.substituted_max.max(frame.pipeline.draw.substituted);
        out.blank_max = out.blank_max.max(frame.pipeline.draw.blank);
        if let Some((seq, plan_ms)) = frame.pipeline.delivered {
            if seq >= edit_seq && out.plan_delivered_ms.is_none() {
                out.plan_delivered_ms = Some(ms(t0));
                out.plan_worker_ms = plan_ms;
            }
        }
        if !first_dispatch_seen && rig.managed.manager().streamer().stats().dispatched > dispatched_before {
            first_dispatch_seen = true;
            out.first_dispatch_ms = Some(ms(t0));
        }
        // Keep the preview on the tiles that still hold the old version.
        if cfg.preview {
            let stale: std::collections::HashSet<TileId> = affected
                .iter()
                .copied()
                .filter(|t| rig.managed.resident().get(t).is_some_and(|r| old_keys.get(t) == Some(&r.mesh_key)))
                .collect();
            planet_render::set_stamp_preview_tiles(stale);
        }
        if !preview_done {
            // The first frame submitted with the preview: visible when the GPU has finished it.
            ctx.probe_wait();
            out.preview_ms = Some(ms(t0));
            preview_done = true;
            if cfg.images.is_some() && !cfg.moving {
                preview_img = Some(rig.renderer.read_color(ctx).rgba);
            }
        }
        // The new plan's tiles that touch the stamp, and whether each holds its new version.
        let mut exact_now = false;
        if let Some(cur) = rig.managed.pipeline().current().filter(|c| c.seq >= edit_seq) {
            let target: Vec<(TileId, u64)> = cur.plan.nodes.iter().filter(|d| touches(d.tile, &stamp)).map(|d| (d.tile, d.mesh_key)).collect();
            let newer = target.iter().filter(|(t, k)| rig.managed.resident().get(t).is_some_and(|r| r.mesh_key == *k && old_keys.get(t) != Some(&r.mesh_key))).count();
            if newer > 0 && !new_key_seen {
                new_key_seen = true;
                out.first_new_ms = Some(ms(t0));
            }
            exact_now = !target.is_empty() && target.iter().all(|(t, k)| rig.managed.resident().get(t).is_some_and(|r| r.mesh_key == *k));
            if exact_now && out.exact_ms.is_none() {
                let tf = Instant::now();
                ctx.probe_wait();
                out.exact_frame_ms = ms(tf);
                out.exact_ms = Some(ms(t0));
                out.target = LevelCounts::default();
                for (t, _) in &target {
                    *out.target.0.entry(t.level()).or_insert(0) += 1;
                }
            }
        }
        let all_new = affected.iter().all(|t| rig.managed.resident().get(t).is_none_or(|r| old_keys.get(t) != Some(&r.mesh_key)));
        if all_new && out.all_ms.is_none() {
            out.all_ms = Some(ms(t0));
        }
        if out.exact_ms.is_some() && out.all_ms.is_some() {
            let idle = rig.managed.manager().streamer().unfinished_jobs() == 0;
            calm = if idle { calm + 1 } else { 0 };
            if calm >= 5 {
                break;
            }
        }
        if t0.elapsed().as_secs() > 180 {
            out.error = Some(format!(
                "timeout after 180 s: exact {:?}, all {:?}, streamer {:?}",
                out.exact_ms,
                out.all_ms,
                rig.managed.manager().streamer().stats()
            ));
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // Generation per level, from the job log.
    let samples = rig.managed.manager().streamer().job_log().since(jobs_before);
    let mut waits = 0.0;
    for s in &samples {
        let e = out.gen.per_level.entry(s.level).or_insert((0, 0.0, 0.0));
        let run = s.run_us as f64 / 1000.0;
        e.0 += 1;
        e.1 += run;
        e.2 = e.2.max(run);
        out.gen.total_ms += run;
        waits += s.wait_us as f64 / 1000.0;
    }
    out.gen.wait_mean_ms = if samples.is_empty() { 0.0 } else { waits / samples.len() as f64 };
    for e in out.gen.per_level.values_mut() {
        e.1 /= e.0 as f64;
    }
    out.load_line = sampler.stop().line();
    if let (Some(dir), false) = (cfg.images, cfg.moving) {
        let after = rig.renderer.read_color(ctx).rgba;
        let w = cfg.size.0;
        let h = cfg.size.1;
        let name = |s: &str| std::path::Path::new(dir).join(s);
        let _ = std::fs::create_dir_all(dir);
        let mut sheet_parts: Vec<(&str, Vec<u8>)> = vec![("before", before_img)];
        if let Some(p) = preview_img {
            sheet_parts.push(("preview", p));
        }
        sheet_parts.push(("exact", after));
        for (n, img) in &sheet_parts {
            let _ = planet_testkit_write(&name(&format!("{n}.png")), w, h, img);
        }
    }
    planet_render::set_stamp_preview(None);
    planet_render::set_stamp_preview_tiles(Default::default());
    planet_frame::pipeline::SPIKE_STALE_OK.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(out)
}

fn planet_testkit_write(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(std::io::Error::other)?;
    wr.write_image_data(rgba).map_err(std::io::Error::other)
}
