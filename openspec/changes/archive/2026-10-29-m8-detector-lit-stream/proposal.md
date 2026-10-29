## Why
Automated owner checks, TEST-070 (planner, 2026-10-29): the top remaining suspect for the owner's flicker is the lit path under streaming, and the plan's frustum margin on small turns (Tracked: "Tiles flash to a coarser level while moving"). TEST-069 holds the lit path over fully resident tiles; TEST-068 streaming on the debug views.

## What Changes
1. `streamperf::lit_stream_stills`: the lit coast view at 2 km drawn by the streaming loop (`Driver`, the rig of the streaming scenes with the lit path) with the water time, the exposure step and the sun held; settles, brings the exposure to its target, reads 6 frames of the held camera, turns the camera 2 degrees and back, settles, and reads one more frame. `Driver` gains a hold and a sun option (additive; the scenes' behaviour is unchanged).
2. Tier C test `gpu_lit_stream_flicker`: the 6 frames are identical and the frame after the turn equals the frame before it, with the content guards of TEST-069 (colours, light in the lit target, water nodes, tiles, exposure at target) and a turned view that differs from the held one.
3. TEST-070 in the testing spec.

## Impact
- Result on main: PASS. Settled in 88 ticks, exposure 15.85 EV, 217 tiles, 116 water nodes; the turn and back settled in 122 ticks and the frame equals the one before. The streaming loop and the lit path draw a still camera identically, and the 2 degree turn does not leave a different picture once the loop has settled. No renderer change.
- Not covered: tiles during the turn (the intermediate frames; TEST-067 covers the level choice while moving), the editor hover composite, wall-clock water. A flash that lasts frames during a turn is not seen by a settled-frame comparison.
- Code: `app/streamperf.rs`, `app/editperf.rs` (`StillFrame` is `Clone`), `app/tests/gpu_lit_stream_flicker.rs`, `app/tests/gpu_all.rs`.
