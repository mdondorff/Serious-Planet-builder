# SPIKE-25 results: tiles flash to a coarser level while the camera moves (lod-flashing-moving)

Diagnosis only (planner brief P1-4, 2026-10-20). Spike code: branch `spike/lod-flashing-moving` (never merged; `flash-detector.patch` here is its diff against main at 25057086). Machine: Rasierklinge, RTX 3080 Ti Laptop, Vulkan, release build, other sessions running.

## Answer in one paragraph
**Not reproduced; no root cause found.** A per-frame detector (below) over 14 scripted moving-camera flights on the real renderer, and over 2,085 frames of the editor window's own frame loop, finds no frame in which the drawn level over a point gets coarser while the camera gets nearer to it, except 8 one-level steps at 14 to 16 km (3 frames in 8,000, one generation worker, a 30 degrees per second turn, 150 stamps). The tracked SPIKE-18 case (a slow approach going one level coarser for a few frames) is fixed: a descent from 300 km to 3 km with a turn and one generation worker gives 0 violations where the SPIKE-18 recipe gave 19 to 223 before `cover-quadrants`. The owner's video shows something that the detector cannot see: the grid of the F3 tiles view changes brightness in bursts (below). Plan refusals (P1-1) and the coarse LOD (F4, SPIKE-19) take no part in any run (0 refused plans, F4 never on). The next step is an F9 capture of a flash (the camera path of the last minute, VIEW-024) replayed through `test-render --repro --replay-path`, which already reports "level drops".

## The detector (spike branch `crates/app/src/coarsen.rs`)
1. 6,000 probe points in a corridor around the camera path (12,000 in the window run), the level of the drawn node over each probe in every frame. A node drawn over some quadrants only (a stand-in, STRM-016) counts for a probe only if its mask holds the probe's quadrant (the first version of the detector did not do this and reported 5,734 of 6,000 probes at L3 in the first moving frame: a false positive of the masks, not a flash).
2. A violation: a probe's level drops between two frames while the camera is nearer to it by more than 0.5 m. Also logged per frame: draw kind, node count, mean level, nodes lying under a drawn ancestor's quadrants (an overlap), refused plans.
3. Scripted flights through the streaming perf harness (`test-render --perf-smoke --scene look-2km --lit --allow-memory-bake --global-res 256 --adapter hardware`, `COARSEN_SETTLE=1` to settle the start view completely first): `FLY_SPEED`, `FLY_ALT_M`, `FLY_ALT_END_M`, `FLY_YAW`, `FLY_PITCH`, `FLY_SECONDS`, `STAMPS=N` (hills and plateaus of 2 to 40 km reach spread over 1,500 km around the site), `--gen-workers 1` to starve the streamer without load.
4. The window run: `planet editor --scene terrain --world W --global-res 256` with the detector in the frame loop (vsync, real frame times, the egui layer, the scripted dive of the viewer); 80 s, 2,085 frames.

## Evidence
| flight | frames (Exact / Stale / Cover) | probe pairs | violations |
|---|---|---|---|
| 500 m, 300 m/s straight | 493 / 176 / 1,201 | 9.8 M | 1 (L16 to L15 at 2.2 km, 3 m nearer: a split boundary) |
| 500 m, 100 m/s, turn 30 deg/s | 451 / 120 / 1,502 | 3.8 M | 0 |
| 20 m, 50 m/s, pitch 5 | 444 / 112 / 1,501 | 10.3 M | 0 |
| 2 km, 600 m/s | 370 / 112 / 1,501 | 10.2 M | 0 |
| 300 km to 3 km, 200 m/s, pitch 40 | 147 / 1,091 / 449 | 8.0 M | 0 |
| the same four with 150 stamps | similar | 4.1 to 11.3 M | 0 |
| 800 km to 100 km at 30 km/s, 100 stamps | 297 / 750 / 976 | 9.8 M | 0 |
| 1, 5, 25 km, 300 m/s, 150 stamps | similar | 10.6 to 14.2 M | 0 |
| 500 m, 300 m/s, turn 30 deg/s, 150 stamps, **1 generation worker** | 3,727 / 1,488 / 1,843 | 13.5 M | **8 in 3 frames** (Cover to Cover, L13 to L12 and L14 to L13 at 4 to 16 km, 3 to 4 m nearer) |
| 300 km to 3 km, 200 m/s, pitch 40, turn 30 deg/s, 150 stamps, 1 generation worker | 990 / 639 / 2,162 | 8.8 M | 0 |
| editor window, scripted dive, 80 s | 0 / 820 / 1,265 | 12,000 probes | 0; 0 overlaps; 0 refused plans |
- The first moving frame after a settled start (Exact to Cover) shows no coarsening once the masks are read: the cover draws what the plan drew (the nodes are 1,776 against 1,783).
- Cover frames are the usual moving frame (a plan of another camera serves only within `STALE_MOVE_FRACTION` of the finest split distance and `STALE_ROTATION_RAD`): at 300 m/s every frame after the first 13 is a Cover. The mean level of Stale and Cover frames is the same (6.20 to 6.29 over 100 frames of one descent), so the pipeline does not alternate between a finer and a coarser draw.
- No plan was refused in any run (`PlanPipeline::refused_plans()` 0 in every csv), including 1 to 25 km flights over 150 stamps (SPIKE-22: the real eroded field gives per-tile bounds, the stamps add local bounds only).
- Two flights at 50 m and 2 km with one generation worker did not finish settling their start view in the 560 s the run was given; no data from them.

