//! REND-013, REND-016, TEST-018: no one-time initialisation inside a measured frame. After construction and `warm_up` the vertex
//! pool and the first block exist, the ring is grown, both pipelines have drawn once, and a script whose first frames
//! fit those creates nothing in frames 0 and 1; the image is the cold renderer's. The guarantee is the counters; the wall-clock
//! probe only reports (CON-17, CON-20: time budgets are the reference machine's).

use planet_core::cube::TangentWarp;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::pool::vertex_slot_bytes;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{tile_data_field, vertex_tile_field};
use planet_generators::height::Reference;
use planet_generators::mesh::grid_indices;
use planet_render::{AdapterPolicy, DrawMode, FirstUse, GpuContext, InitCounts, TerrainRenderer, RING_SETS, WARM_NODES};
use planet_streaming::PoolConfig;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const FINEST: u8 = 26;

fn plan(index: usize) -> TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

/// Vertex-format pools with room for a whole plan in block 0.
fn vertex_pool() -> PoolConfig {
    let slot = vertex_slot_bytes(CELLS);
    PoolConfig {
        slot_bytes: slot,
        block_slots: 512,
        main_blocks: 4,
        vertex_slot_bytes: slot,
        vertex_slots: 8,
        index_bytes: grid_indices(CELLS).len() as u64 * 4,
        node_record_bytes: 4096 * 512,
        frames_in_flight: 1,
        fallback_slots: 0,
    }
}

/// Compact pools: blocks of 576 slots, a roomy vertex pool for levels 0-2 and fallbacks.
fn compact_pool() -> PoolConfig {
    let block_slots = 24 * 24;
    let one = compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    PoolConfig { vertex_slots: 256, fallback_slots: 192, ..one }
}

fn renderer(ctx: &GpuContext, compact: bool) -> TerrainRenderer {
    if compact {
        TerrainRenderer::with_compact_pools(ctx, SIZE, CELLS, compact_pool())
    } else {
        TerrainRenderer::with_pools(ctx, SIZE, CELLS, vertex_pool())
    }
}

fn upload(ctx: &GpuContext, r: &mut TerrainRenderer, p: &TerrainPlan, compact: bool) {
    let field = Reference(1);
    for n in &p.nodes {
        if compact {
            r.upload_tile(ctx, &tile_data_field(&field, &TangentWarp, n.tile, CELLS, R, FINEST)).unwrap();
        } else {
            r.upload(ctx, &vertex_tile_field(&field, &TangentWarp, n.tile, CELLS, R, FINEST)).unwrap();
        }
    }
}

// spec: REND-017, REND-013, REND-016, REND-015, TEST-018
#[test]
fn a_warmed_renderer_creates_nothing_in_its_first_frames() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(5);
    assert!(p.nodes.len() <= WARM_NODES);
    for compact in [false, true] {
        for mode in [DrawMode::PerDraw, DrawMode::Indirect] {
            let what = format!("compact {compact}, {mode:?}");
            let mut r = renderer(&ctx, compact);
            r.set_draw_mode(&ctx, mode);
            let built = r.init_counts();
            assert_eq!(built, InitCounts { pipelines: if compact { 2 } else { 1 }, pool_allocations: 0, ring_builds: 1 }, "{what}");
            assert_eq!(r.live_pools(), (0, false), "{what}: nothing allocated before the warm-up");
            r.warm_up(&ctx);
            let warm = r.init_counts();
            assert_eq!(warm.pipelines, built.pipelines, "{what}: warming creates no pipeline");
            assert_eq!(r.resident_tiles(), 0, "{what}: no tile became resident");
            // The vertex pool and one block: counted in the model (STRM-006), the rest of the share stays on demand.
            assert_eq!(r.live_pools(), (1, true), "{what}");
            let config = *r.pool_config();
            let modelled = config.allocated_bytes_with(1, true) - config.node_record_bytes + r.node_buffer_bytes();
            assert_eq!(r.pool_allocated_bytes(), modelled, "{what}: the real allocation equals the model with one block");
            assert!(r.pool_allocated_bytes() <= config.allocated_bytes(), "{what}: under the share");
            assert!(r.node_buffer_bytes() <= config.node_record_bytes, "{what}: the ring is inside its budget");
            // Frames 0 and 1: tiles upload into the warmed block and draw; nothing is created.
            upload(&ctx, &mut r, &p, compact);
            for frame in 0..3 {
                r.submit(&ctx, &p).unwrap();
                assert_eq!(r.init_counts(), warm, "{what}: frame {frame} created a pipeline, pool block or ring");
                assert_eq!(r.live_pools(), (1, true), "{what}: frame {frame} allocated a block");
            }
            // The counter sees real creations: a cold renderer allocates in the same frames.
            let mut cold = renderer(&ctx, compact);
            cold.set_draw_mode(&ctx, mode);
            upload(&ctx, &mut cold, &p, compact);
            cold.submit(&ctx, &p).unwrap();
            assert!(cold.init_counts().pool_allocations > 0, "{what}: a cold renderer allocates on first use");
        }
    }
}

