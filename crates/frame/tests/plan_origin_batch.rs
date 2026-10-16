//! LOD-009, GEN-020: the plan evaluates the origin heights of the tiles its cache lacks as ONE batch (read block by block), not one
//! `height` per node in tree order, and the plan is bit for bit the plan of the per-node evaluation, at any thread count.
//! The bake is a synthetic multi-block lattice behind a source that counts block decodes like the store's cache (an LRU of
//! `capacity` blocks), so the counts are those of a cold plan over the stored 16384-cell bake, scaled down.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{Face, PlanetFixed, TileId, Vec3};
use planet_frame::scenes::{build_script, dive_altitude_m, StreamScene, DIVE_DESCENT_S, DIVE_START_M, FRAME_STEP_S};
use planet_frame::{
    plan_terrain_cached, plan_terrain_threaded, scripted_views_field, LodParams, SelectionCache, TerrainRequest, TerrainView,
};
use planet_generators::erosion::{ErosionField, ErosionGenerator};
use planet_generators::global::{GlobalSample, LatticeSource, ReadError};
use planet_generators::height::HeightField;
use planet_world_def::{BodyId, GenContext, ReadRecord, WorldDefinition};

const R: f64 = 6_371_000.0;
const RES: u32 = 16384;
const BLOCK: u32 = 1024;

/// A smooth function of the direction of the vertex, so the bake is continuous across the faces (a lattice that jumps at a cube edge
/// would make the plan's morph intervals impossible there).
fn synthetic(face: Face, i: u32, j: u32) -> GlobalSample {
    let (s, t) = (2.0 * f64::from(i) / f64::from(RES) - 1.0, 2.0 * f64::from(j) / f64::from(RES) - 1.0);
    let d = planet_core::cube::face_to_direction(&TangentWarp, face, s, t);
    let e = 800.0 * libm::sin(5.0 * d.x + 1.0) * libm::cos(4.0 * d.y - 0.5) + 200.0 * libm::sin(9.0 * d.z + 3.0 * d.x);
    GlobalSample { continent: 1.0, elevation_m: e as f32, temperature_c: 10.0, precipitation_mm: 500.0 }
}

/// A block store with an LRU cache of `capacity` blocks shared by all threads, and (like the store's `StoreSource`) the block each
/// thread read last pinned: a run of reads inside one block does not meet the cache. It counts block decodes (one per miss).
type BlockKey = (u8, u32, u32);

struct Blocks {
    id: u64,
    capacity: usize,
    state: Mutex<(Vec<BlockKey>, u64)>,
}

thread_local! {
    static PINNED: std::cell::Cell<Option<(u64, BlockKey)>> = const { std::cell::Cell::new(None) };
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl Blocks {
    fn new(capacity: usize) -> Blocks {
        Blocks { id: NEXT_ID.fetch_add(1, Relaxed), capacity, state: Mutex::new((Vec::new(), 0)) }
    }
    fn decodes(&self) -> u64 {
        self.state.lock().unwrap().1
    }
}

impl LatticeSource for Blocks {
    fn res(&self) -> u32 {
        RES
    }
    fn block_cells(&self) -> u32 {
        BLOCK
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let key = (face.0, (i / BLOCK).min(RES / BLOCK - 1), (j / BLOCK).min(RES / BLOCK - 1));
        if PINNED.get() != Some((self.id, key)) {
            let mut st = self.state.lock().unwrap();
            if let Some(at) = st.0.iter().position(|k| *k == key) {
                let k = st.0.remove(at);
                st.0.push(k);
            } else {
                st.1 += 1;
                if st.0.len() == self.capacity {
                    st.0.remove(0);
                }
                st.0.push(key);
            }
            PINNED.set(Some((self.id, key)));
        }
        Ok(synthetic(face, i, j))
    }
}

fn eroded(src: &dyn LatticeSource) -> ErosionField<'_> {
    let def = WorldDefinition::single_planet(R, 7);
    ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, src, [9; 16]).unwrap()
}

/// The eroded field with counters: calls of `height`, of the batch methods, the directions of the batches and the decodes of the
/// source during the batches.
struct Probe<'a> {
    inner: ErosionField<'a>,
    blocks: &'a Blocks,
    singles: AtomicU64,
    batches: AtomicU64,
    batch_dirs: Mutex<Vec<Vec3>>,
    batch_decodes: AtomicU64,
}

