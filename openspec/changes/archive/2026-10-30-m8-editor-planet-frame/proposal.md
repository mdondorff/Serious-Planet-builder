## Why
Owner test 2026-10-27 (`docs/milestones/M8/owner-test-2026-10-27/README.md`, group A item 2): editor mode at the full-planet view costs the CPU up to 1.3 s a frame, viewer mode is cheap; two stalls of 0.85 and 0.92 s sat in "camera, ground heights and the globe overview". Probes (one thread, real store): the editor's hover is a `pick_surface` ray march through the finest height of the field, 3 to 17 ms a pick at 20000 to 1000 km (worst 33 to 122 ms), 0.1 ms below 100 km; the same picks at the tile level whose vertex spacing is a pixel cost 0.0 to 0.1 ms with the same hits. The first height reads of a new area decode a bake block on the main thread: 122 ms and 29 ms, near a second when the camera meets a block no thread has decoded.

## What Changes
1. Planner ruling (2026-10-27): a pick is accurate to a pixel of the current view. The hover, the draft outline and the brush ring read `height_at_level` at `pick_level` (the tile level whose vertex spacing is about a pixel at the camera's height); a click still places at the exact surface. The ground height under the camera reads the same level.
2. `WorldTerrain::warm_heights(dirs)`: a thread of its own (`height-warm`) reads the heights at the camera's ground point and where it will be in 1, 2 and 4 s, so the bake blocks are decoded before the main thread meets them (CON-22). The window sends a batch every sixth frame; a full queue drops the batch.
3. `PLANET_STALL_MS` prints the mean and longest time of every frame phase every 10 s; the egui frame and the present are separate phases.

## Impact
- Spec: VIEW-059 (new). Code: `app/editwin.rs`, `app/window.rs`, `app/worldfield.rs`, `app/stall.rs`. Tests: tier A (`pick_level`), tier B (`oracle_edit_resolve`: a bake block decoded by the warm-up thread is read on the main thread with no decode of its own); probe `pick_cost_report` (ignored).
