# Water cost on the coast flights: the CPU half (2026-10-18)

Question (opus-planner): water GPU time on the coast flights rose from 1.79 ms (window 6, glide, filter off, valid clock) to about 3.0 ms (window 8). More water on screen, or a costlier shader? This note answers "more water" from the CPU; the shader half is explorer's.

## Method
`water_attrib.rs` (beside this note; an example of the `planet` crate, not kept in the tree) replays the `coast-glide` and `coast-descent` camera scripts of `litscenes::lit_views` (240 cameras each, 2560x1440, cells 32, the lit perf scenes' `tau`) over the production store (16,384 cells, seed 1, `target/accept/global16384`, opened read-only), and for every fourth camera (60 per flight) plans the terrain (`plan_terrain_cached`) and the water (`plan_water`, a fresh `WaterTileCache`, lit view) and counts:
- terrain nodes and water nodes of the plans;
- water triangles (`water nodes x 2 x 32^2`) and vertices (`water nodes x 33^2`): the grid the water pass draws per node;
- screen pixels whose CPU ray meets the sea sphere (level 0) where the terrain lies below the level, inside the tile of a water node (1 ray per 8x8 pixels, counts multiplied by 64).
Three builds, each in its own worktree (`.claude/worktrees/attrib-*`, removed afterwards): `b21e4ff` (main before window 6 harness lessons; the last code commit before window 6, assumed, the window itself left no commit note), `f0d68b0` (#248, coastal band: window 7's build), `9f7fb66` (current main, with #261's coarse union). The tool is unchanged between builds except for the `is_sea` helper of the old one. CPU only, no GPU, no change to any branch.

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

## Reading
- The sea on screen is the same: the pixels whose ray meets the sea inside a water node are equal to 0.05 % in both flights. Nothing in #248 or #261 made the water cover more of the picture.
- What changed is how much water geometry is drawn to cover it: after #248 every node of the plan carries water (water nodes = terrain nodes, 100 %) where before about 30 % did, so the water pass draws 3.5 to 3.7 times the triangles and vertices for the same pixels. Most of the added nodes are dry or far tiles, flooded nowhere (the shader floods per vertex where `W - h_morph > 0`), so they cost vertex work, rasterisation of triangles and discards, not shading of more water.
- The GPU time went up x1.7 (1.79 to about 3.0 ms, the clocks of window 8 being lower and not comparable). A x3.7 growth in vertices with unchanged pixels is the kind of change that raises time by a fraction of itself; whether the fraction is the whole of x1.7 or only a part is for the GPU side (explorer: the pass with the nodes of the old selection, and with the fade on and off).
- Terrain nodes also grew about 12 % (2,282 to 2,570), so part of the added water nodes is the added terrain nodes; that growth is in the terrain plan and predates the water selection (it is not explained here).

## Limits
- The pixel count is a geometric estimate (sea sphere hit, terrain below the level, inside a water node), not the rasteriser's coverage; it is built identically for the three builds. Subsampled 1 in 64, cameras 1 in 4.
- The window-6 build is the last code state before that window as far as the log shows, not a recorded commit.
- The store is the production store, opened read-only by builds that did not write it (b21e4ff and f0d68b0 read it as written by the current build).

Raw per-frame lines: `water-attribution/frames-window6-b21e4ff.txt`, `frames-pr248-f0d68b0.txt`, `frames-main-9f7fb66.txt` (`flight frame terrain_nodes water_nodes triangles vertices pixels`, then the medians).
