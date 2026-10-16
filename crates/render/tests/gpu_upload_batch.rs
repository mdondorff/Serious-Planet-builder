//! REND-018, REND-016, STRM-006, REND-013: a frame's compact tiles reach the pools in one queue call per run of adjacent slots instead of two per
//! slot, and the pools, the images and the bookkeeping are those of the per-slot path (the reference, `UploadMode::PerSlot`).
//! Counted on the adapter at hand (the software adapter in CI).

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::{tile_data_field, RenderTile};
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer, UploadMode};
use planet_streaming::{PoolConfig, PoolKind, SlotRef};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
/// Slots along one side of an atlas, and slots of a block (see `small_pool`).
const COLS: u32 = 8;
const BLOCK: u32 = 64;

/// Atlases of 8 x 8 slots; a roomy vertex pool.
fn small_pool() -> PoolConfig {
    let side = COLS * (CELLS + 1);
    let one = compact_format_pool_with((128 << 20) + 40 * u64::from(side) * u64::from(side) * 8, CELLS, side, 1).unwrap();
    PoolConfig { vertex_slots: 4096, fallback_slots: 3968, ..one }
}

fn renderer(ctx: &GpuContext, mode: UploadMode) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_compact_pools(ctx, SIZE, CELLS, small_pool());
    r.set_upload_mode(mode);
    r
}

fn main_slot(index: u32) -> SlotRef {
    SlotRef { pool: PoolKind::Main, index }
}

/// `n` distinct compact tiles of level 6 (11 x 11 and more of them fit a face).
fn tiles(n: usize) -> Vec<RenderTile> {
    let field = Reference(1);
    (0..n as u32)
        .map(|k| {
            let id = TileId::new(Face(0), 6, k % 40, k / 40).unwrap();
            let t = tile_data_field(&field, &TangentWarp, id, CELLS, R, 26);
            assert!(matches!(t, RenderTile::Compact(_)), "the fixture tiles are compact");
            t
        })
        .collect()
}

/// The calls a batch of compact tiles in `slots` needs, by the model of the batch: one per run of adjacent slots in one row of one
/// atlas for the texels, one per run of adjacent slot indices for the records.
fn model_calls(slots: &[u32]) -> u64 {
    let mut s: Vec<u32> = slots.to_vec();
    s.sort_unstable();
    s.dedup();
    let texel_runs =
        1 + s.windows(2).filter(|w| !(w[1] == w[0] + 1 && w[1] / BLOCK == w[0] / BLOCK && !(w[1] % BLOCK).is_multiple_of(COLS))).count();
    let record_runs = 1 + s.windows(2).filter(|w| w[1] != w[0] + 1).count();
    (texel_runs + record_runs) as u64
}

fn place(ctx: &GpuContext, mode: UploadMode, tiles: &[RenderTile], slots: &[u32]) -> TerrainRenderer {
    let mut r = renderer(ctx, mode);
    let batch: Vec<(&RenderTile, SlotRef)> = tiles.iter().zip(slots).map(|(t, s)| (t, main_slot(*s))).collect();
    r.upload_tiles_at(ctx, &batch).unwrap();
    r
}

// spec: REND-018, REND-016, STRM-006
#[test]
fn a_frames_compact_tiles_make_one_call_per_run_and_leave_the_pools_of_the_per_slot_path() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let contiguous = |n: u32, start: u32| (start..start + n).collect::<Vec<u32>>();
    let cases: Vec<(&str, Vec<u32>)> = vec![
        ("1 tile", contiguous(1, 0)),
        ("10 adjacent tiles", contiguous(10, 0)),
        ("118 adjacent tiles (the 1 MiB cap)", contiguous(118, 0)),
        ("10 tiles from the middle of a row", contiguous(10, 5)),
        ("10 tiles, every third slot", (0..10).map(|k| k * 3).collect()),
        ("10 tiles, shuffled", vec![9, 2, 7, 40, 41, 3, 130, 4, 5, 6]),
    ];
    for (name, slots) in cases {
        let t = tiles(slots.len());
        let reference = place(&ctx, UploadMode::PerSlot, &t, &slots);
        let batched = place(&ctx, UploadMode::Batched, &t, &slots);
        let (ref_calls, calls) = (reference.upload_counters(), batched.upload_counters());
        assert_eq!(ref_calls.write_calls, 2 * slots.len() as u64, "{name}: the reference writes a texture and a record per slot");
        assert_eq!(calls.write_calls, model_calls(&slots), "{name}: one call per run");
        assert!(calls.write_calls <= ref_calls.write_calls, "{name}");
        assert_eq!(calls.staging_bytes, ref_calls.staging_bytes, "{name}: the same bytes travel, in fewer calls");
        assert_eq!(batched.uploaded_bytes, reference.uploaded_bytes, "{name}");
        assert_eq!(batched.format_counts(), reference.format_counts(), "{name}");
        for (tile, slot) in t.iter().zip(&slots) {
            assert_eq!(batched.slot_of(tile.id()), Some(main_slot(*slot)), "{name}");
        }
        let (a_ref, rec_ref) = reference.read_compact_pools(&ctx);
        let (a_new, rec_new) = batched.read_compact_pools(&ctx);
        assert_eq!(a_ref.len(), a_new.len(), "{name}: the same atlases are allocated");
        for (b, (x, y)) in a_ref.iter().zip(&a_new).enumerate() {
            assert!(x == y, "{name}: atlas {b} differs between the per-slot and the batched path");
        }
        assert!(rec_ref == rec_new, "{name}: the slot records differ");
        println!("{name}: {} calls batched against {} per slot, {} bytes", calls.write_calls, ref_calls.write_calls, calls.staging_bytes);
    }
}

