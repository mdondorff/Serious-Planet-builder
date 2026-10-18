//! Tier C: terrain from orbit to 1 m through the ten scripted views, in every debug view
//! (generator -> node selection -> TerrainPlan -> GPU -> pixels).

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::Camera;
use planet_frame::{level_color, plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::dump::FACE_COLORS;
use planet_generators::height::Reference;
use planet_generators::mesh::{grid_indices, TileMesh};
use planet_render::{execute_terrain_tiles, AdapterPolicy, GpuContext};
use planet_testkit::{GoldenChecks, Image, Tolerance};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const SEED: u64 = 1;

fn lod() -> LodParams {
    LodParams {
        cells: CELLS,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 2.5,
        max_level: planet_frame::production_finest_level(CELLS),
        ..LodParams::earth_1080p()
    }
}

fn plan_for(name_index: usize, view: TerrainView) -> (&'static str, TerrainPlan) {
    let (name, camera) = scripted_views(R, f64::from(SIZE.0) / f64::from(SIZE.1), SEED)[name_index];
    (name, plan_terrain(&TerrainRequest { camera, size: SIZE, view, lod: lod(), face_mapping: "tangent-v1".into(), seed: 1 }).unwrap())
}

/// The tiles of the plan in the product's format: compact for levels 3 and finer, the vertex format below (REND-016).
fn meshes_for(plan: &TerrainPlan) -> Vec<CompactTile> {
    tiles_with(plan, CELLS)
}

fn tiles_with(plan: &TerrainPlan, cells: u32) -> Vec<CompactTile> {
    plan.nodes.iter().map(|n| compact_tile_field(&Reference(SEED), &TangentWarp, n.tile, cells, R)).collect()
}

/// The finest level the test plans allow (`lod().max_level`).
fn plan_finest(_plan: &TerrainPlan) -> u8 {
    lod().max_level
}

fn render(ctx: &GpuContext, plan: &TerrainPlan, tiles: &[CompactTile]) -> (Image, planet_render::TerrainStats) {
    let refs: Vec<&CompactTile> = tiles.iter().collect();
    let (frame, stats) = execute_terrain_tiles(ctx, plan, &refs).unwrap();
    (Image::new(frame.width, frame.height, frame.rgba), stats)
}

/// The vertex-format meshes of the plan: the reference path, for the tests that read vertices on the CPU.
fn vertex_meshes_for(plan: &TerrainPlan) -> Vec<TileMesh> {
    plan.nodes
        .iter()
        .map(|n| planet_generators::compact::tile_mesh_field(&Reference(SEED), &TangentWarp, n.tile, CELLS, R))
        .collect()
}

fn render_reference(ctx: &GpuContext, plan: &TerrainPlan, meshes: &[TileMesh]) -> Image {
    let refs: Vec<&TileMesh> = meshes.iter().collect();
    let frame = planet_render::reference::execute_reference(ctx, plan, &refs).unwrap();
    Image::new(frame.width, frame.height, frame.rgba)
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

// spec: LOD-005, REND-001, TEST-011
#[test]
fn ten_scripted_views_have_no_holes_and_exact_draw_counts() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    eprintln!("{}", ctx.adapter_log_line());
    let index_triangles = (grid_indices(CELLS).len() / 3) as u64;
    for i in 0..10 {
        let (name, plan) = plan_for(i, TerrainView::TileId);
        let meshes = meshes_for(&plan);
        let (img, stats) = render(&ctx, &plan, &meshes);
        assert_eq!(stats.draw_calls as usize, plan.nodes.len(), "{name}: draw calls must equal planned nodes");
        assert_eq!(stats.triangles, plan.nodes.len() as u64 * index_triangles, "{name}");
        // The renderer matches the counts the plan implies (TEST-011): tier A asserts the budgets on the plan, tier C
        // checks that what the GPU path did equals them.
        let counts = plan.counts();
        assert_eq!((stats.draw_calls, stats.triangles), (counts.draw_calls, counts.triangles), "{name}");
        // The plan's upload bytes are the vertex-format tiles (the reference count); the product uploads each tile in its own format:
        // texels and record for a compact tile, vertices for levels 0-2 and fallbacks.
        let expected: u64 = meshes
            .iter()
            .map(|t| match t {
                RenderTile::Compact(c) => c.resident_bytes(),
                RenderTile::Vertex(m) => m.vertices.len() as u64 * planet_generators::mesh::MeshVertex::GPU_STRIDE,
            })
            .sum();
        assert_eq!(stats.vertex_bytes, expected, "{name}: bytes uploaded");
        assert!(stats.vertex_bytes <= counts.upload_bytes, "{name}: the compact format never uploads more than the vertex format");
        // The vertex format is the reference: the product's compact image equals it up to edge pixels.
        let reference = render_reference(&ctx, &plan, &vertex_meshes_for(&plan));
        let wrong = img.pixels().zip(reference.pixels()).filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > 12)).count();
        assert!(wrong * 1000 <= (img.width * img.height) as usize, "{name}: {wrong} pixels differ from the vertex-format image");
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
            TerrainView::Height | TerrainView::Depth | TerrainView::MorphHeight => {
                assert!(covered.iter().all(|p| p[0] == p[1] && p[1] == p[2]), "{} view must be grey", view.name());
            }
            TerrainView::Morph => {
                // Red is the morph value and blue its complement (up to rounding), so the view is a blue-to-red ramp.
                assert!(covered.iter().all(|p| (i32::from(p[0]) + i32::from(p[2]) - 255).abs() <= 1), "morph view must be a blue-red ramp");
            }
            TerrainView::Relief => {
                // Hillshade, tint, grid and contours: a coloured image with many distinct values, not a few flat colours.
                let distinct: std::collections::BTreeSet<_> = covered.iter().collect();
                assert!(distinct.len() > 200, "relief view must show structure, got {} distinct colours", distinct.len());
            }
            TerrainView::TileId => {}
        }
    }
}

