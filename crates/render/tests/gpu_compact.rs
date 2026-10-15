//! REND-016: the compact tile path (atlases of `Rgba16Uint` texels, the vertex stage placing vertices from texels, exact morph
//! targets) draws the images of the vertex path within the stated model tolerance, on every adapter; fallback tiles (CON-13) and
//! levels 0-2 stay in the vertex format in the same frame; the per-draw loop and the indirect multi-draw agree; the allocation
//! equals the pure model.

use planet_core::cube::TangentWarp;
use planet_frame::pool::{atlas_columns, compact_format_pool_with};
use planet_frame::{
    plan_terrain, plan_terrain_field, scripted_views, scripted_views_field, LodParams, TerrainPlan, TerrainRequest, TerrainView,
};
use planet_generators::compact::{tile_data_field, RenderTile};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::tile_mesh_field;
use planet_generators::stress::Steep;
use planet_render::{AdapterPolicy, DrawMode, GpuContext, ResidentFormat, RgbaFrame, TerrainRenderer};
use planet_streaming::{PoolConfig, PoolKind, SlotRef};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);

/// Small compact pools: atlases of 8 x 8 slots, so a plan spans several atlases.
fn small_pool(cells: u32) -> PoolConfig {
    let side = 8 * (cells + 1);
    // The 128 MiB reserve of the fixed items plus 40 atlases, plenty for these plans.
    let one = compact_format_pool_with((128 << 20) + 40 * u64::from(side) * u64::from(side) * 8, cells, side, 1).unwrap();
    // A roomy vertex pool: the steep fixture makes many fallback tiles.
    PoolConfig { vertex_slots: 4096, fallback_slots: 3968, ..one }
}

fn plan_of(field: &dyn HeightField, cells: u32, max_level: u8, index: usize, view: TerrainView, steep: bool) -> TerrainPlan {
    let lod =
        LodParams { cells, viewport_h_px: if cells >= 32 { 192.0 } else { 96.0 }, tau_px: 2.5, max_level, ..LodParams::earth_1080p() };
    let camera = if steep { scripted_views_field(R, 4.0 / 3.0, field)[index].1 } else { scripted_views(R, 4.0 / 3.0, 1)[index].1 };
    let req = TerrainRequest { camera, size: SIZE, view, lod, face_mapping: "tangent-v1".into(), seed: 1 };
    if steep {
        plan_terrain_field(&req, field).unwrap()
    } else {
        plan_terrain(&req).unwrap()
    }
}

/// Pixels that differ by more than `limit` levels in some colour channel.
fn differing(a: &RgbaFrame, b: &RgbaFrame, limit: u8) -> usize {
    a.rgba.chunks(4).zip(b.rgba.chunks(4)).filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > limit)).count()
}

fn vertex_render(ctx: &GpuContext, field: &dyn HeightField, p: &TerrainPlan) -> RgbaFrame {
    let mut r = TerrainRenderer::new(ctx, SIZE, p.cells);
    for n in &p.nodes {
        r.upload(ctx, &tile_mesh_field(field, &TangentWarp, n.tile, p.cells, R)).unwrap();
    }
    r.render(ctx, p, false).unwrap();
    r.read_color(ctx)
}

fn compact_renderer(ctx: &GpuContext, field: &dyn HeightField, p: &TerrainPlan, finest: u8, scattered: bool) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_compact_pools(ctx, SIZE, p.cells, small_pool(p.cells));
    let (mut main, mut vertex) = (0u32, 0u32);
    for n in &p.nodes {
        let data = tile_data_field(field, &TangentWarp, n.tile, p.cells, R, finest);
        if scattered {
            // Manager-style placement: levels 0-2 in the managed vertex pool, the rest spread over the atlases from the top.
            let slot = if n.tile.level() <= 2 {
                vertex += 1;
                SlotRef { pool: PoolKind::Vertex, index: vertex - 1 }
            } else {
                main = (main + 37) % (40 * 64);
                SlotRef { pool: PoolKind::Main, index: main }
            };
            r.upload_tile_at(ctx, &data, slot).unwrap();
        } else {
            r.upload_tile(ctx, &data).unwrap();
        }
    }
    r
}