impl<'a> Probe<'a> {
    fn new(blocks: &'a Blocks) -> Probe<'a> {
        Probe {
            inner: eroded(blocks),
            blocks,
            singles: AtomicU64::new(0),
            batches: AtomicU64::new(0),
            batch_dirs: Mutex::new(Vec::new()),
            batch_decodes: AtomicU64::new(0),
        }
    }
    fn reset(&self) {
        self.singles.store(0, Relaxed);
        self.batches.store(0, Relaxed);
        self.batch_dirs.lock().unwrap().clear();
        self.batch_decodes.store(0, Relaxed);
    }
    fn batch(&self, dirs: &[Vec3], run: impl FnOnce() -> Vec<f64>) -> Vec<f64> {
        self.batches.fetch_add(1, Relaxed);
        self.batch_dirs.lock().unwrap().extend_from_slice(dirs);
        let before = self.blocks.decodes();
        let out = run();
        self.batch_decodes.fetch_add(self.blocks.decodes() - before, Relaxed);
        out
    }
}

impl HeightField for Probe<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.singles.fetch_add(1, Relaxed);
        self.inner.height(dir)
    }
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        self.batch(dirs, || self.inner.heights_until(dirs, stop))
    }
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        self.batch(dirs, || self.inner.heights_threaded_until(dirs, stop, threads))
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.inner.implementation_id()
    }
    fn version(&self) -> u32 {
        self.inner.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m()
    }
    fn inputs_hash(&self) -> u64 {
        self.inner.inputs_hash()
    }
    fn read_records(&self) -> Vec<ReadRecord> {
        self.inner.read_records()
    }
    fn body(&self) -> BodyId {
        self.inner.body()
    }
    fn source_key(&self) -> Option<[u8; 16]> {
        self.inner.source_key()
    }
    fn height_bounds(&self, centre: Vec3, r: f64) -> (f64, f64) {
        self.inner.height_bounds(centre, r)
    }
}

fn request(camera: planet_frame::Camera, field: &dyn HeightField) -> TerrainRequest {
    TerrainRequest {
        camera,
        size: (256, 144),
        view: TerrainView::Level,
        lod: LodParams {
            cells: 32,
            viewport_h_px: 1440.0,
            tau_px: 12.0,
            max_level: 22,
            max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
            ..LodParams::earth_1080p()
        },
        face_mapping: TangentWarp.id().to_string(),
        seed: field.seed(),
    }
}

/// The dive's camera at `altitude_m` (the script's frame for that altitude).
fn dive_camera(field: &dyn HeightField, altitude_m: f64) -> planet_frame::Camera {
    let script = build_script(StreamScene::Dive, R, 16.0 / 9.0, field, 1);
    let u = libm::log(altitude_m / DIVE_START_M) / libm::log(1.0 / DIVE_START_M);
    let k = ((u * DIVE_DESCENT_S / FRAME_STEP_S).round() as usize).min(script.cameras.len() - 1);
    assert!((dive_altitude_m(k as f64 * FRAME_STEP_S) / altitude_m - 1.0).abs() < 0.05, "frame {k} is not at {altitude_m} m");
    script.cameras[k]
}

/// The cameras of the tests: the ten standard views and the dive's camera at each altitude band above 100 km (the lower bands are the standard views').
fn cameras(field: &dyn HeightField) -> Vec<(String, planet_frame::Camera)> {
    let mut v: Vec<(String, planet_frame::Camera)> =
        scripted_views_field(R, 16.0 / 9.0, field).into_iter().map(|(n, c)| (n.to_string(), c)).collect();
    for alt in [2.0e7, 5.0e6, 2.0e6, 5.0e5, 2.0e5] {
        v.push((format!("dive-{alt:.0}m"), dive_camera(field, alt)));
    }
    v
}