/// Share of the terrain pixels that belong to a wide dark band: dark pixels (clearly darker than the typical terrain
/// pixel) at the centre of a `size` x `size` window that is at least 80 % dark. A thin line, or the crossing of two thin
/// lines, fills far less of such a window whatever its slope; a band at least `size` pixels wide fills it.
fn wide_dark_share(img: &Image, clear: [u8; 4], size: u32) -> f64 {
    let luma = |p: [u8; 4]| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]);
    let mut lumas: Vec<f64> = img.pixels().filter(|p| *p != clear).map(luma).collect();
    if lumas.is_empty() {
        return 0.0;
    }
    lumas.sort_by(f64::total_cmp);
    let dark_below = 0.75 * lumas[lumas.len() / 2];
    let (w, h) = (img.width as usize, img.height as usize);
    let dark: Vec<bool> = (0..w * h)
        .map(|i| {
            let p = img.pixel((i % w) as u32, (i / w) as u32);
            p != clear && luma(p) < dark_below
        })
        .collect();
    // Summed-area table of the dark mask: sat[(y + 1) * (w + 1) + x + 1] counts dark pixels in [0, x] x [0, y].
    let mut sat = vec![0u32; (w + 1) * (h + 1)];
    for y in 0..h {
        for x in 0..w {
            sat[(y + 1) * (w + 1) + x + 1] =
                u32::from(dark[y * w + x]) + sat[y * (w + 1) + x + 1] + sat[(y + 1) * (w + 1) + x] - sat[y * (w + 1) + x];
        }
    }
    let half = (size / 2) as usize;
    let mut wide = 0usize;
    for y in half..h.saturating_sub(half) {
        for x in half..w.saturating_sub(half) {
            if !dark[y * w + x] {
                continue;
            }
            let (x0, y0, x1, y1) = (x - half, y - half, x - half + size as usize, y - half + size as usize);
            let count = sat[y1 * (w + 1) + x1] + sat[y0 * (w + 1) + x0] - sat[y0 * (w + 1) + x1] - sat[y1 * (w + 1) + x0];
            if f64::from(count) >= 0.8 * f64::from(size * size) {
                wide += 1;
            }
        }
    }
    wide as f64 / lumas.len() as f64
}

