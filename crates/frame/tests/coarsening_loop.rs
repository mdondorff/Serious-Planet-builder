//! No point of the planet that stays in view and nears the camera is drawn at a coarser level than in the frame before, through the managed loop with
//! the tiles streamed over frames and evicted under a tight pool (STRM-016, LOD-007, TEST-067). Tier A, fakes only: `cover.rs` holds the same rule over
//! a lagging plan and residency built by hand; this one runs the real scheduler, streamer and residency manager on manual spawners.

#[path = "support/sched.rs"]
mod sched;

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::upload::UploadBudget;
use sched::*;
use std::collections::BTreeSet;

const FRAMES: u32 = 220;

/// The level of the drawn node covering `dir`, none when nothing covers it.
fn drawn_level(nodes: &BTreeSet<TileId>, dir: Vec3) -> Option<(u8, TileId)> {
    (0..=22u8).find_map(|l| {
        let t = TileId::from_direction(&TangentWarp, dir, l);
        nodes.contains(&t).then_some((l, t))
    })
}

/// 2000 ground directions within 0.45 rad of `dir` (a Fibonacci disc).
fn probes(dir: Vec3) -> Vec<Vec3> {
    (0..2000)
        .map(|i| {
            let (u, a) = ((f64::from(i) + 0.5) / 2000.0, 2.399_963_229_728_653 * f64::from(i));
            let theta = 0.45 * libm::sqrt(u);
            let east = dir.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
            let north = dir.cross(east).normalized();
            (dir * libm::cos(theta) + (east * libm::cos(a) + north * libm::sin(a)) * libm::sin(theta)).normalized()
        })
        .collect()
}

/// A probe drawn coarser than in the frame before: `(frame, probe, level before, level now, tile now)`.
type Coarsened = (u32, usize, u8, u8, TileId);

/// What one approach found: the coarsenings, the tiles the renderer forgot, and the frames that drew the cover or a stale plan.
struct Run {
    found: Vec<Coarsened>,
    evictions: usize,
    not_exact_frames: usize,
}

/// One slow approach: `FRAMES` frames from 3000 km down to about 50 km (two per cent lower each frame), the camera turning 0.3 degrees a frame and looking
/// steeply down, `jobs` tile loads finishing per frame, a pool of `capacity` tiles. Returns the frames in which a probe that neared the camera was drawn
/// at a coarser level than in the frame before: `(frame, probe, level before, level now, tile now)`.
fn coarsened(jobs: usize, capacity: usize, prefetch: bool) -> Run {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let probes = probes(dir);
    let mut rig = loop_rig(capacity, prefetch, 9_168, UploadBudget::default());
    let mut prev: Vec<Option<(u8, f64)>> = vec![None; probes.len()];
    let mut found: Vec<Coarsened> = Vec::new();
    let mut evictions = 0;
    let mut not_exact_frames = 0;
    for k in 0..FRAMES {
        let alt = 3_000_000.0 * libm::pow(0.98, f64::from(k));
        let pos = PlanetFixed(dir * (R + alt));
        let req = request(pos, f64::from(k) * 0.3f64.to_radians(), -1.2);
        rig.clock.advance(FRAME_MS);
        let f = rig.lp.begin_frame(&req, &mut rig.sink).unwrap();
        not_exact_frames += usize::from(f.pipeline.draw.kind != planet_frame::pipeline::DrawKind::Exact);
        let nodes: BTreeSet<TileId> = f.pipeline.draw.plan.nodes.iter().map(|n| n.tile).collect();
        for (i, p) in probes.iter().enumerate() {
            let cur = drawn_level(&nodes, *p).map(|(l, t)| (l, t, (pos.0 - *p * R).length()));
            if let (Some((l0, d0)), Some((l1, t1, d1))) = (prev[i], cur) {
                if l1 < l0 && d1 < d0 - 1.0 {
                    found.push((k, i, l0, l1, t1));
                }
            }
            prev[i] = cur.map(|(l, _, d)| (l, d));
        }
        evictions = rig.sink.forgets.len();
        rig.plan_spawner.run(4);
        rig.tile_spawner.run(jobs);
    }
    Run { found, evictions, not_exact_frames }
}

fn report(found: &[Coarsened]) -> String {
    let frames: BTreeSet<u32> = found.iter().map(|f| f.0).collect();
    let first = found.first().map(|f| format!("frame {} probe {}: L{} -> L{} at {:?}", f.0, f.1, f.2, f.3, f.4)).unwrap_or_default();
    format!("{} probe-frames in {} frames; first: {first}", found.len(), frames.len())
}

// spec: STRM-016, LOD-007, TEST-067
#[test]
fn a_slow_approach_over_tiles_streamed_one_or_two_a_frame_never_draws_a_point_coarser_than_the_frame_before() {
    let Run { found, not_exact_frames: substituted, .. } = coarsened(2, 4_000, true);
    assert!(substituted > 20, "only {substituted} frames drew the cover or a stale plan instead of the exact plan: the script does not stream slowly enough to prove anything");
    assert!(found.is_empty(), "the drawn level got coarser while the camera neared: {}", report(&found));
}

// spec: STRM-016, LOD-007, TEST-067
#[test]
fn an_approach_under_a_tight_pool_that_evicts_never_draws_a_point_coarser_than_the_frame_before() {
    let Run { found, evictions, .. } = coarsened(64, 600, true);
    assert!(evictions > 0, "the script must force evictions, or the test proves nothing");
    assert!(found.is_empty(), "the drawn level got coarser while the camera neared ({evictions} tiles evicted): {}", report(&found));
}
