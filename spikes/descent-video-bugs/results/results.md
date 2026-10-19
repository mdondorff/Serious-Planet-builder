# SPIKE-17 results: two bugs of the owner's descent video (descent-video-bugs)

Diagnosis only (owner, 2026-10-19). Spike code and the failing test: branch `spike/descent-video-bugs` (never merged). Machine: Rasierklinge, RTX 3080 Ti Laptop, Vulkan, release build. Video: `descent.mp4`, 1274x712, 30 fps, 65.9 s; frames in `docs/dev_screenshots/2026-10-19-bug-*.png`.

## Bug 1: the atmosphere colour jump (0:26 to 0:28): cause found, test fails today

### Answer in one paragraph
**Terrain farther than 256 km along its ray is composited by two different formulas, one inside the atmosphere top (100 km) and one outside, and the two disagree by about 95 % in the blue channel.** The camera hovering around 100 km flips between them. Inside the top, `atm_ap_lookup` clamps the distance to the last slice of the aerial-perspective volume, `AP_MAX_DISTANCE_M` = 256 km (a constant, `FrameGeometry::words` puts it in `misc.z`), so land at the horizon (up to 1,130 km away) gets the haze of 256 km: clear and green. Outside the top the orbit march integrates the whole ray to the geometry: milky blue-grey. The continuity test that exists (ATMO-006, `the_orbit_march_meets_the_sky_view_just_inside_the_top_of_the_atmosphere`) covers the sky only (no geometry), so nothing noticed.

### Evidence
- The video, one frame per 0.1 s from 25.6 s: a sharp switch at about 26.2 s to a clear look (green far land, dark sea, a darker limb), a sharp switch back at about 28.1 s, and again from about 29.2 s. The near terrain (the islands) is identical in both looks and the camera is almost still: only the far field changes. The hazy look is the state before and after, the clear look is the transient.
- `atm_ap_lookup` (`crates/render/src/atmosphere.rs`): `w = sqrt(t / A.misc.z)`, `pz` clamped to the last slice. The CPU reference (`FrameGeometry::ap_lookup`) is the same. `ap_texel` marches to `min(slice distance, distance to the top)`.
- Test, same ray, same camera pose 1 km below and 1 km above the top, land of lit radiance (0.010, 0.012, 0.006), rays whose ground distance is more than 1.2 x 256 km:

  | ground distance | inside rgb | outside rgb |
  |---|---|---|
  | 801 km | 0.01002 0.01205 0.00617 | 0.04643 0.06347 0.11403 |
  | 569 km | 0.01005 0.01213 0.00638 | 0.03196 0.04882 0.09374 |
  | 463 km | 0.01011 0.01230 0.00689 | 0.02680 0.04200 0.08086 |
  | 396 km | 0.01019 0.01261 0.00802 | 0.02385 0.03764 0.07174 |
  | 349 km | 0.01035 0.01327 0.01055 | 0.02188 0.03452 0.06486 |
  | 314 km | 0.01096 0.01498 0.01622 | 0.02044 0.03214 0.05944 |

  Inside the top the haze adds almost nothing at these distances (the land keeps its green), outside it adds blue 10 to 20 times the land's own radiance.