/// The relief view of the ground-1m camera as it looked before the grid fix: a wide dark band (the 10 m grid decade lost
/// centimetres to f32 rounding, which made plateaus) along the diagonal. 256x192, rendered by `test-render --script 6`.
const BAND_BEFORE_FIX: &str = "tests/data/relief_band_before_fix.png";

// spec: REND-006
#[test]
fn the_band_detector_flags_the_pre_fix_relief_image() {
    let img = planet_testkit::read_png(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BAND_BEFORE_FIX)).unwrap();
    let share = wide_dark_share(&img, planet_frame::plan::CLEAR_COLOR, 6);
    assert!(share > 0.01, "the detector must flag the known defect image (wide dark share {share:.5}, expected about 0.018)");
}

// spec: REND-006
#[test]
fn the_relief_grid_has_only_thin_lines_at_the_ground_cameras() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [5usize, 6] {
        let (name, camera) = scripted_views(R, 256.0 / 192.0, SEED)[index];
        let lod = LodParams {
            cells: 16,
            viewport_h_px: 192.0,
            tau_px: 4.0,
            max_level: planet_frame::production_finest_level(16),
            ..LodParams::earth_1080p()
        };
        let plan = plan_terrain(&TerrainRequest {
            camera,
            size: (256, 192),
            view: TerrainView::Relief,
            lod,
            face_mapping: "tangent-v1".into(),
            seed: 1,
        })
        .unwrap();
        let meshes = tiles_with(&plan, 16);
        let (img, _) = render(&ctx, &plan, &meshes);
        let share = wide_dark_share(&img, plan.clear_color, 6);
        assert!(share < 0.001, "{name}: {:.2} % of the terrain is a wide dark band, not thin grid lines (grid precision?)", share * 100.0);
    }
}

// spec: REND-006
#[test]
fn the_relief_view_renders_identically_twice() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [0usize, 6] {
        let (name, plan) = plan_for(index, TerrainView::Relief);
        let meshes = meshes_for(&plan);
        assert_eq!(render(&ctx, &plan, &meshes).0, render(&ctx, &plan, &meshes).0, "{name}: the relief view is not deterministic");
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
    // Every golden is checked before the test fails, so one run writes every candidate (ADR 0017).
    let mut goldens = GoldenChecks::new(&ctx.adapter_key(), &ctx.driver_string());
    for (index, view, golden) in [
        (0, TerrainView::TileId, "terrain_orbit_tile_id"),
        (3, TerrainView::Level, "terrain_aerial20km_level"),
        (2, TerrainView::Height, "terrain_aerial200km_height"),
        (6, TerrainView::Depth, "terrain_ground1m_depth"),
        (4, TerrainView::Morph, "terrain_low2km_morph"),
    ] {
        let (_, plan) = plan_for(index, view);
        let meshes = meshes_for(&plan);
        let (img, _) = render(&ctx, &plan, &meshes);
        goldens.check(golden, &img, Tolerance::Exact);
    }
    // The normals golden looks straight down at the cube corner from 3,000 km: the normal colour then changes across
    // all three faces, and a seam would show as a step. (The ground cameras only see a plane.)
    let plan = corner_normals_plan();
    let (img, _) = render(&ctx, &plan, &meshes_for(&plan));
    goldens.check("terrain_cube_corner_normals", &img, Tolerance::Exact);
    goldens.finish();
}

fn corner_normals_plan() -> TerrainPlan {
    let (la, lo) = (35.264_389_682_754_654f64.to_radians(), 45f64.to_radians());
    let dir = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
    let camera = Camera::at_surface_point(
        PlanetFixed(dir * (R + 3.0e6)),
        Vec3::new(0.0, 0.0, 1.0),
        85f64.to_radians(),
        60f64.to_radians(),
        f64::from(SIZE.0) / f64::from(SIZE.1),
        0.1,
    );
    plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Normals, lod: lod(), face_mapping: "tangent-v1".into(), seed: 1 })
        .unwrap()
}

