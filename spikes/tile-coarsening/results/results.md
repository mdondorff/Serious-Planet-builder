# SPIKE-18 results: the drawn tiles go coarser for a moment during a slow approach (tile-coarsening)

Diagnosis only (owner, 2026-10-19). Spike code and the failing test: branch `spike/tile-coarsening` (never merged). Machine: Rasierklinge, RTX 3080 Ti Laptop, Vulkan, release build.

## Answer in one paragraph
**It reproduces only when the machine is starved, and the cause is the cover's split rule.** `cover_selection_bounded` draws a tile's children only if *every child in view* is resident; if one is not, the tile is drawn itself (STRM-016: "a split with a tile missing is undone"). A tile that was split a frame ago because its sibling was out of view (culled) is therefore taken back to its parent, one level coarser over its whole area, the moment the sibling comes into view (the camera turns; a straight approach did not show it) before it has streamed in, although the finer tiles are still resident. Without load the streamer is never behind and the frame never notices; with the plan worker and the loaders starved (the owner's laptop with three coder sessions building) it happens for a few frames at a time, in any overlay (the tiles view only shows it).

## Method
1. A detector in the streaming driver (`crates/app/src/coarsen.rs`): 3,000 probe points over the sphere plus 3,000 around the target; for each probe the level of the drawn node whose tile covers it; a **violation** is a probe whose level drops between two frames while the camera is nearer to it. Sanity: 8,454 finer transitions and 8 M comparisons in one run, so it sees changes.
2. A scripted slow approach (`COARSEN_APPROACH=1`, `watergap::approach_script`): the altitude falls by e^(-k t) from 20,000 km to 3 km, 75 degrees down (default), optional yaw `COARSEN_YAW` degrees per second, look-2km coast, the pipelined driver of the window (`ManagedLoop`, plan and cover plans) on the RTX.
   `COARSEN_APPROACH=1 COARSEN_LOG=<file> COARSEN_RATE=0.15 COARSEN_YAW=30 planet test-render --perf-smoke --scene look-2km --lit --allow-memory-bake --global-res 256 --adapter hardware --frames 3600`
3. Load: 24 busy-loop processes next to the run (a machine with other builds running).

## Evidence
| setup | result |
|---|---|
| unloaded, rates 0.03, 0.08, 0.2, 0.3, 0.6 per s, pitch 20 to 90, 256 bake | 0 violations in every run |
| unloaded, 16384 store (`explorer/target/accept/global16384`), 0.08 per s | 0 violations |
| unloaded, with yaw 20 and 60 degrees per s | 0 violations |
| exact plan only (`spike18_exact`), 6 targets, pitch 20 to 90, 440 to 880 steps | 0 violations: the selection is monotone |
| **loaded, 0.15 per s, yaw 30** | **223, 19, 78, 21, 29 and 25 violations in six runs (9 to 34 frames each)** |
| loaded, 0.15 per s, no yaw | 0 violations (3,600 frames): a turn is needed, the sibling must come into view; a straight approach alone does not do it |
- Every violation is in a frame of kind **Cover**; 19 of 21 follow another Cover frame (so it is not "stale plan dropped for the cover"), 2 follow a Stale one. One level at a time: L4 to L3 at 4,703 km (132 probes in one frame), L5 to L4 at 1,593 km, L6 to L5 at 176 to 302 km, L7 to L6 at 70 to 105 km.
- At each violation: the previous (finer) tile is **still resident** (21 of 21), its parent resident, and the new (coarser) tile has 1 to 3 children **not resident**, none of them **ever resident** before (`-n` in every case, none `-e`: not evicted, never streamed): `["-n","-n","Re","Re"]` and the like (`target/coarsen-ever.log`).
- A plan **refusal** is not the cause: `PlanPipeline::begin_frame` returns `Err("plan for frame N: ...")` for a refused plan, which stops the window (the tracked frame-35 flake), it never draws a coarser plan. F4's `with_coarse_fallback` only matters with the coarse view on (the owner's overlay said `LOD x1`).

## Why the cover is coarser than the frame before
`cover_selection_bounded` (`crates/frame/src/lod.rs`): a tile is split while its distance is below the level's split distance **and every child that is in view is resident**. A child that the frustum, the horizon or the tight cut culls does not count. One frame the sibling is culled and the tile is split into its resident children; a frame later (the camera turned or came closer so the sibling's volume is in view) the sibling counts, is not resident (the streamer loads the tiles in view first and the rest of R, the out-of-view tiles, later; under starvation "later" is many frames) and the whole parent is drawn instead. Nothing is blank, so no test notices; the frame is just a level coarser over the parent's area.

## The test (fails today)
- `crates/frame/tests/cover.rs`, `one_missing_in_view_tile_does_not_take_its_resident_siblings_back_to_their_parent` (spec STRM-002, STRM-016, LOD-007), tier A, GPU-free, 0.01 s run time (the test binary builds in about 50 s once): R of a camera resident except one leaf `s` and what is under it; the cover must keep drawing the resident siblings of `s` at their level or finer. Fails: `aerial-20km: with TileId(432345564227571462) (level 6) missing, its resident sibling TileId(432345564227571463) is drawn as TileId(360287970189640641), a level coarser`.
- The patch is `cover-missing-sibling-test.patch` here. It states the property the fix must give; it contradicts the cover's documented rule ("a split with a tile missing is undone"), so the fix changes the rule and the spec text of STRM-016.
- Not enough on its own as a repro of the owner's frame: the model tests that move a camera over a lagging plan and a D-first residency did not reproduce it (all Cover frames, 0 violations) because they stream every tile of R; the starvation recipe below is the end-to-end check.

## Proposed root fix (the main lane decides)
1. **Draw the resident children and keep the parent under them for the missing child's area only** (the parent drawn too, the children win by the depth test with a polygon offset, or the missing child's quarter drawn from the parent with a stencil): a resident tile is never taken away because a sibling is missing. Costs: some overdraw, a renderer change (REND), the plan node list carries both.
2. Or **hysteresis in the cover**: keep drawing a tile that was drawn in the last frame while it is resident and its parent's split would be undone only by a child that came into view this frame (state in the loop, not a pure function; the pure cover would take the last frame's leaves as input).
3. Reduce the frequency (does not remove the case): load the **siblings of every drawn tile** with the in-view tiles (D) so a turn or an approach finds them resident: widen D to the parent's children.
Option 1 or 2 satisfies the test above; option 3 only makes it rarer.

## Starvation recipe (reproducible setup)
24 busy-loop processes (`while :; do :; done &` x 24 under Git Bash) next to the harness command above, yaw 30 degrees per second, rate 0.15 per second: 19 to 223 violations in 3,600 frames; about 4 to 8 minutes per run under load. A deterministic variant that needs no load was not found (the model tests pass); the tier A test above is the deterministic check of the mechanism.

## Files (spike branch)
`crates/app/src/coarsen.rs` (detector), `crates/app/src/watergap.rs` (`approach_script`), `crates/app/src/streamperf.rs` (hook), `crates/app/examples/spike18_exact.rs`, `crates/frame/tests/cover.rs` (the failing test and two model tests that pass).
