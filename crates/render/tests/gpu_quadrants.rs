//! STRM-016 (SPIKE-18): a node drawn over some quadrants of its tile covers exactly those quadrants of it and nothing else, on every
//! adapter. The level view is flat colour per node, so the pixels of the node are the pixels that are not the clear colour; seen from
//! straight above, the quadrant masks of one node then partition them.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::camera::Camera;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{TerrainDraw, TerrainPlan, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::{HeightField, Reference};
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use std::collections::BTreeSet;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (192, 144);
const CELLS: u32 = 32;

/// The screen position in pixels of the planet-fixed point `p` for `plan`'s camera and matrix (camera-relative, column-major, row 0 on
/// top), or none behind the camera.
fn project(plan: &TerrainPlan, p: Vec3) -> Option<(f64, f64)> {
    let rel = p - plan.camera.position.0;
    let m = plan.view_projection.map(f64::from);
    let clip = |row: usize| m[row] * rel.x + m[4 + row] * rel.y + m[8 + row] * rel.z + m[12 + row];
    let (x, y, w) = (clip(0), clip(1), clip(3));
    (w > 0.0).then(|| ((x / w * 0.5 + 0.5) * f64::from(SIZE.0), (1.0 - (y / w * 0.5 + 0.5)) * f64::from(SIZE.1)))
}

fn lit_pixels(ctx: &GpuContext, data: &planet_generators::compact::CompactTile, plan: &TerrainPlan) -> BTreeSet<(u32, u32)> {
    let block_slots = 8 * 8;
    let pool = compact_format_pool_with(
        (128 << 20) + 40 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, pool);
    r.upload_tile(ctx, data).unwrap();
    r.render(ctx, plan, false).unwrap();
    let image = r.read_color(ctx);
    let clear = plan.clear_color;
    image
        .rgba
        .chunks(4)
        .enumerate()
        .filter(|(_, px)| px[..3] != clear[..3])
        .map(|(i, _)| ((i as u32) % SIZE.0, (i as u32) / SIZE.0))
        .collect()
}

// spec: STRM-016, REND-016
#[test]
fn a_node_drawn_over_some_quadrants_covers_exactly_those_quadrants_of_its_tile() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    // A tile of level 5 seen from straight above, filling the picture: nothing of it hides another part of it.
    let at = Vec3::new(0.8, 0.45, 0.35).normalized();
    let tile = TileId::from_direction(&TangentWarp, at, 5);
    let centre = tile.sample_direction(&TangentWarp, CELLS, CELLS / 2, CELLS / 2);
    let data = compact_tile_field(&field, &TangentWarp, tile, CELLS, R);
    let origin = data.origin;
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let camera = Camera::looking_at_centre(PlanetFixed(centre * (R + 330_000.0)), Vec3::new(0.0, 0.0, 1.0), 0.9, aspect, 1_000.0);
    let rel = origin.0 - camera.position.0;
    let node = |quadrants: u8| TerrainDraw {
        tile,
        level: tile.level(),
        origin,
        camera_relative_origin: [rel.x as f32, rel.y as f32, rel.z as f32],
        morph: 0.0,
        morph_start_m: 1.0e30,
        morph_end_m: 1.0e30,
        color: [200, 60, 60, 255],
        mesh_key: data.key,
        preview: false,
        quadrants,
    };
    let plan = |quadrants: u8| TerrainPlan {
        size: SIZE,
        view: TerrainView::Level,
        camera,
        view_projection: camera.view_projection(),
        clear_color: [10, 10, 30, 255],
        height_range: [-9_000.0, 9_000.0],
        cells: CELLS,
        shore_ramp_per_m: 0.0,
        nodes: vec![node(quadrants)],
        lit: None,
        water: None,
    };
    let lit = |quadrants: u8| lit_pixels(&ctx, &data, &plan(quadrants));
    let whole = lit(0b1111);
    assert!(whole.len() > 2_000, "the tile fills a good part of the picture: {} px", whole.len());
    let single: Vec<BTreeSet<(u32, u32)>> = (0..4).map(|k| lit(1 << k)).collect();
    // The four quadrants partition the node's pixels.
    let union: BTreeSet<(u32, u32)> = single.iter().flatten().copied().collect();
    assert_eq!(union, whole, "the quadrants together are the whole node");
    for a in 0..4 {
        for b in a + 1..4 {
            assert!(single[a].is_disjoint(&single[b]), "quadrants {a} and {b} do not overlap");
        }
        assert!(single[a].len() * 8 > whole.len(), "quadrant {a} draws about a quarter: {} of {}", single[a].len(), whole.len());
    }
    // Which quadrant is which: child k = 2 * (x half) + (y half); the centre of its quadrant is drawn by its own mask and by no other.
    for (k, quadrant) in single.iter().enumerate() {
        let (dx, dy) = ((k / 2) as u32, (k % 2) as u32);
        let q = tile.sample_direction(&TangentWarp, CELLS, CELLS / 4 + CELLS / 2 * dx, CELLS / 4 + CELLS / 2 * dy);
        let (x, y) = project(&plan(0b1111), q * (R + field.height(q))).expect("in front of the camera");
        let px = (x as u32, y as u32);
        assert!(quadrant.contains(&px), "the centre of child {k} at {px:?} is drawn by its mask");
        for (other, set) in single.iter().enumerate() {
            assert!(other == k || !set.contains(&px), "and not by the mask of child {other}");
        }
    }
    // Two quadrants are the union of their sets.
    assert_eq!(lit(0b0101), single[0].union(&single[2]).copied().collect::<BTreeSet<_>>());
}