// spec: REND-006
#[test]
fn height_contours_and_depth_bands_are_visible() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // Dark pixels inside a smooth area of the ramp are the lines: compare against the same view's local neighbours.
    for (index, view) in [(2usize, TerrainView::Height), (6, TerrainView::Depth)] {
        let (name, plan) = plan_for(index, view);
        let (img, _) = render(&ctx, &plan, &meshes_for(&plan));
        let grey = |x: u32, y: u32| f64::from(img.pixel(x, y)[0]);
        let mut lines = 0;
        for y in 1..img.height - 1 {
            for x in 1..img.width - 1 {
                // Skip the horizon: next to the sky colour every terrain pixel looks like a dip.
                if [(x, y), (x, y - 1), (x, y + 1)].iter().any(|&(px, py)| img.pixel(px, py) == plan.clear_color) {
                    continue;
                }
                let (c, a, b) = (grey(x, y), grey(x, y - 1), grey(x, y + 1));
                // A line is a local dip against both vertical neighbours.
                if a - c > 12.0 && b - c > 12.0 {
                    lines += 1;
                }
            }
        }
        assert!(lines >= 20, "{name} {}: only {lines} line pixels; contours or bands are missing", view.name());
    }
}

/// Pixels that are a local luminance dip against both vertical neighbours (a thin dark line: grid or contour), skipping the
/// horizon.
fn relief_line_pixels(img: &Image, clear: [u8; 4]) -> usize {
    let luma = |x: u32, y: u32| {
        let p = img.pixel(x, y);
        0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2])
    };
    let mut lines = 0;
    for y in 1..img.height - 1 {
        for x in 0..img.width {
            if [(x, y), (x, y - 1), (x, y + 1)].iter().any(|&(px, py)| img.pixel(px, py) == clear) {
                continue;
            }
            let c = luma(x, y);
            if luma(x, y - 1) - c > 6.0 && luma(x, y + 1) - c > 6.0 {
                lines += 1;
            }
        }
    }
    lines
}

// spec: REND-006, TEST-014
#[test]
fn the_relief_view_shows_terrain_below_the_horizon_with_grid_and_contour_lines() {
    // The automatic check of the relief view at both flight-review cameras (TEST-014: the owner's review of the 1280x720
    // flight is the other half). Removing the grid and contours from the relief shader makes the line count fall to zero.
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [0usize, 6] {
        let (name, plan) = plan_for(index, TerrainView::Relief);
        let (img, _) = render(&ctx, &plan, &meshes_for(&plan));
        let h = holes(&plan, &img);
        assert!(h.is_empty(), "{name}: {} clear-colour pixels below the horizon, first at {:?}", h.len(), h[0]);
        let lines = relief_line_pixels(&img, plan.clear_color);
        eprintln!("{name}: {lines} line pixels");
        assert!(lines >= 20, "{name}: only {lines} line pixels; the relief grid and contours are missing");
    }
}

// spec: REND-003
#[test]
fn nearer_terrain_hides_farther_terrain_regardless_of_draw_order() {
    // Without horizon culling the far side of the planet is drawn too, and projects onto the same pixels as the near
    // side. With reversed-Z (Greater, clear 0) the picture must equal the culled one; with a broken depth test the
    // far nodes drawn later would overwrite it.
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (_, camera) = scripted_views(R, f64::from(SIZE.0) / f64::from(SIZE.1), SEED)[0];
    let mk = |cull: bool| {
        let lod = LodParams { cull_horizon: cull, ..lod() };
        plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::TileId, lod, face_mapping: "tangent-v1".into(), seed: SEED })
            .unwrap()
    };
    let (culled, all) = (mk(true), mk(false));
    assert!(
        all.nodes.len() > culled.nodes.len(),
        "the uncull view must add far-side nodes ({} vs {})",
        all.nodes.len(),
        culled.nodes.len()
    );
    let (a, _) = render(&ctx, &culled, &meshes_for(&culled));
    let (b, _) = render(&ctx, &all, &meshes_for(&all));
    assert_eq!(a, b, "far-side terrain showed through nearer terrain: the depth test is not reversed-Z");
}

