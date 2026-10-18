//! Fakes shared by the scheduler's equivalence and cost tests (STRM-005, STRM-019): the reference field, a camera on a ground
//! track, a source of empty tiles with the key the plan expects, a counting sink and the managed loop on manual spawners.
#![allow(dead_code)]

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::managed::{ManagedLoop, TileSink};
use planet_frame::pipeline::{PlanPipeline, PlanWorker, ResidentRecords};
use planet_frame::prefetch::PrefetchConfig;
use planet_frame::upload::UploadBudget;
use planet_frame::{Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::CompactTile;
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::TileMesh;
use planet_streaming::{FakeClock, ManualSpawner, ResidencyManager, SlotRef, Spawner, StreamConfig, Streamer, TileSource};
use std::collections::BTreeSet;
use std::sync::Arc;

pub const R: f64 = 6_371_000.0;
pub const SIZE: (u32, u32) = (160, 90);
pub const CELLS: u32 = 16;
pub const FRAME_MS: u64 = 33;

/// FNV-1a over u64 words: the digest of a run.
pub struct Digest(pub u64);

impl Digest {
    pub fn new() -> Self {
        Digest(0xcbf2_9ce4_8422_2325)
    }
    pub fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    pub fn tile(&mut self, t: TileId) {
        self.word(t.raw());
    }
}

pub fn lod() -> LodParams {
    LodParams {
        cells: CELLS,
        viewport_h_px: 360.0,
        tau_px: 8.0,
        max_level: 16,
        // The loop tests of the scheduler start at the six face tiles (floor 0); the floor-3 loop has its own tests (LOD-011).
        min_level: 0,
        max_height_m: Reference(1).max_abs_height_m(),
        ..LodParams::earth_1080p()
    }
}

pub fn position(dir: Vec3, height: f64) -> PlanetFixed {
    PlanetFixed(dir.normalized() * (R + height))
}

pub fn camera(pos: PlanetFixed, yaw: f64, pitch: f64) -> Camera {
    let hint = Vec3::new(libm::cos(yaw), libm::sin(yaw), 0.37 * libm::sin(2.0 * yaw) + 0.2);
    Camera::at_surface_point(pos, hint, pitch, 60f64.to_radians(), f64::from(SIZE.0) / f64::from(SIZE.1), 0.1)
}

pub fn request(pos: PlanetFixed, yaw: f64, pitch: f64) -> TerrainRequest {
    request_with(lod(), pos, yaw, pitch)
}

pub fn request_with(lod: LodParams, pos: PlanetFixed, yaw: f64, pitch: f64) -> TerrainRequest {
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
pub fn track(metres: f64, height: f64) -> PlanetFixed {
    let start = Vec3::new(0.3, 0.5, 0.8).normalized();
    let east = start.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
    position((start + east * (metres / R)).normalized(), height)
}

pub fn here() -> PlanetFixed {
    position(Vec3::new(0.3, 0.5, 0.8), 800.0)
}

pub fn there() -> PlanetFixed {
    position(Vec3::new(-0.6, -0.2, -0.5), 300.0)
}

// ---- the managed loop ----

pub struct Source {
    pub lod: LodParams,
}

impl TileSource<CompactTile> for Source {
    fn load(&self, tile: TileId) -> Result<CompactTile, String> {
        let field = Reference(1);
        let rec = ResidentRecords::new(&field, &TangentWarp, &self.lod).record(tile);
        Ok(RenderTile::Vertex(TileMesh {
            id: tile,
            cells: CELLS,
            key: rec.mesh_key,
            origin: rec.origin,
            vertices: Vec::new(),
            climate: Default::default(),
        }))
    }
}

#[derive(Default)]
pub struct CountingSink {
    pub order: Vec<TileId>,
    pub forgets: Vec<TileId>,
    pub bytes: u64,
}

impl TileSink for CountingSink {
    fn upload_bytes(&self, _tile: &CompactTile) -> u64 {
        self.bytes
    }

    fn upload_at(&mut self, tile: &CompactTile, _slot: SlotRef) -> Result<(), String> {
        self.order.push(tile.id());
        Ok(())
    }

    fn forget(&mut self, tile: TileId) {
        self.forgets.push(tile);
    }
}

pub struct LoopRig {
    pub lp: ManagedLoop,
    pub plan_spawner: Arc<ManualSpawner>,
    pub tile_spawner: Arc<ManualSpawner>,
    pub sink: CountingSink,
    pub clock: Arc<FakeClock>,
}

pub fn loop_rig(capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget) -> LoopRig {
    loop_rig_for(lod(), capacity, prefetch, tile_bytes, budget)
}

pub fn loop_rig_for(lod: LodParams, capacity: usize, prefetch: bool, tile_bytes: u64, budget: UploadBudget) -> LoopRig {
    let clock = Arc::new(FakeClock::default());
    let plan_spawner = Arc::new(ManualSpawner::default());
    let tile_spawner = Arc::new(ManualSpawner::default());
    let pipe = PlanPipeline::new(PlanWorker::new(plan_spawner.clone(), Arc::new(Reference(1)), 2));
    let config = StreamConfig {
        max_in_flight: 64,
        capacity,
        retire_frames: 2,
        retry_after_ms: 100,
        floor: lod.floor_level(),
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
    pub fn step(&mut self, req: &TerrainRequest, jobs: usize, d: &mut Digest) {
        self.clock.advance(FRAME_MS);
        let log_before = self.lp.manager().streamer().dispatch_log.len();
        let f = self.lp.begin_frame(req, &mut self.sink).unwrap();
        // The twelve columns the digests were taken with; `upload_calls` (REND-018) is a figure of the sink, pinned by its own tests.
        for v in &f.counts.values()[..planet_frame::managed::FrameCounts::DIGESTED_COLUMNS] {
            d.word(*v);
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

    pub fn finish(&self, d: &mut Digest) {
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
