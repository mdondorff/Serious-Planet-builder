## Why
Owner (2026-10-27): the viewer and every perf scene drive one frame-loop function, so a scene cannot skip a step; each enabled feature reports a counter and the scene fails when an enabled feature's counter is 0.

The per-frame chunk step exists in five copies, and the copies have drifted. Findings (read only, main at #794):
| Copy | Chunk prefetch | Horizon jobs | Stale hold | Landed / evicted to the managed loop | Plan-cache announcement of an item change |
|---|---|---|---|---|---|
| `window.rs` (the viewer) | track set | `HorizonRig::frame` (also uploads) | yes | yes | its own edit path |
| `streamperf::tick` (stream and envelope scenes) | track set (since #791) | `horizon_frame` when asked | no | yes | none |
| `editperf::Harness::step` (edit-latency) | `&[]`: none | `horizon_frame` | yes (`--no-stale-hold` option) | yes | `announce_item_change` |
| `stills.rs` (two places) | `&[]`: none | `horizon_frame` in one | no | no | none |
| `editresolve::settle` | `&[]`: none | none | no | no | none |
The prefetch was missing from all of them until #782 and is missing from three still. Window 8 measured the 300 m flight without any chunk prefetch and nobody could tell from the log.

## What Changes
1. **One step.** `chunkstage::frame_step(world, managed, tiles, camera, &FrameFeatures) -> StepReport` does, in the order of the viewer: the stale hold from the managed loop's pending work, `chunk_frame_with` with the track-set prefetch from the loop's travel estimator, `chunk_landed` / `chunk_evicted` on the managed loop, and `horizon_frame`. `FrameFeatures` is the knob set a scene may turn off on purpose (`prefetch`, `horizon`, `stale_hold`); each is on by default, and a scene that turns one off says so in its log. The viewer's upload of horizon maps and the fade clock stay in the viewer, called after the step.
2. **Counters.** `StepReport` adds to a `FeatureCounters` the scene keeps: chunks asked to prefetch (frames and keys), prefetch-class jobs started, horizon maps baked and landed, stale-hold frames, chunks landed and evicted, plan-cache announcements (the edit scenes' `announce_item_change` moves into the same module). The log of every scene carries the counters as one object.
3. **The check.** At the end of a scene, `FeatureCounters::check(&FrameFeatures, &Expect)` returns an error naming the feature when an enabled one counted 0 where the scene expects it to act: prefetch in any flight with travel, horizon bakes when chunks landed and horizon is on, plan-cache announcements in any scene that edits. The scene exits non-zero with the counter table (CON: failure output is actionable).
4. **Callers.** `window.rs`, `streamperf::tick`, `editperf::Harness::step` and `stills.rs` call `frame_step`; their copies go. `editresolve::settle` (a settling loop, not a frame) calls it with the prefetch off and says so.
5. **Tests.** Tier A with the stand-in chunk stage and `ManualSpawner`: (a) a frame loop that calls `frame_step` over a travelling camera counts prefetch, horizon bakes and announcements; (b) the same loop with a step removed (a test double that skips the prefetch) fails the check with the feature named; (c) a scene with a feature turned off passes and its log says `off`. Tier C: the existing smoke runs fail if a counter is 0.

## Questions
- Which counters are `Expect`ed per scene (the stream dives do not edit; the edit scene does not travel): a table in the spec, one row per scene.
- The viewer is a window: its counters go to the crash/stall note every 10 s and to `--smoke`'s exit status; the `--smoke` run of the viewer is the check there.

## Impact
- Spec: STRM-039/STRM-042 (the step), TEST-058 (new: the counters and the check). Code: `app/chunkstage.rs`, `app/window.rs`, `app/streamperf.rs`, `app/editperf.rs`, `app/stills.rs`, `app/editresolve.rs`. No golden.
- Risk: the viewer's frame order must not change (the upload budget and the fade clock follow the step); the first PR moves the step with the viewer's order and the scenes adopt it afterwards.
