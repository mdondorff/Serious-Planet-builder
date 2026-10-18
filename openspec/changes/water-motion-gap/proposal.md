## Why
- The owner saw dark patches on the sea near the camera in the viewer (free camera, 82 m over a coast, RTX, no pre-fill): the water surface is missing there and the seabed shows, only while the camera moves or rotates, gone when still.
- The same patches reproduce on main 15c47d7 (before #304): not a result of the vertex removal.
- The headless probe (in-memory 1,024-cell bake, the viewer's loop, cover and stale frames) shows no missing water near the camera; the patches it does show (0.1 to 0.8 % of sea pixels at 2 to 3 km, 1 to 3 m deep) come from coarse cover tiles and are not a defect. The frame of the owner's patch cannot be rebuilt from a screenshot.
- Hypothesis 1: the water pass draws a vertex only where `W - h_morph > 0` (ATMO-004); at morphing vertices (adjacent vertices alternate between morphed and not) the per-vertex sign flips and whole triangles disappear, which matches the regular sawtooth edge of the screenshot.
- Hypothesis 2: the water node list of a stale or cover frame misses nodes (horizon, height or exact-minimum cut, or a tile with no water answer).

## What changes
- F9 in the viewer writes a repro bundle of the frame drawn (TEST-008, format version 3): the request, the plan as drawn with its stand-in nodes, its lit and water parts, the world, how the frame was decided.
- Replay rebuilds the world from the bundle, regenerates the tiles of the drawn nodes and draws the recorded lit frame with its water (PNG), reporting the drawn nodes beside the request's own plan.
- With that bundle from the owner's frame: a failing test, the root cause, the fix.

## Impact
- Specs: TEST-008 (frame bundle), VIEW-003 (F9). The water requirement (ATMO-004 or ATMO-020) changes in this change once the cause is known.
- Code: `planet_frame::TerrainBundle` (optional frame section), `viewer` (command), `app` window (capture) and replay.
- No tile format, key, mesh version or golden changes from the capture.
