//! Tier C: the horizon shadows' plumbing (REND-038 part b). A slot written through `set_horizon` reads back as written; a lit frame with a
//! horizon set whose table holds no map (slot -1 everywhere), with the array allocated, equals the frame of a renderer that never heard of it.

use planet_core::cube::TangentWarp;
use planet_frame::horizonview::{HorizonFrame, HorizonNode, SlotUpdate, SLOT_BYTES};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_streaming::PoolConfig;
use planet_testkit::Image;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (160, 120);
const CELLS: u32 = 16;
const FINEST: u8 = 26;
const EV: f32 = 15.0;

fn plan(index: usize) -> TerrainPlan {
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let (_, camera) = scripted_views(R, aspect, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    let mut p =
        plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 })
            .unwrap();
    p.lit = Some(LitPlan::fixed(EV));
    p
}

fn compact_pool() -> PoolConfig {
    let block_slots = 48 * 48;
    compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap()
}

fn renderer(ctx: &GpuContext, p: &TerrainPlan) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, p.size, CELLS, compact_pool());
    r.enable_lit(ctx);
    let field = Reference(1);
    for n in &p.nodes {
        if r.slot_of(n.tile).is_none() {
            r.upload_tile(ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).unwrap();
        }
    }
    r
}

fn frame(r: &mut TerrainRenderer, ctx: &GpuContext, p: &TerrainPlan) -> Image {
    r.render(ctx, p, false).unwrap();
    let f = r.read_color(ctx);
    Image::new(f.width, f.height, f.rgba)
}

// spec: REND-038
#[test]
fn a_slot_written_through_set_horizon_reads_back_as_written() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3);
    let mut r = renderer(&ctx, &p);
    assert_eq!(r.horizon_bytes(), 0, "the array is a dummy before a capacity is set");
    let none = HorizonFrame { capacity: 3, nodes: Vec::new(), update: SlotUpdate::default() };
    let pattern = |seed: u32| -> Vec<u8> {
        (0..SLOT_BYTES).map(|i| ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(seed) >> 13) as u8).collect()
    };
    let (a, b) = (pattern(1), pattern(2));
    r.set_horizon(&ctx, &none, &[(0, a.clone()), (2, b.clone())]);
    assert_eq!(r.horizon_bytes(), 3 * SLOT_BYTES as u64);
    assert!(r.read_horizon_slot(&ctx, 0) == a, "slot 0 reads back as written");
    assert!(r.read_horizon_slot(&ctx, 2) == b, "slot 2 reads back as written");
    assert!(r.read_horizon_slot(&ctx, 1).iter().all(|&x| x == 0), "an unwritten slot is zero");
}

// spec: REND-038
#[test]
fn a_frame_with_no_map_in_the_table_is_the_frame_without_the_horizon() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [0usize, 3, 6] {
        let p = plan(index);
        let mut r = renderer(&ctx, &p);
        let plain = frame(&mut r, &ctx, &p);
        // The array is allocated and written, every node reads slot -1.
        let nodes = vec![HorizonNode { slot: -1, sun_azimuth: 0.0, sun_elevation: 0.0, fade: 0.0, rect: [0.0; 4] }; p.nodes.len()];
        let set = HorizonFrame { capacity: 2, nodes, update: SlotUpdate::default() };
        r.set_horizon(&ctx, &set, &[(0, vec![255u8; SLOT_BYTES])]);
        let with = frame(&mut r, &ctx, &p);
        assert!(plain.pixels().eq(with.pixels()), "view {index}: slot -1 everywhere leaves the frame unchanged");
        r.clear_horizon();
        let cleared = frame(&mut r, &ctx, &p);
        assert!(plain.pixels().eq(cleared.pixels()), "view {index}: a cleared horizon leaves the frame unchanged");
    }
}
