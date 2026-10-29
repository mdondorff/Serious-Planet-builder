## Why
Automated owner checks, part 1b (planner, 2026-10-29): the orbit and dark-pixel flickers (owner, 2026-10-22) were not reproduced; a still camera must not change the picture once streaming has settled.

## What Changes
1. Tier C test `gpu_still_flicker` on the software adapter: after the edit-latency scene's settle, 6 frames of the same request in the level, relief and normals views must be pixel-identical; a failure writes both frames and the diff to `target/review/still-flicker/` and names the pixels, their box and the differing tiles.
2. `editperf::still_frames` (the settle and the read-back), TEST-068 in the testing spec.

## Impact
- Result on main: PASS in all three views (the streaming, LOD and shading of the debug views are stable for a still camera). The lit path (atmosphere, water glint, exposure, hover overlay) is not covered; a follow-up.
- Code: `app/editperf.rs`, `app/tests/gpu_still_flicker.rs`, `app/tests/gpu_all.rs`. Docs: Tracked item in `docs/status.md`.
