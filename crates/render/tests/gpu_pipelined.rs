//! STRM-016: the renderer can submit a frame without waiting for it, reports how many frames the GPU has finished, and waits
//! only when asked to (the loop's bound on frames in flight). The image of frames submitted without waiting equals the image of
//! frames rendered with the wait. WARP and hardware alike. Compact tiles (task 2.2 of `remove-vertex-render-path`).

use planet_core::cube::TangentWarp;
use planet_frame::pipeline::{FramesInFlight, MAX_FRAMES_IN_FLIGHT};
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;

fn plan(index: usize) -> planet_frame::TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

// spec: STRM-016
#[test]
fn frames_submitted_without_waiting_are_counted_bounded_and_draw_the_same_image() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let plans: Vec<_> = [3usize, 4, 5, 4, 3].iter().map(|&i| plan(i)).collect();
    let mut renderer = TerrainRenderer::with_frames_in_flight(&ctx, SIZE, CELLS, u64::from(MAX_FRAMES_IN_FLIGHT));
    let mut reference = TerrainRenderer::new(&ctx, SIZE, CELLS);
    for p in &plans {
        for n in &p.nodes {
            let m: CompactTile = compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R);
            renderer.upload_tile(&ctx, &m).unwrap();
            reference.upload_tile(&ctx, &m).unwrap();
        }
    }
    let mut fi = FramesInFlight::new(MAX_FRAMES_IN_FLIGHT);
    for (k, p) in plans.iter().enumerate() {
        fi.retire_to(renderer.retired_frames(&ctx));
        while fi.is_full() {
            fi.note_wait();
            fi.retire_to(renderer.wait_oldest(&ctx));
        }
        let (timing, stats) = renderer.submit(&ctx, p).unwrap();
        fi.submit();
        assert!(fi.in_flight() <= MAX_FRAMES_IN_FLIGHT, "frame {k}");
        assert_eq!(stats.draw_calls as usize, p.nodes.len());
        assert!(timing.gpu_ms.is_none() && timing.total_ms == timing.cpu_ms, "a submit has no GPU time and does not wait");
        // The image of this frame, read back after the loop's own bookkeeping, equals the waited render of the same plan.
        let image = renderer.read_color(&ctx);
        reference.render(&ctx, p, false).unwrap();
        assert_eq!(image, reference.read_color(&ctx), "frame {k}");
    }
    // Reading back waited for everything: all frames are finished and counted.
    let mut finished = renderer.retired_frames(&ctx);
    while finished < plans.len() as u64 {
        finished = renderer.wait_oldest(&ctx);
    }
    assert_eq!(finished, plans.len() as u64);
    fi.retire_to(finished);
    assert_eq!(fi.in_flight(), 0);
    assert!(fi.peak() <= MAX_FRAMES_IN_FLIGHT);
}
