//! Tier C, TEST-015: the ten scripted views on the steep stress field (generators::stress::Steep) render without holes
//! (generator -> node selection -> TerrainPlan -> GPU -> pixels), on `PLANET_ADAPTER`.

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
use planet_generators::mesh::{tile_mesh_field, TileMesh};
use planet_generators::stress::Steep;
use planet_render::{execute_terrain, AdapterPolicy, GpuContext};
use planet_testkit::Image;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
/// The steep field needs few cells per tile and a large split distance for a crack-free morph interval to exist.
const CELLS: u32 = 4;
const STEEP: Steep = Steep(1);

/// Pixels whose view ray hits the sphere just below the lowest steep terrain but show the background.
fn holes(plan: &TerrainPlan, img: &Image) -> Vec<(u32, u32)> {
    let cam = plan.camera;
    let f = cam.forward.normalized();
    let r = f.cross(cam.up).normalized();
    let u = r.cross(f);
    let ty = libm::tan(cam.fov_y_rad / 2.0);
    let coarsest = plan.nodes.iter().map(|n| n.level).min().unwrap_or(0);
    let cell_angle = 1.3 * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << coarsest) / f64::from(CELLS);
    let sagitta = R * (1.0 - libm::cos(cell_angle / 2.0));
    let inner = R - STEEP.max_abs_height_m() - 100.0 - sagitta;
    let o = cam.position.0;
    let mut out = Vec::new();
    for py in 0..img.height {
        for px in 0..img.width {
            let x = (f64::from(px) + 0.5) / f64::from(img.width) * 2.0 - 1.0;
            let y = 1.0 - (f64::from(py) + 0.5) / f64::from(img.height) * 2.0;
            let dir: Vec3 = (f + r * (x * ty * cam.aspect) + u * (y * ty)).normalized();
            let b = o.dot(dir);
            let disc = b * b - (o.dot(o) - inner * inner);
            if disc >= 0.0 && -b - disc.sqrt() > 0.0 && img.pixel(px, py) == plan.clear_color {
                out.push((px, py));
            }
        }
    }
    out
}

// spec: TEST-015
#[test]
fn ten_scripted_views_on_steep_terrain_have_no_holes() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    eprintln!("{}", ctx.adapter_log_line());
    let lod = LodParams { cells: CELLS, viewport_h_px: 2.0 * f64::from(SIZE.1), tau_px: 2.5, max_level: 19, ..LodParams::earth_1080p() };
    for (name, camera) in scripted_views_field(R, f64::from(SIZE.0) / f64::from(SIZE.1), &STEEP) {
        let req =
            TerrainRequest { camera, size: SIZE, view: TerrainView::TileId, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 1 };
        let t0 = std::time::Instant::now();
        let plan = plan_terrain_field(&req, &STEEP).unwrap();
        let t_plan = t0.elapsed();
        // Meshes in parallel (each a pure function of its tile, collected in node order): most nodes are tiny tiles near
        // the camera, and generating them one by one dominated the test's time.
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let meshes: Vec<TileMesh> = std::thread::scope(|s| {
            let handles: Vec<_> = plan
                .nodes
                .chunks(plan.nodes.len().div_ceil(threads).max(1))
                .map(|part| {
                    s.spawn(move || part.iter().map(|n| tile_mesh_field(&STEEP, &TangentWarp, n.tile, CELLS, R)).collect::<Vec<_>>())
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        let t_mesh = t0.elapsed();
        let refs: Vec<&TileMesh> = meshes.iter().collect();
        let (frame, _) = execute_terrain(&ctx, &plan, &refs).unwrap();
        eprintln!("{name}: {} nodes, plan {t_plan:?}, meshes {:?}, render {:?}", plan.nodes.len(), t_mesh - t_plan, t0.elapsed() - t_mesh);
        let img = Image::new(frame.width, frame.height, frame.rgba);
        // Every view is written for review (not a golden): target/review/steep/<adapter>/<view>.png.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/review/steep").join(ctx.adapter_key());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.png"));
        planet_testkit::write_png(&path, &img).unwrap();
        let h = holes(&plan, &img);
        if !h.is_empty() {
            panic!(
                "{name}: {} hole pixels on steep terrain, first at {:?} ({} nodes); image {}",
                h.len(),
                h[0],
                plan.nodes.len(),
                path.display()
            );
        }
        let covered = img.pixels().filter(|p| *p != plan.clear_color).count();
        assert!(covered > 100, "{name}: only {covered} pixels show terrain");
    }
}