// spec: REND-016
#[test]
fn compact_tiles_draw_the_images_of_the_vertex_format_within_the_model_tolerance() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let field = Reference(1);
    let cells = 16;
    let (mut compact_tiles, mut worst_level, mut worst_other) = (0usize, 0f64, 0f64);
    for view in [TerrainView::Level, TerrainView::Relief, TerrainView::Normals] {
        for index in 0..10 {
            let p = plan_of(&field, cells, 26, index, view, false);
            let reference = vertex_render(&ctx, &field, &p);
            let mut r = compact_renderer(&ctx, &field, &p, 26, false);
            r.render(&ctx, &p, false).unwrap();
            let image = r.read_color(&ctx);
            let (_, c, _) = r.format_counts();
            compact_tiles += c;
            // The level view is flat colour per node: only edge pixels may differ. Relief and normals read the model error and the
            // 16-bit normals: lines move by a fraction of a pixel.
            let wrong = differing(&reference, &image, 12) as f64 / (SIZE.0 * SIZE.1) as f64;
            if view == TerrainView::Level {
                worst_level = worst_level.max(wrong);
            } else {
                worst_other = worst_other.max(wrong);
            }
        }
    }
    println!("compact against vertex: worst fraction of pixels over 12 levels: level {worst_level:.5}, relief/normals {worst_other:.5}; compact tiles drawn {compact_tiles}");
    assert!(compact_tiles > 100, "the compact path must actually be exercised");
    assert!(worst_level <= 0.0005, "level view: {worst_level} of the pixels differ");
    assert!(worst_other <= 0.002, "relief/normals views: {worst_other} of the pixels differ");
}

// spec: REND-016, REND-015
#[test]
fn the_compact_path_is_identical_per_draw_and_indirect_wherever_the_tiles_sit() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let cells = 16;
    for index in [1usize, 4, 6, 8] {
        let p = plan_of(&field, cells, 26, index, TerrainView::Relief, false);
        let mut free_list_image = None;
        for scattered in [false, true] {
            let mut r = compact_renderer(&ctx, &field, &p, 26, scattered);
            assert_eq!(r.set_draw_mode(&ctx, DrawMode::PerDraw), DrawMode::PerDraw);
            let (_, per_draw) = r.render(&ctx, &p, false).unwrap();
            let reference = r.read_color(&ctx);
            assert_eq!(per_draw.encoded_draws as usize, p.nodes.len());
            let compact_nodes = p.nodes.iter().filter(|n| n.tile.level() > 2).count() as u64;
            assert_eq!(per_draw.tile_ref_bytes, 16 * p.nodes.len() as u64);
            if ctx.indirect_draw_supported() {
                r.set_draw_mode(&ctx, DrawMode::Indirect);
                let (_, indirect) = r.render(&ctx, &p, false).unwrap();
                assert_eq!(r.read_color(&ctx), reference, "view {index} scattered {scattered}: indirect differs from per-draw");
                assert!(indirect.encoded_draws >= 1 && indirect.encoded_draws <= indirect.draw_calls);
                if scattered && compact_nodes > 3 {
                    assert!(indirect.encoded_draws > 1, "nodes in several atlases need several multi-draws: {indirect:?}");
                }
            }
            // Where a tile sits does not change the image.
            match &free_list_image {
                None => free_list_image = Some(reference),
                Some(first) => assert_eq!(&reference, first, "view {index}: scattering the slots changed the image"),
            }
        }
    }
}

// spec: REND-016
#[test]
fn fallback_tiles_and_coarse_levels_draw_in_the_vertex_format_beside_compact_tiles() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let steep = Steep(1);
    let cells = 4;
    let mut fallbacks = 0;
    let mut checked = 0;
    for index in [2usize, 3, 5, 7] {
        // Declaring level 9 the finest makes the steep field's tiles of level 9 and finer exceed 65.535 m of relief: fallbacks.
        let p = plan_of(&steep, cells, 12, index, TerrainView::Relief, true);
        let reference = vertex_render(&ctx, &steep, &p);
        for scattered in [false, true] {
            let mut r = compact_renderer(&ctx, &steep, &p, 9, scattered);
            r.render(&ctx, &p, false).unwrap();
            let (v, c, f) = r.format_counts();
            assert_eq!(v + c + f, r.resident_tiles());
            fallbacks += f;
            for n in &p.nodes {
                let expected = if n.tile.level() <= 2 {
                    ResidentFormat::Vertex
                } else if matches!(tile_data_field(&steep, &TangentWarp, n.tile, cells, R, 9), RenderTile::Vertex(_)) {
                    ResidentFormat::Fallback
                } else {
                    ResidentFormat::Compact
                };
                assert_eq!(r.format_of(n.tile), Some(expected), "{:?}", n.tile);
            }
            let wrong = differing(&reference, &r.read_color(&ctx), 12) as f64 / (SIZE.0 * SIZE.1) as f64;
            assert!(wrong <= 0.01, "steep view {index} scattered {scattered}: {wrong} of the pixels differ from the vertex path");
            checked += 1;
            // A fallback slot is the renderer's own: forgetting the tile gives it back.
            let (in_use_before, _, _) = r.fallback_slot_counts();
            assert_eq!(in_use_before as usize, f);
            for n in &p.nodes {
                r.forget(n.tile);
            }
            assert_eq!(r.fallback_slot_counts().0, 0);
        }
    }
    assert!(fallbacks > 0, "the fixture must exercise fallback tiles");
    assert_eq!(checked, 8);
}