// spec: LOD-009
// spec: GEN-020
#[test]
fn a_cold_plan_evaluates_its_origin_heights_as_one_batch_that_decodes_each_block_once() {
    // Altitude views: their cold plans are the cost of the dive (hundreds of blocks for tens of nodes) and select cheaply.
    let any = Blocks::new(usize::MAX);
    let cams = cameras(&eroded(&any));
    let mut checked = 0;
    for (name, camera) in
        cams.iter().filter(|(n, _)| ["orbit-20000km", "high-2000km", "dive-5000000m", "dive-500000m"].contains(&n.as_str()))
    {
        // A cache of ONE block: any block read twice is decoded twice.
        let small = Blocks::new(1);
        let probe = Probe::new(&small);
        let req = request(*camera, &probe);
        for threads in [1usize, 20] {
            probe.reset();
            let plan = plan_terrain_threaded(&req, &probe, &mut SelectionCache::new(), threads).unwrap();
            assert_eq!(probe.singles.load(Relaxed), 0, "{name}: no origin is evaluated one `height` at a time");
            assert_eq!(probe.batches.load(Relaxed), 1, "{name}: one batch per plan (at most the tree depth), {threads} threads");
            let dirs = probe.batch_dirs.lock().unwrap().clone();
            assert_eq!(dirs.len(), plan.nodes.len(), "{name}: the batch holds the origin of every node of a cold plan");
            // The decodes of an unbounded cache over the same directions: every distinct block once.
            let unbounded = Blocks::new(usize::MAX);
            eroded(&unbounded).heights(&dirs);
            let distinct = unbounded.decodes();
            let decodes = probe.batch_decodes.load(Relaxed);
            println!("{name} ({threads} threads): {} nodes, {distinct} distinct blocks, {decodes} decodes in the batch", plan.nodes.len());
            assert!(distinct > 0 && decodes <= distinct, "{name}: {decodes} decodes for {distinct} distinct blocks with a one-block cache");
            checked += 1;
        }
    }
    assert_eq!(checked, 8);
}

// spec: LOD-009
// spec: GEN-020
#[test]
fn the_plan_equals_the_per_node_evaluation_at_every_thread_count() {
    let any = Blocks::new(usize::MAX);
    let field = eroded(&any);
    let map = TangentWarp;
    for (name, camera) in cameras(&field) {
        let req = request(camera, &field);
        let one = plan_terrain_cached(&req, &field, &mut SelectionCache::new()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let many = plan_terrain_threaded(&req, &field, &mut SelectionCache::new(), 20).unwrap();
        assert_eq!(one, many, "{name}: the plan of 20 threads equals the plan of one");
        // The old plan: one `height(centre)` per node.
        for n in &one.nodes {
            let centre = n.tile.center_direction(&map);
            let origin = PlanetFixed(centre * (R + field.height(centre)));
            assert_eq!(
                (n.origin.0.x.to_bits(), n.origin.0.y.to_bits(), n.origin.0.z.to_bits()),
                (origin.0.x.to_bits(), origin.0.y.to_bits(), origin.0.z.to_bits()),
                "{name}: origin of {:?} (level {})",
                n.tile,
                n.level
            );
        }
    }
}

// spec: LOD-009
#[test]
fn a_warm_plan_makes_no_batch_and_a_moved_camera_batches_only_its_new_tiles() {
    let any = Blocks::new(usize::MAX);
    let probe = Probe::new(&any);
    let a = request(dive_camera(&probe, 5.0e5), &probe);
    let mut b = a.clone();
    b.camera.position = PlanetFixed(b.camera.position.0 + b.camera.forward * 20_000.0);
    let mut cache = SelectionCache::new();
    let first = plan_terrain_cached(&a, &probe, &mut cache).unwrap();
    probe.reset();
    let again = plan_terrain_cached(&a, &probe, &mut cache).unwrap();
    assert_eq!((probe.singles.load(Relaxed), probe.batches.load(Relaxed)), (0, 0), "a warm plan evaluates no origin height");
    assert_eq!(first, again);
    let moved = plan_terrain_cached(&b, &probe, &mut cache).unwrap();
    let (singles, batches, batched) = (probe.singles.load(Relaxed), probe.batches.load(Relaxed), probe.batch_dirs.lock().unwrap().len());
    let known: std::collections::HashSet<TileId> = first.nodes.iter().map(|n| n.tile).collect();
    let new = moved.nodes.iter().filter(|n| !known.contains(&n.tile)).count();
    assert!(new > 0 && new < moved.nodes.len(), "the step keeps some tiles and brings new ones ({new} of {})", moved.nodes.len());
    assert_eq!((singles, batches, batched), (0, 1, new), "one batch of exactly the new tiles");
    let fresh = plan_terrain_cached(&b, &probe, &mut SelectionCache::new()).unwrap();
    assert_eq!(moved, fresh, "the cached plan of the moved camera equals the plan of a fresh cache");
}