// spec: REND-018, REND-016
#[test]
fn a_slot_written_twice_in_one_batch_holds_the_last_tile_as_in_the_per_slot_path() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let t = tiles(3);
    let slots = [4u32, 5, 4];
    let reference = place(&ctx, UploadMode::PerSlot, &t, &slots);
    let batched = place(&ctx, UploadMode::Batched, &t, &slots);
    let (a_ref, rec_ref) = reference.read_compact_pools(&ctx);
    let (a_new, rec_new) = batched.read_compact_pools(&ctx);
    assert!(a_ref == a_new && rec_ref == rec_new, "the pools differ");
    assert_eq!(batched.slot_of(t[2].id()), Some(main_slot(4)));
    assert_eq!(batched.slot_of(t[0].id()), None, "the first tile of slot 4 was replaced");
}

// spec: REND-018, REND-016, STRM-006
#[test]
fn a_batch_draws_the_image_of_the_per_slot_path_for_the_standard_views() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let mut drawn = 0usize;
    for index in 0..10 {
        let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
        let camera = scripted_views(R, 4.0 / 3.0, 1)[index].1;
        let req = TerrainRequest { camera, size: SIZE, view: TerrainView::Relief, lod, face_mapping: "tangent-v1".into(), seed: 1 };
        let plan = plan_terrain(&req).unwrap();
        // Manager-style placement as in `gpu_compact`: levels 0-2 in the managed vertex pool, the rest spread over the atlases.
        let (mut main, mut vertex) = (0u32, 0u32);
        let placed: Vec<(RenderTile, SlotRef)> = plan
            .nodes
            .iter()
            .map(|n| {
                let data = tile_data_field(&field, &TangentWarp, n.tile, CELLS, R, 26);
                let slot = if n.tile.level() <= 2 {
                    vertex += 1;
                    SlotRef { pool: PoolKind::Vertex, index: vertex - 1 }
                } else {
                    main = (main + 1) % (40 * 64);
                    main_slot(main)
                };
                (data, slot)
            })
            .collect();
        let mut images = Vec::new();
        let mut calls = Vec::new();
        for mode in [UploadMode::PerSlot, UploadMode::Batched] {
            let mut r = renderer(&ctx, mode);
            let batch: Vec<(&RenderTile, SlotRef)> = placed.iter().map(|(t, s)| (t, *s)).collect();
            r.upload_tiles_at(&ctx, &batch).unwrap();
            r.render(&ctx, &plan, false).unwrap();
            images.push(r.read_color(&ctx));
            calls.push(r.upload_counters().write_calls);
        }
        assert!(images[0] == images[1], "view {index}: the batched path draws another image");
        assert!(calls[1] <= calls[0], "view {index}: {calls:?}");
        drawn += plan.nodes.len();
    }
    assert!(drawn > 100, "the views must exercise the path");
}

// spec: REND-018, REND-016
#[test]
fn a_failed_batch_writes_the_tiles_placed_before_the_error() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let t = tiles(3);
    let mut r = renderer(&ctx, UploadMode::Batched);
    // The third tile is placed outside the pool: BadSlot after two good ones.
    let bad = main_slot(40 * 64 + 5);
    let batch = [(&t[0], main_slot(0)), (&t[1], main_slot(1)), (&t[2], bad)];
    assert!(r.upload_tiles_at(&ctx, &batch).is_err());
    assert_eq!(r.upload_counters().write_calls, 2, "the two good tiles are one texture run and one record run");
    assert_eq!(r.slot_of(t[1].id()), Some(main_slot(1)));
    let reference = place(&ctx, UploadMode::PerSlot, &t[..2], &[0, 1]);
    let (a, rec) = r.read_compact_pools(&ctx);
    let (a_ref, rec_ref) = reference.read_compact_pools(&ctx);
    assert!(a == a_ref && rec == rec_ref);
}
