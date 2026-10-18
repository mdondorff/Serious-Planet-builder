//! The managed loop at render floor 3 (LOD-011, STRM-001, STRM-006, STRM-007, STRM-016), tier A, fakes only: the 384 roots are
//! written before frame 0 beyond the upload cap (the load of a body's roots is not a steady frame), the cap governs every later
//! frame, no frame is blank while the roots are resident, and a dive and a teleport never evict or reload a root.

#[path = "support/sched.rs"]
mod sched;

use planet_frame::upload::{UploadBudget, DEFAULT_TILE_BYTES_PER_FRAME};
use planet_frame::{floor_tiles, LodParams};
use sched::*;

const TILE_BYTES: u64 = 9_168;

fn lod3() -> LodParams {
    LodParams { min_level: 3, ..lod() }
}

// spec: STRM-001, STRM-006, LOD-011
#[test]
fn the_384_roots_are_written_before_frame_zero_beyond_the_cap_and_the_first_steady_frame_obeys_the_cap() {
    let lod = lod3();
    let roots = floor_tiles(&lod);
    assert_eq!(roots.len(), 384);
    assert!(
        roots.len() as u64 * TILE_BYTES > DEFAULT_TILE_BYTES_PER_FRAME,
        "the roots are more than the default cap (3.4 MiB against 1 MiB)"
    );
    let mut rig = loop_rig_for(lod.clone(), 100_000, false, TILE_BYTES, UploadBudget::default());
    let req = request_with(lod, here(), 0.3, -0.4);
    let before = rig.sink.order.len();
    let f = rig.lp.begin_frame(&req, &mut rig.sink).unwrap();
    let written = &rig.sink.order[before..];
    assert!(roots.iter().all(|t| written.contains(t)), "all 384 roots were written in frame 0");
    assert_eq!(written.len(), 384, "nothing else is resident yet");
    assert_eq!(f.counts.upload_bytes, 384 * TILE_BYTES, "the bytes are counted, and exceed the cap");
    assert_eq!(f.counts.deferred, 0, "no root waits");
    assert_eq!(f.pipeline.draw.blank, 0, "a floor tile is resident for every node of frame 0");
    // Every later frame obeys the cap (or the one-tile rule): the exemption was the load only.
    rig.tile_spawner.run(10_000);
    rig.plan_spawner.run(10);
    let mut steady_peak = 0u64;
    for k in 0..40 {
        rig.clock.advance(FRAME_MS);
        let before = rig.sink.order.len();
        let f = rig.lp.begin_frame(&req, &mut rig.sink).unwrap();
        let tiles = rig.sink.order.len() - before;
        assert!(
            f.counts.upload_bytes <= DEFAULT_TILE_BYTES_PER_FRAME || tiles == 1,
            "frame {k}: {} bytes in {tiles} tiles",
            f.counts.upload_bytes
        );
        assert!(rig.sink.order[before..].iter().all(|t| t.level() > 3), "frame {k}: a root was written again");
        steady_peak = steady_peak.max(f.counts.upload_bytes);
        rig.tile_spawner.run(64);
        rig.plan_spawner.run(2);
    }
    assert!(steady_peak > 0, "the script must upload something after the roots");
}

// spec: STRM-001, STRM-007, STRM-016, LOD-011
#[test]
fn a_dive_and_a_teleport_never_evict_or_reload_a_root_and_no_frame_is_blank() {
    let lod = lod3();
    let roots = floor_tiles(&lod);
    // A pool just above R of the far views, so the dive and the teleport must evict, yet never a root.
    let mut rig = loop_rig_for(lod.clone(), 1_500, true, TILE_BYTES, UploadBudget::default());
    let mut evicted_non_roots = 0usize;
    let mut frames = 0usize;
    let mut run = |rig: &mut LoopRig, pos, yaw: f64| {
        rig.clock.advance(FRAME_MS);
        let f = rig.lp.begin_frame(&request_with(lod.clone(), pos, yaw, -0.4), &mut rig.sink).unwrap();
        assert_eq!(f.pipeline.draw.blank, 0, "frame {frames}: blank while the roots are resident");
        for t in &rig.sink.forgets {
            assert!(t.level() > 3, "frame {frames}: the renderer forgot a root: {t:?}");
        }
        evicted_non_roots = rig.sink.forgets.len();
        let st = rig.lp.manager().streamer().stats();
        assert!(st.main_in_use <= 1_500, "frame {frames}: {} slots in use", st.main_in_use);
        assert!(roots.iter().all(|&t| rig.lp.manager().streamer().is_resident(t)), "frame {frames}: a root left the streamer");
        rig.tile_spawner.run(256);
        rig.plan_spawner.run(4);
        frames += 1;
    };
    // The descent from 20,000 km to 200 m, the ground segment, the teleport to the other side of the planet, and a hold.
    for k in 0..45 {
        let h = 20_000_000.0 * libm::pow(200.0 / 20_000_000.0, f64::from(k) / 45.0);
        run(&mut rig, track(0.0, h), 0.3);
    }
    for k in 0..30 {
        run(&mut rig, track(f64::from(k) * 20.0, 200.0), 0.3);
    }
    for k in 0..40 {
        run(&mut rig, there(), 1.0 + f64::from(k % 5) * 0.2);
    }
    // No root is requested, loaded or uploaded again after start-up: the dispatch log holds tiles below the floor tiles only, and
    // the sink received each root exactly once (at start-up).
    let dispatched = &rig.lp.manager().streamer().dispatch_log;
    assert!(dispatched.iter().all(|t| t.level() > 3), "a root was dispatched again");
    for r in &roots {
        assert_eq!(rig.sink.order.iter().filter(|t| *t == r).count(), 1, "{r:?} was written more than once");
    }
    assert!(evicted_non_roots > 0, "the script must force evictions, or the test proves nothing");
}
