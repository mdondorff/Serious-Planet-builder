Opened: 2026-11-01
Question: Does `frame::pick::pick_surface` across face seams cause the editor's 90 ms of CPU per frame at cube edges and corners (Tracked item "Editor-mode CPU frame cost", owner 2026-10-21, marker "TheEdgeHill" at 35.2538 N 135.0217 E)?
Why it matters: the item has been open since M7 (under 10 fps near the marker in editor mode, under 2 ms with the editor off).
Time box: 2 h (Sonnet)
Success criteria: the cost reproduced headless or not; the frame's CPU split by phase; the pick hypothesis confirmed or refuted; one line for a fix PR.
Fallback if it fails: a CPU-only bench of the pick path with the same inputs.
Code location: `crates/app/examples/spike87_pick.rs` on branch `spike/editor-edge-cost` (never merged); copy in `proto/`.
Closed: 2026-11-01

## Headline
- **The pick is not the cause on today's code, and the 90 ms does not reproduce.** At the marker the hover pick costs 0.06-0.18 ms (mean of 45 picks, warm), the same as at a face centre (0.05-0.33 ms) and at a cube edge (0.06-0.29 ms). The editor's whole update phase in an editor smoke at the marker averages 0.17 ms (longest 2.1 ms) against 0.11 ms (1.3 ms) at a face site.
- **The marker is a cube corner**: its direction is (-0.5776, 0.5772, 0.5772), the corner (-1, 1, 1)/sqrt 3 within 0.03 degrees.
- **Only a cold pick is slow, anywhere**: the first pick that reads an undecoded bake block takes 17-43 ms (corner 40 ms, edge 43 ms, face centre 19 ms, face 21 ms); warm, the same rays take under 1.7 ms. Since 2026-10-27/28 (#831 VIEW-059 read-ahead, #858 async hover pick) that decode happens off the main thread. The owner's report (2026-10-21) predates both, when the pick ran on the main thread every frame.

## Method
- Pick bench (`spike87_pick`): the real store `global16384` (read only), the window's limits (`PickLimits::new(R, max(9,000 m, max |h|))`), cameras 100 m, 1 km, 10 km and 100 km above the ground at five sites (the marker's corner, two edge points at 0 N 45 E and 20 N 45 E, the face centre 0 N 0 E, and 10 N 10 E), heading north, pitch 0.3 and 1.2 rad, a 1 rad vertical field of view at 2560 x 1440; 45 cursors on a 9 x 5 grid; each set picked twice (cold, then warm). Per set: mean and worst ms, mean height calls, the share of time inside `field.height`, and microseconds per call. One thread, BelowNormal.
- Editor smoke: `planet editor --smoke --store global16384 --site LAT,LON` on the software adapter (Dx12 WARP) with `PLANET_STALL_MS=60000`, so the stall probe prints the mean and longest of every frame phase; at the marker with `--edit` and without it, and at 10 N 10 E with `--edit`. The smoke script runs 30 frames in the editor mode (place, drag, type, delete, undo, redo).
- Store unchanged: all 15 files under `global16384` have the same size and mtime before and after (`raw/store-files-*.txt`). No perf run.

## Results
### Pick bench, real store (`raw/pick-bench-store.txt`)
Mean / worst ms per pick over 45 cursors, warm, pitch 0.3 (oblique; pitch 1.2 is cheaper everywhere: 0.05-0.12 ms).
| Site | 100 m | 1 km | 10 km | 100 km | Cold worst |
|---|---|---|---|---|---|
| Corner (marker) | 0.18 / 0.5 | 0.15 / 0.3 | 0.18 / 1.3 | 0.08 / 0.3 | 40.4 ms (100 m), 21.3 ms (100 km) |
| Edge 0 N 45 E | 0.24 / 0.7 | 0.21 / 0.7 | 0.26 / 1.3 | 0.10 / 0.3 | 17.5 ms |
| Edge 20 N 45 E | 0.21 / 0.7 | 0.20 / 0.6 | 0.28 / 1.2 | 0.10 / 0.3 | 43.3 ms |
| Face centre 0 N 0 E | 0.33 / 1.2 | 0.25 / 0.8 | 0.28 / 1.7 | 0.09 / 0.3 | 19.0 ms |
| Face 10 N 10 E | 0.20 / 1.3 | 0.18 / 0.8 | 0.21 / 1.0 | 0.08 / 0.3 | 21.5 ms |
- Height calls per pick: mean 37-202, worst 1,208 (a grazing ray at a face centre); the corner is not higher (42-133).
- 97-100 % of a pick's time is `field.height`; warm 1.2-3.2 us a call at every site; cold sets 4-17 us a call (the first decode of a block).
- The march has no seam-dependent path: `pick_surface` reads the field by direction only (`r(d) = R + h(d)`), so a face seam changes nothing in the march; only the field's block reads differ.

### Editor smoke, frame phases (mean / longest ms over the run; `raw/smoke_*.txt`)
| Phase | Marker, editor on (89 frames) | Marker, editor off (46) | 10 N 10 E, editor on (88) |
|---|---|---|---|
| editor update (picks, outlines, lists) | 0.17 / 2.1 | 0.00 / 0.0 | 0.11 / 1.3 |
| edit republish | 0.13 / 2.0 | 0.00 / 0.0 | 0.11 / 1.5 |
| plan and residency | 2.55 / 18.2 | 3.49 / 9.1 | 2.19 / 7.9 |
| editor panels (egui) | 0.51 / 4.4 | 0.42 / 4.4 | 0.51 / 4.4 |
| input, commands and the smoke script | 26.3 / 2,200 | 46.2 / 2,026 | 11.8 / 961 |
- The software adapter's render submit and exposure readback (seconds a frame) dominate the wall time and are left out; the CPU phases above are the editor's.
- The one large CPU phase, "input, commands and the smoke script" (longest 2.0-2.2 s at the marker, 0.96 s at the face), is as large with the editor off, so it is not editor-mode cost; it is the smoke script's own steps (the site's flight and the first reads there) and is outside this item.

## Cause
- Refuted: the pick across face seams. The pick is as cheap at the corner as anywhere (sub-millisecond warm), and its only expensive case, a cold block decode (up to about 43 ms), is site-independent and has run off the main thread since #858 (VIEW-059, with the read-ahead of #831).
- Most likely history (not reproducible on today's code): on 2026-10-21 the hover pick ran on the main thread every frame, so each first read of an undecoded block cost a frame tens of milliseconds; near a corner a moving cursor meets blocks of three faces, which would show more of those frames. Not tested at the October commit (the store and the field changed since: M8's store, no chunk stage).
- Not covered: the owner's own items at the marker (the smoke places at most one edit), and the discrete GPU.

## What a fix PR would change
- Nothing in code. Close the Tracked line as resolved by #831/#858. The owner can confirm with one editor session at the marker on the RTX: editor update under 1 ms mean, as at any other site.

## Visuals
Nothing to look at: CPU timings only, no renders.
