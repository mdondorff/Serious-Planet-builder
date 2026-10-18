//! The scheduler's frame work through the managed loop follows what changed, not the size of R (STRM-019, STRM-005, TEST-018),
//! tier A, fakes only, counts only (CON-20); the streamer-level cases are in `crates/streaming/tests/scheduler_work.rs`.

#[path = "support/sched.rs"]
mod sched;

use planet_frame::upload::UploadBudget;
use planet_frame::{LodParams, TerrainRequest};
use sched::*;

// ---- cost through the managed loop ----

/// Frames at `req` until the view is the exact frame and nothing is queued, running or waiting; returns the sizes of D and R.
fn settle(rig: &mut LoopRig, req: &TerrainRequest) -> (usize, usize) {
    let mut d = Digest::new();
    for k in 0..600 {
        rig.step(req, 10_000, &mut d);
        let counts = rig.lp.manager().streamer().stats();
        let r = &rig.lp.wants().residency;
        if k > 3
            && counts.queued == 0
            && counts.in_flight == 0
            && rig.lp.pending().is_empty()
            && !r.is_empty()
            && r.iter().all(|(t, _)| rig.lp.resident().contains_key(t))
        {
            return (rig.lp.wants().draw.len(), r.len());
        }
    }
    panic!("did not settle");
}

fn work_of(rig: &mut LoopRig, req: &TerrainRequest) -> planet_streaming::SchedWork {
    let before = rig.lp.work();
    let mut d = Digest::new();
    rig.step(req, 10_000, &mut d);
    rig.lp.work().since(&before)
}

fn finer(tau: f64) -> LodParams {
    LodParams { tau_px: tau, ..lod() }
}

// spec: STRM-019, STRM-005, STRM-001, TEST-018
#[test]
fn a_settled_still_view_through_the_loop_does_no_scheduler_work_and_a_rotation_touches_no_tile_of_r_at_any_size_of_r() {
    let mut sizes = Vec::new();
    for tau in [8.0, 3.0] {
        let lp = finer(tau);
        let mut rig = loop_rig_for(lp.clone(), 1_000_000, true, 9_168, UploadBudget::default());
        let first = request_with(lp.clone(), here(), 0.0, -0.4);
        let (_, r_len) = settle(&mut rig, &first);
        sizes.push(r_len);
        // Still: the loop's wants do not change, the manager and the streamer have nothing to do.
        for _ in 0..30 {
            let w = work_of(&mut rig, &first);
            assert_eq!(w, planet_streaming::SchedWork::default(), "tau {tau}: R {r_len} tiles, a settled frame touches nothing");
        }
        // Turning: the plan changes with the view, R does not (the worker hands the same set), nothing is requested or cancelled.
        let mut turned_d = 0u64;
        let mut d_applied = rig.lp.wants().draw.len();
        for k in 0..12u32 {
            let req = request_with(lp.clone(), here(), f64::from(k) * 0.26, [-1.0, -0.4, 0.2][k as usize % 3]);
            let before = rig.lp.manager().streamer().stats();
            for _ in 0..2 {
                let d_before = rig.lp.wants().draw.len();
                let w = work_of(&mut rig, &req);
                // The list handed over this frame is the one the wants held before it, the layer held the one before that, and the loop
                // rebuilds the wants from the plan it received.
                let d_len = d_before.max(d_applied).max(rig.lp.wants().draw.len());
                d_applied = d_before;
                assert_eq!(w.layer_updates[1], 0, "tau {tau} yaw {k}: R is untouched by a turn");
                assert_eq!(w.layer_updates[2], 0, "no prefetch");
                // The draw set is compared with the last (a sequential pass over D, a few times), the tiles that differ are written.
                assert!(w.scanned <= 6 * d_len as u64 + 64, "tau {tau} yaw {k}: {w:?} with D of {d_len}");
                turned_d += w.updated;
            }
            let after = rig.lp.manager().streamer().stats();
            assert_eq!(
                (after.dispatched, after.cancelled),
                (before.dispatched, before.cancelled),
                "yaw {k}: a turn requests and cancels nothing"
            );
        }
        eprintln!("tau {tau}: R {r_len}, a turn wrote {turned_d} elements in 24 frames");
    }
    assert!(sizes[1] >= 3 * sizes[0], "the second scene has a much larger R: {sizes:?}");
}

// spec: STRM-019, STRM-005, TEST-018
#[test]
fn a_step_of_travel_through_the_loop_writes_the_tiles_that_changed_and_keys_the_draw_set_not_r() {
    let lp = finer(3.0);
    let mut rig = loop_rig_for(lp.clone(), 1_000_000, false, 0, UploadBudget::default());
    let start = request_with(lp.clone(), track(0.0, 200.0), 0.3, -0.4);
    let (d_len, r_len) = settle(&mut rig, &start);
    // A 3 m step on the ground: the plan and R are asked for, delivered, and the wants follow.
    let before = rig.lp.work();
    let mut d = Digest::new();
    let step = request_with(lp, track(3.0, 200.0), 0.3, -0.4);
    for _ in 0..4 {
        rig.step(&step, 0, &mut d);
    }
    let w = rig.lp.work().since(&before);
    eprintln!("D {d_len} R {r_len}: a 3 m step over 4 frames: {w:?}");
    let changed: u64 = w.layer_updates.iter().sum();
    assert!(changed * 20 < r_len as u64, "few tiles join or leave the classes: {w:?} against R of {r_len}");
    // The writes beyond them key the new requests of the draw set: each plan delivered while draw requests wait is keyed once, a
    // pass over D's tiles and the ancestors above them (a few thousand writes at the most, whatever R holds).
    assert!(w.updated <= changed * 8 + 8 * d_len as u64 + 64, "{w:?} with D of {d_len}");
}