// spec: REND-001, REND-003
#[test]
fn nodes_appear_where_their_vertices_project() {
    // Oracle for the vertex stage: the pixel at the f64 projection of a node's centre vertex must show that node (or a
    // nearer one). Misaligned vertex data or a wrong matrix puts the geometry elsewhere and fails nearly every node.
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [1usize, 2, 3, 4] {
        let (name, plan) = plan_for(index, TerrainView::TileId);
        // The CPU oracle reads the canonical vertices; the image comes from the product's compact tiles.
        let meshes = vertex_meshes_for(&plan);
        let (img, _) = render(&ctx, &plan, &meshes_for(&plan));
        let (mut inside, mut matched) = (0, 0);
        for (d, m) in plan.nodes.iter().zip(&meshes) {
            let v = m.vertices[(CELLS as usize / 2) * (CELLS as usize + 1) + CELLS as usize / 2];
            let rel = d.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2])) - plan.camera.position.0;
            let c = plan.camera.project(rel);
            // Only nodes seen reasonably face-on: grazing slivers can be thinner than a pixel.
            let normal = Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]));
            if c[3] <= 0.0 || normal.dot((-rel).normalized()) < 0.5 {
                continue;
            }
            let (x, y) = (c[0] / c[3], c[1] / c[3]);
            if x.abs() > 0.98 || y.abs() > 0.98 {
                continue;
            }
            let px = ((x * 0.5 + 0.5) * f64::from(SIZE.0)) as u32;
            let py = ((0.5 - y * 0.5) * f64::from(SIZE.1)) as u32;
            inside += 1;
            let colour = img.pixel(px, py);
            // The pixel shows this node, or something nearer (any node's colour except the background).
            if colour == d.color {
                matched += 1;
            }
            if colour == plan.clear_color {
                panic!("{name}: the centre vertex of {:?} projects to ({px},{py}) but the pixel is background", d.tile);
            }
        }
        assert!(inside >= 5, "{name}: only {inside} nodes project inside the frame");
        assert!(
            matched * 10 >= inside * 7,
            "{name}: only {matched} of {inside} nodes show their own colour at their projected centre vertex"
        );
    }
}

// spec: REND-009, LOD-004
#[test]
fn gpu_morph_values_match_the_cpu_formula_at_projected_vertices() {
    // Oracle for the vertex-stage morph: read the morph view at the pixel where a vertex projects and compare with the
    // f64 formula on the vertex position. A misread attribute (garbage pos) fails nearly all.
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [2usize, 3, 4] {
        let (name, plan) = plan_for(index, TerrainView::Morph);
        // The CPU oracle reads the canonical vertices; the image comes from the product's compact tiles.
        let meshes = vertex_meshes_for(&plan);
        let (img, _) = render(&ctx, &plan, &meshes_for(&plan));
        let (mut tested, mut close) = (0, 0);
        for (d, m) in plan.nodes.iter().zip(&meshes) {
            if d.level == 0 {
                continue;
            }
            for (i, j) in [(4usize, 4usize), (8, 8), (12, 4), (4, 12), (12, 12)] {
                let v = m.vertices[j * (CELLS as usize + 1) + i];
                let rel = d.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2])) - plan.camera.position.0;
                let c = plan.camera.project(rel);
                let normal = Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]));
                if c[3] <= 0.0 || normal.dot((-rel).normalized()) < 0.5 {
                    continue;
                }
                let (x, y) = (c[0] / c[3], c[1] / c[3]);
                if x.abs() > 0.95 || y.abs() > 0.95 {
                    continue;
                }
                let (px, py) = (((x * 0.5 + 0.5) * f64::from(SIZE.0)) as u32, ((0.5 - y * 0.5) * f64::from(SIZE.1)) as u32);
                let reference =
                    d.origin.0 - plan.camera.position.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]));
                let expected =
                    ((reference.length() - f64::from(d.morph_start_m)) / f64::from(d.morph_end_m - d.morph_start_m)).clamp(0.0, 1.0);
                let got = f64::from(img.pixel(px, py)[0]) / 255.0;
                tested += 1;
                if (got - expected).abs() <= 0.06 {
                    close += 1;
                }
            }
        }
        assert!(tested >= 20, "{name}: only {tested} vertices could be compared");
        assert!(close * 10 >= tested * 9, "{name}: GPU morph agrees with the CPU formula at only {close} of {tested} vertices");
    }
}
