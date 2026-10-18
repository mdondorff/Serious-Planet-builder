# Water cost on the coast flights: the CPU half (2026-10-18)

Question (opus-planner): water GPU time on the coast flights rose from 1.79 ms (window 6, glide, filter off, valid clock) to about 3.0 ms (window 8). More water on screen, or a costlier shader? This note answers "more water" from the CPU; the shader half is explorer's.

## Method
`water_attrib.rs` (beside this note; an example of the `planet` crate, not kept in the tree) replays the `coast-glide` and `coast-descent` camera scripts of `litscenes::lit_views` (240 cameras each, 2560x1440, cells 32, the lit perf scenes' `tau`) over the production store (16,384 cells, seed 1, `target/accept/global16384`, opened read-only), and for every fourth camera (60 per flight) plans the terrain (`plan_terrain_cached`) and the water (`plan_water`, a fresh `WaterTileCache`, lit view) and counts:
- terrain nodes and water nodes of the plans;
- water triangles (`water nodes x 2 x 32^2`) and vertices (`water nodes x 33^2`): the grid the water pass draws per node;
- screen pixels whose CPU ray meets the sea sphere (level 0) where the terrain lies below the level, inside the tile of a water node (1 ray per 8x8 pixels, counts multiplied by 64).
Each build replayed ITS OWN cameras (the scripts of its `lit_views`); #248 changed `find_coast`, so the flights stand in different places before and after it (see "Same cameras" below). Three builds, each in its own worktree (`.claude/worktrees/attrib-*`, removed afterwards): `b21e4ff` (the window-6 build: window 6 left no commit note; it ran after the flights existed (`dcf03e3`, 16:50 on 2026-10-17) and before the harness changes of #244 (`be11027`, 18:41), and `dcf03e3` and `b21e4ff` have the same water, plan and camera code, so the choice between them does not matter), `f0d68b0` (#248, coastal band: window 7's build), `9f7fb66` (current main, with #261's coarse union). The tool is unchanged between builds except for the `is_sea` helper of the old one. CPU only, no GPU, no change to any branch.

## Result (medians of 60 frames)
| flight | build | terrain nodes | water nodes | water triangles | water vertices | sea pixels (CPU) |
|---|---|---|---|---|---|---|
| glide | window 6 `b21e4ff` | 2,282 | 688 | 1.41 M | 0.75 M | 2.108 M |
| glide | #248 `f0d68b0` | 2,570 | 2,570 | 5.26 M | 2.80 M | 2.107 M |
| glide | main `9f7fb66` | 2,570 | 2,570 | 5.26 M | 2.80 M | 2.107 M |
| descent | window 6 | 1,985 | 632 | 1.29 M | 0.69 M | 2.270 M |
| descent | #248 | 2,233 | 2,233 | 4.57 M | 2.43 M | 2.268 M |
| descent | main | 2,233 | 2,233 | 4.57 M | 2.43 M | 2.268 M |

Ratios, current main against window 6: glide water nodes, triangles and vertices x3.74 (688 to 2,570; range over the flight 574-761 against 2,552-2,597); descent x3.53 (632 to 2,233; 277-888 against 1,754-2,586); sea pixels x1.00 on both (2.108 M against 2.107 M; 2.270 M against 2.268 M); terrain nodes x1.13 (glide) and x1.12 (descent). #261 changed nothing on these flights (#248 and main are identical).

## Same cameras (the check of 2026-10-18)
`find_coast` changed in #248 (a terrain-height beach instead of the bake's sea edge), so the table above mixes the place of the flights with the selection of the water. Current main run again with window 6's coast (the old `find_coast` copied into the tool, `--old-coast`; `frames-main-9f7fb66-window6-cameras.txt`):

| flight | window 6 build, its cameras | main, window 6's cameras | main, its own cameras |
|---|---|---|---|
| glide: terrain nodes | 2,282 | 2,282 | 2,570 |
| glide: water nodes | 688 | 2,033 | 2,570 |
| glide: water vertices | 0.75 M | 2.21 M | 2.80 M |
| glide: sea pixels (CPU) | 2.108 M | 2.171 M | 2.107 M |
| descent: terrain nodes | 1,985 | 1,985 | 2,233 |
| descent: water nodes | 632 | 1,730 | 2,233 |
| descent: sea pixels (CPU) | 2.270 M | 2.380 M | 2.268 M |

- The terrain +12 % is the place: at window 6's cameras the terrain plans are identical in node count (2,282 and 1,985).
- At the same cameras the selection alone gives x2.95 (glide, 688 to 2,033) and x2.74 (descent, 632 to 1,730) water nodes, and the CPU sea pixels grow 3.0 % and 4.8 % (the band floods a little more of the plain).
- The place adds x1.26 (glide, 2,033 to 2,570) and x1.29 (descent, 1,730 to 2,233) on top; the x3.7 and x3.5 of the first table is selection x place. The two places have the same sea on screen to 0.05 %, a coincidence of the two shores.

## Reading
- The sea on screen is the same: the pixels whose ray meets the sea inside a water node are equal to 0.05 % in both flights. Nothing in #248 or #261 made the water cover more of the picture.
- What changed is how much water geometry is drawn to cover it: after #248 every node of the plan carries water (water nodes = terrain nodes, 100 %) where before about 30 % did, so the water pass draws 3.5 to 3.7 times the triangles and vertices for the same pixels. Most of the added nodes are dry or far tiles, flooded nowhere (the shader floods per vertex where `W - h_morph > 0`), so they cost vertex work, rasterisation of triangles and discards, not shading of more water.
- The GPU time went up x1.7 (1.79 to about 3.0 ms, the clocks of window 8 being lower and not comparable). A x3.7 growth in vertices with unchanged pixels is the kind of change that raises time by a fraction of itself; whether the fraction is the whole of x1.7 or only a part is for the GPU side (explorer: the pass with the nodes of the old selection, and with the fade on and off).
- Terrain nodes also grew about 12 % (2,282 to 2,570): that is the new place of the flights (the beach found by terrain height), not a change of the plan (same cameras, same nodes). Of the x3.7 water growth about x2.9 is the selection and x1.26 the place.

## Limits
- The pixel count is a geometric estimate (sea sphere hit, terrain below the level, inside a water node), not the rasteriser's coverage; it is built identically for the three builds. Subsampled 1 in 64, cameras 1 in 4.
- The store is the production store, opened read-only by builds that did not write it (b21e4ff and f0d68b0 read it as written by the current build).

Raw per-frame lines: `water-attribution/frames-window6-b21e4ff.txt`, `frames-pr248-f0d68b0.txt`, `frames-main-9f7fb66.txt` (`flight frame terrain_nodes water_nodes triangles vertices pixels`, then the medians).

## Terrain occlusion and the exact dry tile (2026-10-18, CPU only, 16384 store read-only, main dd967b3)
- A conservative terrain-occlusion cut for water nodes (z-buffer of node quads at R + hmin - sag, 128x72) never dropped visible water, but over 88 cases only 0-8 nodes per frame (106 in total, under 1 % of the 1,100-1,650 kept) are hidden by terrain; the prototype removed 0. CPU 0.35-0.5 ms per frame for a gain of about 0.004 ms; 4-6 coder-days. Not worth building.
- The surplus is 400-700 kept nodes per frame with no visible water (coastal-band and coarse-tile wet answers over land): not occluded, not off-screen.
- An exact dry-tile cut (the minimum height over the tile's own (cells+1)^2 mesh vertices at or above W + A + 4 eps_f32 max(1, |W|, |min|)) drops 238-372 nodes per view with 0 visible-water violations: coast-2km 1,082 to 844, coast-3m 1,181 to 840, glide 1,237-1,354 to 897-1,039, descent 1,157-1,473 to 804-1,135; about 0.09 / 0.17 / 0.16-0.19 ms of water pass (about 0.5 us per node). The minimum must come from the tile's worker job, stored beside the WaterBook entry. About 2-3 coder-days; held for the owner's coast-3m budget decision (about 2.0 ms at 1,290 MHz with it, 2.17 without).
- Sources: scratch `water_occ.rs` and `sweep.txt` (the coordinator's scratchpad); the old `water_attrib.rs` no longer compiles (WaterRequest gained `cut`) and its pixel count ignores terrain occlusion, so the field-march variant was used.
