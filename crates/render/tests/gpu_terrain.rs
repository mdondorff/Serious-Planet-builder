//! Tier C: terrain from orbit to 1 m through the ten scripted views, in every debug view
//! (generator -> node selection -> TerrainPlan -> GPU -> pixels).

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::{level_color, plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::dump::FACE_COLORS;
use planet_generators::mesh::{grid_indices, tile_mesh, TileMesh};
use planet_render::{execute_terrain, AdapterPolicy, GpuContext};
use planet_testkit::{assert_golden, Image, Tolerance};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const SEED: u64 = 1;

fn lod() -> LodParams {
    LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 4.0, max_level: 26, ..LodParams::earth_1080p() }
}

fn plan_for(name_index: usize, view: TerrainView) -> (&'static str, TerrainPlan) {
    let (name, camera) = scripted_views(R, f64::from(SIZE.0) / f64::from(SIZE.1), SEED)[name_index];
    (name, plan_terrain(&TerrainRequest { camera, size: SIZE, view, lod: lod(), face_mapping: "tangent-v1".into() }).unwrap())
}

fn meshes_for(plan: &TerrainPlan) -> Vec<TileMesh> {
    plan.nodes.iter().map(|n| tile_mesh(SEED, &TangentWarp, n.tile, CELLS, R)).collect()
}

fn render(ctx: &GpuContext, plan: &TerrainPlan, meshes: &[TileMesh]) -> (Image, planet_render::TerrainStats) {
    let refs: Vec<&TileMesh> = meshes.iter().collect();
    let (frame, stats) = execute_terrain(ctx, plan, &refs).unwrap();
    (Image::new(frame.width, frame.height, frame.rgba), stats)
}

/// Pixels whose view ray hits the sphere just below the lowest terrain: they must show terrain, never the background.
fn holes(plan: &TerrainPlan, img: &Image) -> Vec<(u32, u32)> {
    let cam = plan.camera;
    let f = cam.forward.normalized();
    let r = f.cross(cam.up).normalized();
    let u = r.cross(f);
    let ty = libm::tan(cam.fov_y_rad / 2.0);
    // Terrain lies within +-4000 m; a coarse mesh also cuts corners (sagitta of its widest cell), which moves the silhouette in.
    let coarsest = plan.nodes.iter().map(|n| n.level).min().unwrap_or(0);
    let cell_angle = 1.3 * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << coarsest) / f64::from(CELLS);
    let sagitta = R * (1.0 - libm::cos(cell_angle / 2.0));
    let inner = R - 4100.0 - sagitta;
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

// spec: LOD-005, REND-001
#[test]
fn ten_scripted_views_have_no_holes_and_exact_draw_counts() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    eprintln!("adapter: {:?} {}", ctx.info.backend, ctx.info.name);
    let index_triangles = (grid_indices(CELLS).len() / 3) as u64;
    for i in 0..10 {
        let (name, plan) = plan_for(i, TerrainView::TileId);
        let meshes = meshes_for(&plan);
        let (img, stats) = render(&ctx, &plan, &meshes);
        assert_eq!(stats.draw_calls as usize, plan.nodes.len(), "{name}: draw calls must equal planned nodes");
        assert_eq!(stats.triangles, plan.nodes.len() as u64 * index_triangles, "{name}");
        let h = holes(&plan, &img);
        assert!(
            h.is_empty(),
            "{name}: {} hole pixels inside the planet, first at {:?} (tile-id view, {} nodes)",
            h.len(),
            h[0],
            plan.nodes.len()
        );
        // The planet must actually be visible in every view.
        let covered = img.pixels().filter(|p| *p != plan.clear_color).count();
        assert!(covered > 100, "{name}: only {covered} pixels show terrain");
        let (_, again) = (0, render(&ctx, &plan, &meshes).0);
        assert_eq!(img, again, "{name}: rendering is not deterministic");
    }
}

// spec: REND-006, TEST-010
#[test]
fn every_debug_view_renders_and_obeys_its_colour_rules() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for view in TerrainView::ALL {
        let (name, plan) = plan_for(3, view); // aerial-20km
        let meshes = meshes_for(&plan);
        let (img, _) = render(&ctx, &plan, &meshes);
        let covered: Vec<[u8; 4]> = img.pixels().filter(|p| *p != plan.clear_color).collect();
        assert!(covered.len() > 500, "{name} {}: only {} terrain pixels", view.name(), covered.len());
        match view {
            TerrainView::Face => assert!(covered.iter().all(|p| FACE_COLORS.contains(p)), "face view must only use face colours"),
            TerrainView::Level => {
                let allowed: Vec<[u8; 4]> = (0..=26).map(level_color).collect();
                assert!(covered.iter().all(|p| allowed.contains(p)), "level view must only use level colours");
                let distinct: std::collections::BTreeSet<_> = covered.iter().collect();
                assert!(distinct.len() >= 3, "a perspective view must show several levels, got {}", distinct.len());
            }
            TerrainView::Normals => assert!(covered.iter().any(|p| p[0] != p[1] || p[1] != p[2]), "normals view must be coloured"),
            TerrainView::Height | TerrainView::Morph | TerrainView::Depth => {
                assert!(covered.iter().all(|p| p[0] == p[1] && p[1] == p[2]), "{} view must be grey", view.name());
            }
            TerrainView::TileId => {}
        }
    }
}

// spec: REND-003
#[test]
fn depth_view_is_reversed_z_near_is_brighter_than_far() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (name, plan) = plan_for(5, TerrainView::Depth); // ground-200m, looking along the ground
    let meshes = meshes_for(&plan);
    let (img, _) = render(&ctx, &plan, &meshes);
    // Mean grey of terrain pixels in the bottom quarter (near ground) versus the band just under the horizon.
    let mean = |rows: std::ops::Range<u32>| {
        let g: Vec<f64> = rows
            .flat_map(|y| (0..img.width).map(move |x| (x, y)))
            .map(|(x, y)| img.pixel(x, y))
            .filter(|p| *p != plan.clear_color)
            .map(|p| f64::from(p[0]))
            .collect();
        assert!(!g.is_empty(), "{name}: no terrain pixels in the sampled rows");
        g.iter().sum::<f64>() / g.len() as f64
    };
    let (near, far) = (mean(img.height * 3 / 4..img.height), mean(img.height / 3..img.height / 2));
    assert!(near > far + 5.0, "{name}: near terrain ({near}) must be brighter than far terrain ({far}) in reversed-Z");
}

// spec: REND-006, TEST-010
#[test]
fn selected_views_match_the_adapters_goldens() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for (index, view, golden) in [
        (0, TerrainView::TileId, "terrain_orbit_tile_id"),
        (3, TerrainView::Level, "terrain_aerial20km_level"),
        (5, TerrainView::Height, "terrain_ground200m_height"),
        (8, TerrainView::Normals, "terrain_cube_corner_normals"),
        (6, TerrainView::Depth, "terrain_ground1m_depth"),
        (4, TerrainView::Morph, "terrain_low2km_morph"),
    ] {
        let (_, plan) = plan_for(index, view);
        let meshes = meshes_for(&plan);
        let (img, _) = render(&ctx, &plan, &meshes);
        assert_golden(&ctx.adapter_key(), golden, &img, Tolerance::Exact);
    }
}