## What the owner's video shows (`0ea4703d-2026-10-20-bug-tile-lod-flashing-on-movement.mp4`, 36.9 s, 30 fps, 1276 x 744)
- 0 to 11 s: the F3 tiles overlay (grid cells coloured by level: white, pink, yellow) during a descent from 760 km towards three edits. Frame by frame, cells subdivide as the camera descends (coarse to fine in one step, e.g. frames 64 to 65) and stay; no coarse-fine-coarse step of the cell size was seen in the frames inspected (about 50 of the 330, around the brightness bursts and the steps of the pixel count).
- What does change in bursts is the line brightness: counting bright line pixels per frame (crop of the 3D view, red > 200 and green > 120), 16,000 to 17,900 for most frames and 20,400 to 22,000 for runs of 4 to 6 frames every 15 to 17 frames (frames 64 to 69, 80 to 84, 98 to 102, 118 to 121, 133 to 138, 147 to 151, 162 to 167), then 10,100 to 10,600 for 40 frames. In the burst frames the lines are brighter; in the frames between they are dimmer and pinker, with the same cell sizes. The title shows "EV 15.2 to 16.0 (adapted)": the adaptive exposure is moving, which dims and brightens a thin overlay line. This is a hypothesis; it was not checked (no capture of the frames' plans).
- 11 to 37 s: free-camera flight over the sea down to the waves; no tile level is visible there (no overlay); the frames are continuous apart from the camera's own moves.
- Second video (`67216f3b-...-edit-latency-tile-refresh.mp4`, 9.2 s): a crater of 2.32 km radius edited at 3.3 km altitude (depth 292 m to 39 m to 344 m); the terrain follows the edit within a few frames; thin white streaks (one or two straight lines about 100 px long) appear in 3 of 16 sampled frames across the crater; the title shows frames of 24 fps (cpu 33 to 41 ms) while the camera moves after an edit. Not analysed further (no plan data).

## Why the detector can still miss the owner's flash
1. It reads the plan's node levels. A flash that is a shader effect (the quadrant cut, the shore ramp of SPIKE-16, the overlay) does not change a level.
2. The world is a 256-cell in-memory bake with 150 stamps; the owner's is the editor's world (bake 6/6) with his edits and, at times, three sessions building beside it. SPIKE-18's failure needed that load; one generation worker is a weaker form of it.
3. The flights are scripted (no free camera); the window run flies the viewer's dive.

## Relation to the other open items
- **Coarse LOD (F4, SPIKE-19)**: `coarse()` keeps the plan's start plus 1 %; F4 was off in every run of mine; whether the owner had it on in the video is not known (the title bar does not say).
- **Plan refusals near stamps (P1-1)**: 0 refusals, so no frame was drawn from a stale plan because of one. Since #483 a refusal keeps the newest plan (Stale) or the cover; that path is covered by `plan-refusal-degrades`, and a run that refuses is invalid (#485).
- **SPIKE-18's fix**: holds under the one-worker starvation, with a turn.

## Next steps (the planner decides)
1. Ask the owner for an F9 right after a flash (the last minute of camera poses and the bundle); `cargo xtask repro` / `test-render --repro B --replay-path P` replays it frame by frame and lists "level drops" (`pathreplay::level_drops`); the detector would then run on his exact poses and world.
2. If the capture shows no level drop, compare two frames of the burst with F9 PNGs (VIEW-037) with and without the overlay and with the exposure held; the overlay line brightness is the lead.
3. Keep `flash-detector.patch` as the starting point for a tier-C test (a scripted flight with a per-frame check that no probe's level drops while it gets nearer): it is deterministic on the plan side, 0 violations today, and would have caught SPIKE-18 (19 to 223 under load).

## Files
Branch `spike/lod-flashing-moving`: `crates/app/src/coarsen.rs` (detector, flights, stamps, window hook), `crates/app/src/streamperf.rs` and `window.rs` (hooks), `crates/app/examples/spike25_def.rs` (a world file). Logs and csv per frame: `target/sp25/` of the spike worktree (not kept).
