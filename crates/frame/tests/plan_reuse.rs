//! LOD-009: a plan reuses what is a pure function of the tile (origin height, mesh key) across plans, so a warm plan of
//! unchanged tiles evaluates the height field zero times.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use planet_core::cube::FaceMapping;
use planet_core::{TileId, Vec3};
use planet_frame::{plan_terrain_cached, scripted_views_field, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::height::{HeightField, Reference};
use planet_world_def::ReadRecord;

const R: f64 = 6_371_000.0;

/// `Reference` that counts every call that evaluates it.
struct Counting {
    inner: Reference,
    height: AtomicU64,
    bounds: AtomicU64,
    keys: AtomicU64,
    cells: bool,
    surface: &'static str,
    nan_once: AtomicU64,
}

impl Counting {
    fn new() -> Counting {
        Counting {
            inner: Reference(1),
            height: AtomicU64::new(0),
            bounds: AtomicU64::new(0),
            keys: AtomicU64::new(0),
            cells: false,
            surface: planet_world_def::SPHERE_SURFACE_ID,
            nan_once: AtomicU64::new(0),
        }
    }
    /// (height, height_bounds, per-tile key reads) since the last call.
    fn take(&self) -> (u64, u64, u64) {
        (self.height.swap(0, Relaxed), self.bounds.swap(0, Relaxed), self.keys.swap(0, Relaxed))
    }
}

impl HeightField for Counting {
    fn height(&self, dir: Vec3) -> f64 {
        self.height.fetch_add(1, Relaxed);
        if self.nan_once.compare_exchange(1, 2, Relaxed, Relaxed).is_ok() {
            return f64::NAN;
        }
        self.inner.height(dir)
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
    fn read_records_for(&self, _: TileId, _: &dyn FaceMapping) -> Vec<ReadRecord> {
        self.keys.fetch_add(1, Relaxed);
        self.inner.read_records()
    }
    fn reads_cells(&self) -> bool {
        self.cells
    }
    fn surface_id(&self) -> &'static str {
        self.surface
    }
    fn height_bounds(&self, centre: Vec3, r: f64) -> (f64, f64) {
        self.bounds.fetch_add(1, Relaxed);
        self.inner.height_bounds(centre, r)
    }
}

fn request(view: usize, cells: u32) -> TerrainRequest {
    let (_, camera) = scripted_views_field(R, 4.0 / 3.0, &Reference(1))[view];
    TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: LodParams { cells, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() },
        face_mapping: "tangent-v1".into(),
        seed: 1,
    }
}

// spec: LOD-009
#[test]
fn a_warm_plan_of_unchanged_tiles_evaluates_the_field_zero_times() {
    for view in [3, 5, 6] {
        let req = request(view, 16);
        let field = Counting::new();
        let mut cache = SelectionCache::new();
        let cold = plan_terrain_cached(&req, &field, &mut cache).unwrap();
        let (h, b, k) = field.take();
        let _ = k;
        assert!(h >= cold.nodes.len() as u64 && b > 0, "view {view}: the cold plan evaluates the field ({h}, {b}, {k})");
        let warm = plan_terrain_cached(&req, &field, &mut cache).unwrap();
        assert_eq!(field.take(), (0, 0, 0), "view {view}: a warm plan of {} unchanged tiles must not evaluate the field", warm.nodes.len());
        assert_eq!(warm, cold, "view {view}: the warm plan equals the cold plan");
        let uncached = plan_terrain_cached(&req, &field, &mut SelectionCache::new()).unwrap();
        assert_eq!(warm, uncached, "view {view}: the cached plan equals the plan of a fresh cache");
    }
}

// spec: LOD-009
#[test]
fn a_moved_camera_evaluates_only_the_tiles_it_has_not_seen() {
    let a = request(6, 16);
    let mut b = request(6, 16);
    // One step of a slow glide: most tiles are the same.
    b.camera.position = planet_core::PlanetFixed(b.camera.position.0 + b.camera.forward * 30.0);
    let field = Counting::new();
    let mut cache = SelectionCache::new();
    plan_terrain_cached(&a, &field, &mut cache).unwrap();
    field.take();
    let plan_b = plan_terrain_cached(&b, &field, &mut cache).unwrap();
    let (h, _, k) = field.take();
    assert!(
        h < plan_b.nodes.len() as u64 && k < plan_b.nodes.len() as u64,
        "tiles seen by the first plan are not evaluated again ({h}, {k} of {})",
        plan_b.nodes.len()
    );
}

// spec: LOD-009
#[test]
fn the_mesh_key_cache_starts_over_when_the_grid_size_changes() {
    let (c16, c32) = (request(5, 16), request(5, 32));
    let field = Counting::new();
    let mut cache = SelectionCache::new();
    let p16 = plan_terrain_cached(&c16, &field, &mut cache).unwrap();
    let p32 = plan_terrain_cached(&c32, &field, &mut cache).unwrap();
    let fresh32 = plan_terrain_cached(&c32, &field, &mut SelectionCache::new()).unwrap();
    assert_eq!(p32, fresh32, "keys of the new grid size, not the cached ones of cells 16");
    let key_of = |p: &planet_frame::TerrainPlan| p.nodes.iter().map(|n| n.mesh_key).collect::<Vec<_>>();
    assert_ne!(key_of(&p16), key_of(&p32));
    let again16 = plan_terrain_cached(&c16, &field, &mut cache).unwrap();
    assert_eq!(again16, p16);
}

// spec: LOD-009
#[test]
fn a_field_that_reads_cells_is_not_reused_because_an_edit_keeps_its_identity() {
    let req = request(5, 16);
    let field = Counting { cells: true, ..Counting::new() };
    let mut cache = SelectionCache::new();
    plan_terrain_cached(&req, &field, &mut cache).unwrap();
    field.take();
    let n = plan_terrain_cached(&req, &field, &mut cache).unwrap().nodes.len() as u64;
    let (h, _, k) = field.take();
    assert!(h >= n && k >= n, "per-tile reads are declared at plan time and cannot be cached ({h}, {k} of {n})");
}

// spec: LOD-009
#[test]
fn another_reference_surface_or_radius_starts_the_reused_values_over() {
    let req = request(5, 16);
    let mut cache = SelectionCache::new();
    let sphere = Counting::new();
    plan_terrain_cached(&req, &sphere, &mut cache).unwrap();
    let ellipsoid = Counting { surface: "ellipsoid-test", ..Counting::new() };
    let reused = plan_terrain_cached(&req, &ellipsoid, &mut cache).unwrap();
    assert_eq!(reused, plan_terrain_cached(&req, &ellipsoid, &mut SelectionCache::new()).unwrap());
    let mut larger = request(5, 16);
    larger.lod.radius_m += 1.0;
    let p = plan_terrain_cached(&larger, &sphere, &mut cache).unwrap();
    assert_eq!(p, plan_terrain_cached(&larger, &sphere, &mut SelectionCache::new()).unwrap());
}

// spec: LOD-009
#[test]
fn a_non_finite_origin_height_is_not_kept() {
    let req = request(5, 16);
    let field = Counting::new();
    field.nan_once.store(1, Relaxed);
    let mut cache = SelectionCache::new();
    let first = plan_terrain_cached(&req, &field, &mut cache).unwrap();
    assert!(first.nodes.iter().any(|n| n.camera_relative_origin[0].is_nan()));
    let second = plan_terrain_cached(&req, &field, &mut cache).unwrap();
    assert_eq!(second, plan_terrain_cached(&req, &field, &mut SelectionCache::new()).unwrap());
}