// spec: REND-017, REND-013, REND-016
#[test]
fn warming_up_does_not_change_the_image() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(5);
    for compact in [false, true] {
        let mut cold = renderer(&ctx, compact);
        upload(&ctx, &mut cold, &p, compact);
        cold.render(&ctx, &p, false).unwrap();
        let mut warm = renderer(&ctx, compact);
        warm.warm_up(&ctx);
        upload(&ctx, &mut warm, &p, compact);
        warm.render(&ctx, &p, false).unwrap();
        assert!(cold.read_color(&ctx).rgba == warm.read_color(&ctx).rgba, "compact {compact}: the warm-up changed the image");
    }
}

// spec: REND-017, REND-013, TEST-018
#[test]
fn frame_zero_timing_is_reported_with_and_without_the_warm_up() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let p = plan(5);
    for compact in [true, false] {
        for warm in [false, true] {
            let t = std::time::Instant::now();
            let mut r = renderer(&ctx, compact);
            r.set_draw_mode(&ctx, DrawMode::Indirect);
            let built_ms = t.elapsed().as_secs_f64() * 1000.0;
            let t = std::time::Instant::now();
            if warm {
                r.warm_up(&ctx);
            }
            let warm_ms = t.elapsed().as_secs_f64() * 1000.0;
            let t = std::time::Instant::now();
            upload(&ctx, &mut r, &p, compact);
            println!(
                "compact {compact} warm {warm}: construction {built_ms:.2} ms, warm_up {warm_ms:.2} ms, upload of {} tiles {:.2} ms",
                p.nodes.len(),
                t.elapsed().as_secs_f64() * 1000.0
            );
            let mut ms: Vec<f64> = (0..11).map(|_| r.submit(&ctx, &p).unwrap().0.cpu_ms).collect();
            let first = ms[0];
            ms.remove(0);
            ms.sort_by(f64::total_cmp);
            let median = ms[ms.len() / 2];
            println!("compact {compact} warm {warm}: frame 0 record {first:.3} ms, median of the next 10 {median:.3} ms");
        }
    }
}

// spec: REND-017, STRM-006
#[test]
fn growth_beyond_the_warmed_block_stays_on_demand() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(5);
    let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, PoolConfig { block_slots: 64, main_blocks: 16, ..vertex_pool() });
    r.warm_up(&ctx);
    let warm = r.init_counts();
    assert_eq!(r.live_pools(), (1, true));
    upload(&ctx, &mut r, &p, false);
    let (blocks, _) = r.live_pools();
    assert!(blocks > 1, "a plan of {} nodes needs more than the warmed block", p.nodes.len());
    assert_eq!(r.init_counts().pool_allocations, warm.pool_allocations + (blocks - 1), "each further block is one counted allocation");
    let config = *r.pool_config();
    let modelled = config.allocated_bytes_with(blocks, true) - config.node_record_bytes + r.node_buffer_bytes();
    assert_eq!(r.pool_allocated_bytes(), modelled);
}

