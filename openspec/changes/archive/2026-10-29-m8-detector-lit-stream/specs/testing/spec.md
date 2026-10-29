## ADDED Requirements

### Requirement: TEST-070 The lit path over streamed tiles draws the same pixels for a still camera and after a small turn and back
On the software adapter, the coast view at 2 km of the lit scenes drawn by the streaming loop (tiles arriving over frames) SHALL give, once the loop is exact and idle and the exposure is at its target at a held water time, 6 consecutive frames that are pixel-identical (`Tolerance::Exact`). After the camera turns 2 degrees about its up axis and back, with the loop settled and the exposure at its target again, the frame SHALL equal the frame before the turn. A change SHALL fail the test naming the changed pixels, their box and the drawn tiles that differ, and SHALL write the frames and the diff image to `target/review/still-flicker/`.

Verify: C
Status: active
Source: owner live test 2026-10-19/22 (tiles flash while moving, orbit flicker), opus-planner (2026-10-29), TEST-067, TEST-068, TEST-069

Detail: The rig is the streaming scenes' with the lit path (`streamperf::Driver`): the in-memory bake with a sea, the tiles loaded by the streamer, the water plan on each frame. The exposure is read back and steps one second a tick until it is at its target (60 steps at most); the water time is held. The look's own sun is below the horizon at this coast (night, a black picture), so the run sets a sun from the side at 35 degrees. The run proves it can falsify: plan nodes that carry water, more than 4 drawn tiles, more than 16 colours, light in more than half of the lit target, and a turned view that differs from the held one; the first frame is kept in `target/review/still-flicker/`. The tiles listed on a change include a tile that changed level or was dropped. The tile format is the compact one; the adapter is WARP on Windows and lavapipe on Linux. Not covered: the editor's hover composite and wall-clock water.

#### Scenario: A still camera over streamed tiles
- **WHEN** the loop is settled and the exposure is at its target
- **THEN** 6 consecutive frames have zero differing pixels

#### Scenario: A small turn and back
- **WHEN** the camera turns 2 degrees and back and the loop settles again
- **THEN** the frame has zero differing pixels from the frame before the turn
