//! REND-031: the test-only reference renderer draws a plan from CPU meshes in the debug views; it agrees with the compact renderer
//! within REND-016's model tolerance, and (while the vertex path exists) pixel for pixel with the vertex renderer, so the tests that
//! use it as their independent geometry source are meaningful.

use planet_core::cube::TangentWarp;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, vertex_tile_field};
use planet_generators::height::{HeightField, Reference};
use planet_render::reference::execute_reference;
use planet_render::{AdapterPolicy, GpuContext, RgbaFrame, TerrainRenderer};
use planet_streaming::PoolConfig;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const FINEST: u8 = 26;

fn pool() -> PoolConfig {
    let block_slots = 8 * 8;
    compact_format_pool_with(
        (128 << 20) + 40 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap()
}

fn plan_of(index: usize, view: TerrainView) -> TerrainPlan {
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    let camera = scripted_views(R, 4.0 / 3.0, 1)[index].1;
    plan_terrain(&TerrainRequest { camera, size: SIZE, view, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

fn differing(a: &RgbaFrame, b: &RgbaFrame, limit: u8) -> usize {
    a.rgba.chunks(4).zip(b.rgba.chunks(4)).filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > limit)).count()
}

fn reference_image(ctx: &GpuContext, field: &dyn HeightField, p: &TerrainPlan) -> RgbaFrame {
    let meshes: Vec<_> = p.nodes.iter().map(|n| tile_mesh_field(field, &TangentWarp, n.tile, CELLS, R)).collect();
    let refs: Vec<_> = meshes.iter().collect();
    execute_reference(ctx, p, &refs).unwrap()
}

// spec: REND-031
#[test]
fn the_reference_renderer_draws_the_images_of_the_compact_renderer_within_the_model_tolerance() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let field = Reference(1);
    let (mut compact_tiles, mut worst_level, mut worst_other) = (0usize, 0f64, 0f64);
    for view in [TerrainView::Level, TerrainView::Relief, TerrainView::Normals] {
        for index in 0..10 {
            let p = plan_of(index, view);
            let reference = reference_image(&ctx, &field, &p);
            let mut r = TerrainRenderer::with_compact_pools(&ctx, SIZE, CELLS, pool());
            for n in &p.nodes {
                r.upload_tile(&ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).unwrap();
            }
            r.render(&ctx, &p, false).unwrap();
            let image = r.read_color(&ctx);
            compact_tiles += r.format_counts().1;
            // The level view is flat colour per node: only edge pixels may differ. Relief and normals read the model error and the
            // 16-bit normals: lines move by a fraction of a pixel (the tolerances of REND-016).
            let wrong = differing(&reference, &image, 12) as f64 / (SIZE.0 * SIZE.1) as f64;
            if view == TerrainView::Level {
                worst_level = worst_level.max(wrong);
            } else {
                worst_other = worst_other.max(wrong);
            }
        }
    }
    println!("reference against compact: worst fraction over 12 levels: level {worst_level:.5}, relief/normals {worst_other:.5}; compact tiles {compact_tiles}");
    assert!(compact_tiles > 100, "the compact path must actually be exercised");
    assert!(worst_level <= 0.0005, "level view: {worst_level} of the pixels differ");
    assert!(worst_other <= 0.002, "relief/normals views: {worst_other} of the pixels differ");
}

// spec: REND-031
#[test]
fn the_reference_renderer_equals_the_vertex_renderer_pixel_for_pixel_in_every_debug_view() {
    // Removed with the vertex renderer (task 3.1 of remove-vertex-render-path): until then it proves the reference is the vertex stage
    // it replaces, so the comparison against compact above stands on an exact reference.
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let mut drawn_total = 0usize;
    for view in [TerrainView::Level, TerrainView::Relief, TerrainView::Normals, TerrainView::MorphHeight, TerrainView::Height] {
        for index in [0usize, 3, 6, 9] {
            let p = plan_of(index, view);
            let reference = reference_image(&ctx, &field, &p);
            let mut r = TerrainRenderer::new(&ctx, SIZE, CELLS);
            for n in &p.nodes {
                r.upload(&ctx, &tile_mesh_field(&field, &TangentWarp, n.tile, CELLS, R)).unwrap();
            }
            r.render(&ctx, &p, false).unwrap();
            let clear = [p.clear_color[0], p.clear_color[1], p.clear_color[2]];
            let drawn = reference.rgba.chunks(4).filter(|q| q[..3] != clear).count();
            assert!(drawn > 0, "{view:?} view {index}: the reference drew nothing");
            drawn_total += drawn;
            assert_eq!(
                differing(&reference, &r.read_color(&ctx), 0),
                0,
                "{view:?} view {index}: reference differs from the vertex renderer"
            );
        }
    }
    println!("pixels drawn over the 20 comparisons: {drawn_total}");
    assert!(drawn_total > 20 * SIZE.0 as usize * SIZE.1 as usize / 4, "the comparisons must draw terrain, not the clear colour");
}

// spec: REND-031
#[test]
fn the_reference_renderer_refuses_a_lit_plan() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut p = plan_of(0, TerrainView::Level);
    p.lit = Some(planet_frame::look::LitPlan::fixed(10.0));
    let e = execute_reference(&ctx, &p, &[]).unwrap_err();
    assert!(matches!(e, planet_render::ExecError::LitNotEnabled), "{e:?}");
}