// spec: STRM-006, REND-016
#[test]
fn the_compact_allocation_equals_the_pure_model() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let cells = 16;
    let p = plan_of(&field, cells, 26, 5, TerrainView::Level, false);
    let mut r = compact_renderer(&ctx, &field, &p, 26, false);
    r.render(&ctx, &p, false).unwrap();
    let (blocks, vertex_pool) = r.live_pools();
    let config = *r.pool_config();
    assert!(blocks >= 1);
    // The pools, records, index buffer and node records: the model counts the node records at their configured maximum, the
    // renderer holds the ring its largest plan needed.
    let model = config.allocated_bytes_with(blocks, vertex_pool);
    let held = r.pool_allocated_bytes();
    assert_eq!(model - config.node_record_bytes, held - r.node_buffer_bytes(), "pools, records and indices equal the model");
    assert!(held <= model);
    assert!(
        r.node_buffer_bytes() <= config.node_record_bytes,
        "the ring (globals, nodes, arguments, tile references) stays inside the node-record budget"
    );
    assert!(config.allocated_bytes() <= 2u64 << 30, "STRM-006: the share");
    assert_eq!(atlas_columns(8 * (cells + 1), cells), 8);
}

// spec: REND-016, CON-11
#[test]
fn production_tiles_of_32_cells_draw_the_vertex_images_including_the_ground_view() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let cells = 32;
    let mut worst = 0f64;
    // The scripted views include the 1 m ground camera far from the origin (CON-11): every shader value is tile-local.
    for index in [3usize, 6, 9] {
        for view in [TerrainView::Relief, TerrainView::Level] {
            let p = plan_of(&field, cells, 26, index, view, false);
            let reference = vertex_render(&ctx, &field, &p);
            let mut r = compact_renderer(&ctx, &field, &p, 26, false);
            r.render(&ctx, &p, false).unwrap();
            assert!(r.format_counts().1 > 0);
            worst = worst.max(differing(&reference, &r.read_color(&ctx), 12) as f64 / (SIZE.0 * SIZE.1) as f64);
        }
    }
    println!("cells 32: worst fraction of pixels over 12 levels {worst:.5}");
    assert!(worst <= 0.002, "{worst} of the pixels differ");
}

// spec: REND-016
#[test]
fn a_morph_forced_to_the_parent_grid_agrees_so_the_shaders_morph_target_is_the_meshs() {
    // The morph target only shows where the morph factor is above zero, a thin band of a normal view: force the factor to 1 for every
    // node, so each vertex is drawn at its parent-grid position and height, on the steep field where the parent grid is metres away
    // from the vertex, and read the height target through the morph-height view (the morphed minus the own height over a window).
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let steep = Steep(1);
    let cells = 4;
    let mut worst = 0f64;
    for index in [3usize, 5, 7] {
        for view in [TerrainView::Relief, TerrainView::MorphHeight] {
            let mut p = plan_of(&steep, cells, 12, index, view, true);
            if view == TerrainView::MorphHeight {
                p.height_range = [-60.0, 60.0];
            }
            for n in &mut p.nodes {
                n.morph_start_m = 0.0;
                n.morph_end_m = 1.0e-3;
            }
            let reference = vertex_render(&ctx, &steep, &p);
            let mut r = compact_renderer(&ctx, &steep, &p, 26, false);
            r.render(&ctx, &p, false).unwrap();
            let image = r.read_color(&ctx);
            if view == TerrainView::MorphHeight {
                // The view is not flat grey: the height target is doing something.
                let (lo, hi) = image.rgba.chunks(4).fold((255u8, 0u8), |(l, h), px| (l.min(px[0]), h.max(px[0])));
                assert!(hi - lo > 40, "view {index}: the morph-height window shows no variation ({lo}..{hi})");
            }
            worst = worst.max(differing(&reference, &image, 12) as f64 / (SIZE.0 * SIZE.1) as f64);
        }
    }
    println!("fully morphed: worst fraction of pixels over 12 levels {worst:.5}");
    assert!(worst <= 0.01, "{worst} of the pixels differ with every vertex morphed");
}

