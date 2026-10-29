## Why
Automated owner checks, part 2 (planner, 2026-10-29): the owner's flicker suspects are on the lit path (atmosphere, water glint, exposure, overlay); TEST-068 covers the debug views only.

## What Changes
1. Tier C test `gpu_lit_still_flicker` and `litperf::lit_still_frames`: the coast view at 2 km on the lit path, 6 frames per case at one water time, exposure adapted (read back), in the cases plain, sun ahead and overlay on the tiles; exact comparison; failure names pixels, box and tiles and writes images.
2. TEST-069 in the testing spec; TEST-068's Detail points to it.

## Impact
- Result on main: PASS in all three cases (exposure 12.00 EV by snap, 2.88 EV reached in 10 one-second steps, 16.00 EV in 5; 116 water nodes, 217 tiles). The lit passes are stable for a still camera and a fixed water time; not covered: streaming with the lit path, the editor's hover composite, wall-clock water.
- Code: `app/litperf.rs`, `app/tests/gpu_lit_still_flicker.rs`, `app/tests/gpu_still_flicker.rs` (`changes` shared), `app/tests/gpu_all.rs`. Docs: Tracked item in `docs/status.md`.