// spec: REND-017, REND-013, REND-016, STRM-016
#[test]
fn the_warm_up_frame_touches_everything_a_measured_frame_first_uses() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(5);
    for compact in [false, true] {
        for mode in [DrawMode::PerDraw, DrawMode::Indirect] {
            let what = format!("compact {compact}, {mode:?}");
            let mut r = renderer(&ctx, compact);
            let mode = r.set_draw_mode(&ctx, mode);
            assert_eq!(r.first_use(), FirstUse::default(), "{what}: nothing is touched before the warm-up");
            r.warm_up(&ctx);
            let warm = r.first_use();
            // Every set of the per-frame ring, every pipeline of the format, the timestamp pass where the adapter has one and the
            // draw mode in effect: the lazy first-use work of each happens in the waited warm-up submission.
            assert_eq!(warm.ring_sets, RING_SETS, "{what}: every ring set is used by the warm-up frame");
            assert_eq!(warm.pipelines, if compact { 2 } else { 1 }, "{what}: every pipeline of the format draws");
            assert_eq!(warm.timestamp_passes, u32::from(ctx.timestamps_supported()), "{what}: the timed pass runs where supported");
            assert_eq!(warm.draw_modes, 1, "{what}: the active draw mode ({mode:?}) and no other");
            assert_eq!(warm.waited_submissions, 1, "{what}: one submission, waited for before frame 0");
            assert_eq!(r.resident_tiles(), 0, "{what}: no tile became resident");
            // Frames 0..3, untimed and timed: nothing is touched for the first time.
            upload(&ctx, &mut r, &p, compact);
            for frame in 0..4 {
                if frame % 2 == 0 {
                    r.submit(&ctx, &p).unwrap();
                } else {
                    r.render(&ctx, &p, true).unwrap();
                }
                let now = r.first_use();
                assert_eq!(
                    FirstUse { waited_submissions: 0, ..now },
                    FirstUse { waited_submissions: 0, ..warm },
                    "{what}: frame {frame} first-used something"
                );
            }
            // The counters see real first uses: a cold renderer touches ring sets and pipelines in its first two frames.
            let mut cold = renderer(&ctx, compact);
            cold.set_draw_mode(&ctx, mode);
            upload(&ctx, &mut cold, &p, compact);
            cold.submit(&ctx, &p).unwrap();
            cold.submit(&ctx, &p).unwrap();
            assert_eq!(cold.first_use().ring_sets, RING_SETS, "{what}: a cold renderer uses the sets in its first frames");
            assert_eq!(cold.first_use().waited_submissions, 0, "{what}: and waited for nothing");
        }
    }
}

// spec: REND-017, VIEW-007
#[test]
fn the_lit_warm_up_touches_the_lit_and_atmosphere_passes_before_the_first_measured_lit_frame() {
    use planet_frame::look::file::LookParams;
    use planet_frame::viewer::{lit_warm_plan, viewer_lit_plan};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(5);
    for compact in [false, true] {
        let what = format!("compact {compact}");
        let mut r = renderer(&ctx, compact);
        r.warm_up(&ctx);
        // `enable_lit` is not part of `warm_up`: the debug warm-up leaves the lit path untouched.
        assert!(!r.lit_enabled() && r.first_use().lit_variants == 0, "{what}");
        let lit = viewer_lit_plan(&LookParams::default(), R, 0.016, true, true).unwrap();
        r.warm_up_lit(&ctx, &lit_warm_plan(p.camera, SIZE, CELLS, lit)).unwrap();
        let warm = r.first_use();
        assert!(r.lit_enabled(), "{what}");
        assert_eq!(warm.lit_variants, 2, "{what}: a lit frame and a lit frame with the atmosphere were recorded");
        assert_eq!(warm.waited_submissions, 1 + RING_SETS, "{what}: the debug warm-up's, and one waited frame per ring set");
        assert_eq!(r.retired_frames(&ctx), 0, "{what}: the lit warm-up frames are not frames of the loop");
        assert_eq!(r.resident_tiles(), 0, "{what}: no tile became resident");
        let objects = r.lit_objects();
        // The measured lit frames, with tiles, adaptive and with the atmosphere, create and first-use nothing more.
        upload(&ctx, &mut r, &p, compact);
        for frame in 0..4 {
            let mut plan = p.clone();
            plan.lit = Some(viewer_lit_plan(&LookParams::default(), R, 0.016, frame == 0, true).unwrap());
            if frame % 2 == 0 {
                r.submit(&ctx, &plan).unwrap();
            } else {
                r.render(&ctx, &plan, true).unwrap();
            }
            let now = r.first_use();
            assert_eq!(
                FirstUse { waited_submissions: 0, ..now },
                FirstUse { waited_submissions: 0, ..warm },
                "{what}: lit frame {frame} first-used something"
            );
            assert_eq!(r.lit_objects(), objects, "{what}: lit frame {frame} created a long-lived object");
        }
        let e = r.lit_path().unwrap().read_exposure(&ctx);
        assert!(e.ev100.is_finite() && e.target_ev100.is_finite(), "{what}: {e:?}");
    }
}