// spec: REND-016, STRM-006
#[test]
fn the_production_atlas_geometry_works_on_this_adapter() {
    use planet_frame::pool::ATLAS_SIDE;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let cells = 32;
    // One real 4096 x 4096 atlas: 124 x 124 slots, tiles in the last slots of the last rows.
    let pool = compact_format_pool_with((128 << 20) + 4096 * 4096 * 8, cells, ATLAS_SIDE, 1).unwrap();
    assert_eq!((pool.main_blocks, pool.block_slots), (1, 15_376));
    let p = plan_of(&field, cells, 26, 6, TerrainView::Relief, false);
    let reference = vertex_render(&ctx, &field, &p);
    let mut r = TerrainRenderer::with_compact_pools(&ctx, SIZE, cells, pool);
    let mut next = 15_375u32;
    for n in &p.nodes {
        let data = tile_data_field(&field, &TangentWarp, n.tile, cells, R, 26);
        if n.tile.level() <= 2 {
            r.upload_tile(&ctx, &data).unwrap();
        } else {
            r.upload_tile_at(&ctx, &data, SlotRef { pool: PoolKind::Main, index: next }).unwrap();
            next -= 1;
        }
    }
    r.render(&ctx, &p, false).unwrap();
    assert!(differing(&reference, &r.read_color(&ctx), 12) as f64 / (SIZE.0 * SIZE.1) as f64 <= 0.002);
}

// spec: REND-016
#[test]
fn a_tile_in_the_wrong_pool_is_refused_not_written() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let cells = 16;
    let mut r = TerrainRenderer::with_compact_pools(&ctx, SIZE, cells, small_pool(cells));
    let coarse = planet_core::TileId::new(planet_core::Face(1), 2, 1, 1).unwrap();
    let mesh = tile_mesh_field(&field, &TangentWarp, coarse, cells, R);
    let main = SlotRef { pool: PoolKind::Main, index: 3 };
    assert!(
        matches!(r.upload_at(&ctx, &mesh, main), Err(planet_render::ExecError::CompactUnsupported { .. })),
        "a level 0-2 mesh in an atlas slot"
    );
    let fine = planet_core::TileId::new(planet_core::Face(1), 6, 1, 1).unwrap();
    let c = planet_generators::compact::compact_tile_field(&field, &TangentWarp, coarse, cells, R);
    assert!(
        matches!(r.upload_tile_at(&ctx, &RenderTile::Compact(c), main), Err(planet_render::ExecError::CompactUnsupported { .. })),
        "a compact tile of level 2"
    );
    let c = planet_generators::compact::compact_tile_field(&field, &TangentWarp, fine, cells, R);
    let out = SlotRef { pool: PoolKind::Main, index: 40 * 64 };
    assert!(matches!(r.upload_tile_at(&ctx, &RenderTile::Compact(c.clone()), out), Err(planet_render::ExecError::BadSlot { .. })));
    let vertex = SlotRef { pool: PoolKind::Vertex, index: 1 };
    assert!(matches!(r.upload_tile_at(&ctx, &RenderTile::Compact(c), vertex), Err(planet_render::ExecError::CompactUnsupported { .. })));
    assert_eq!(r.resident_tiles(), 0);
    // A vertex renderer refuses compact tiles.
    let mut v = TerrainRenderer::new(&ctx, SIZE, cells);
    let c = planet_generators::compact::compact_tile_field(&field, &TangentWarp, fine, cells, R);
    assert!(matches!(v.upload_tile(&ctx, &RenderTile::Compact(c)), Err(planet_render::ExecError::CompactUnsupported { .. })));
}