### The test (fails today)
- `crates/frame/tests/atmosphere.rs`, `terrain_beyond_the_aerial_perspective_range_is_hazed_the_same_just_inside_and_just_outside_the_top` (spec ATMO-006), tier A, GPU-free; the patch is `atmosphere-top-continuity-test.patch` here.
- Run: `cargo test -p planet-frame --test it terrain_beyond`. Run time 0.07 to 0.09 s (the test binary builds in about 50 s once). Fails: `far terrain jumps by 94.6 % across the top of the atmosphere: row 21 t_ground 801 km channel 2: inside 0.00617 vs outside 0.11403`.
- It passes when the inside composite of far land (past the AP range) equals the outside one within 10 %. The 10 % is generous (ATMO-001's GPU bound is 1 %); the fix should reach the discretisation bound of the two marches (the sky test uses 8 % + 3e-4).
- After the fix the test's inside side must read the frame's range instead of `AP_MAX_DISTANCE_M` (`ap_texel` and `ap_slice_distance` take a range). Add a second assertion in the same file: the range is exactly 256 km for h <= 5 km (the near-ground haze is unchanged by construction).

### Proposed root fix
Make the aerial-perspective range a per-frame value: `range = max(256 km, sqrt(c))` with `c = h (2R + h)` the camera's tangent distance squared (1,130 km at the top, 256 km up to h of about 5 km).
- `misc.z` is already a per-frame uniform; `ap_slice_distance(k)` and the froxel march take the range; the slices stay quadratic (`range x ((k + 1)/32)^2`), so the near slices are as fine as before below 5 km altitude and about 4.4 x coarser at the top (slice 0 at 1.1 km instead of 250 m, which is invisible at 100 km altitude). The march keeps `min(2 (iz + 1), AP_MAX_STEPS)` steps; at 1,130 km the last slices have 64 steps over 90 km each, check the ATMO-007 bound at h = 100 km (`the_aerial_perspective_march_is_within_the_bound_of_the_converged_march` has no row above 20 km: add h = 60 and 99 km).
- Costs: spec delta on ATMO-001 and ATMO-007 (range), `numerics-reviewer` (shader), the atmosphere goldens move only where the camera is above 5 km (the lit orbit and aerial 20 km views: `golden-review`, escalated classes), perf: no new pass.
- An alternative with the same outcome and more work: composite far geometry with the orbit march from inside as well (a march per pixel). Not recommended.

## Bug 2: the sea ends at a straight edge: NOT reproduced; cause open

### What the video shows (the frames at 0:34 to the end, one per 2 s)
From 0:36 on the water of the sea stops at a straight line along tile borders (stair steps of several sizes while descending), and beyond it the seabed shows without water, flat and grey (at 0:53 the camera is about 100 m up and the edge is a few hundred metres away, running straight for over a kilometre). The shoreline foam on the left ends exactly at the corner where the edge starts, and the missing region continues the sea beyond. So whole tiles of open sea have no water, not a shallow-ramp effect: the SPIKE-16 patches were dark water with waves absent; these are grey dry seabed. The albedo ramp fix (REND-030, #356) is in main.

### What was checked (all on the editor's default world: `single_planet` seed 1, in-memory 256-cell bake, as `WorldTerrain::of_perf_options`)
1. **Classification near coasts.** 98 coasts found on 5-degree parallels; for levels 9 to 16, all tiles within 4 tiles of each coast point: 0 tiles with a dry answer and terrain (9x9 samples) more than 2 m under the sea. `crates/app/examples/spike17_dry_tiles.rs` (levels 6 to 8 show hundreds of "bad" tiles, all inland basins that are not ocean-connected: the example's truth has no connectivity test; by design dry, ADR 0056 owner constraint 2).
2. **Classification inland (the plain).** Tiles 1 to 130 km inland of the coasts, ocean-connected (bake ocean vertex within 100 km), terrain under -2 m: levels 10 and 12: 0 dry of 1,640 and 1,475; levels 14 and 16: 1 dry of 1,326 and 1,265, both at the 130 km edge of the reach. `spike17_inland.rs`.
3. **Exact plans.** `plan_terrain_field` over `WaterBounded` plus `plan_water` with the cut, from 150 poses over the sea at 60 m, 600 m and 6 km, 4 headings each: 0 nodes near the camera with terrain under the sea and no water node (at 60 km: 18,247 "no-water-answer" nodes 430 to 600 km out, inland basins). `spike17_plans.rs`, about 5 min.
4. **Streaming descents.** The harness of SPIKE-16 (`--perf-smoke --scene look-2km --lit`, the pipelined loop, hardware adapter) with a scripted descent from 100 km to 60 m, altitude falling by e^(-k t), k = 0.255 (the video's pace), 0.6 and 1.2 per second, over the sea off the look-2km coast: 0 gap frames in 3,100, 1,650 and 1,130 frames (a gap: a node within 4 x altitude, terrain under the sea by the 9x9 samples, no water node, any reason). The images (`target/sp17-d2-k0.6.shots`) show water everywhere, also with the plan at 3 to 5 levels coarser than the camera needs. `crates/app/src/watergap.rs` (`WATER_GAP_DESCENT=1`, `WATER_GAP_RATE`, `WATER_GAP_BACK`, `WATER_GAP_DRIFT`; `WATER_GAP_KEEP=1` keeps the scene's own script).
5. **Invariant "a parent is wet wherever its children are" (ADR 0058).** For tiles near coasts, a tile that answers dry while a grandchild answers wet: levels 4 to 8: 3, 3, 1, 2, 1 of about 4,700 per level (seed 1, 256-cell bake); levels 9 to 14: 0. On the 16384-cell store (`explorer/target/accept/global16384`, read-only): 0, 0, 0, 1, 1 at levels 4 to 8 and 0 at levels 9 to 14. The summary answer and the point-by-point answer are identical for every tile checked at levels 6 to 10 (0 differences): the coarse summary is no more generous than the 5x5 samples, so a small sea inside a coarse tile is missed by both. `spike17_parent.rs`.

### Conclusion and open candidates
Nothing in the CPU decision (classification, the node list, the cuts) reproduces a missing sea in the editor's default world in a steady plan or in a scripted streaming descent (one coast, three descent rates). Candidates left, in order:
1. A coarse tile (level 4 to 8, 39 to 600 km wide) drawn near the camera because its children are not resident (a fast descent outruns the streaming; the owner's title bar showed "lag" values) whose water answer is dry although it holds sea: invariant break 5. Rare in the bake checked (1 in 2,000), but it would produce exactly one huge straight edge and a flat grey (coarse) seabed. It needs the owner's coast to test: this is the lead.
2. A state only the window has (the editor UI, `ManagedLoop` with items and preview, the globe overview; the descent in the video starts at the globe at 0:06), e.g. a plan or book entry that the descent from the globe leaves stale.
3. A world other than seed 1 / 256 cells (the owner's command line is unknown: asked via opus-planner).

### Needed to close
- The owner's pose (F9 repro bundle with the camera pose, TEST-008 v3) or the latitude and longitude near the coast at 0:53 and the altitude at 0:36 and 0:53: then a replay with `watergap` logs the verdict of every node (it prints reason, tile, level, true minimum, distance) and the tier A test can be written from the failing tile (a pure `tile_water` check on the failing tile id, GPU-free, expected under 0.1 s).
- Test shape if candidate 1 holds: tier A, `crates/frame/tests/water_band.rs` or `water_summary`, "a tile is wet where any descendant is wet" over the failing tile's descendants down to the first wet one (a real-world check needs a bake: the 256-cell bake opens in 1.7 s, `WaterTileCache::tile_water` is microseconds).

## Files (spike branch)
- `crates/frame/tests/atmosphere.rs` (the failing test), `crates/app/src/watergap.rs`, `crates/frame/src/water.rs` (the per-node diagnostic record), `crates/app/src/streamperf.rs`, `crates/app/examples/spike17_*.rs`.

## Addendum: the 16384-cell store (`explorer/target/accept/global16384`, read-only)
Same probes, same result as the 256-cell bake: inland plains (levels 10, 12, 14, 16; 1,611, 1,460, 1,325, 1,269 ocean-connected tiles under -2 m): 0 dry, except 1 at 130 km at level 12 (the reach edge); near-coast tiles of levels 10 and up: 0 dry under -2 m (levels 6 to 9 count inland basins that are not ocean-connected, by design dry); the summary and the point-by-point answers are identical (0 differences at levels 6 to 10); invariant break 5: 2 of 4,775 tiles at levels 7 and 8, none at levels 4 to 6 and 9 to 14. A finer bake does not make the classification a lead; the owner's pose is still needed.
