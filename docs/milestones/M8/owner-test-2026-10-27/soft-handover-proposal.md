# A soft hand-over of the chunk across the levels below its apply level (proposal, option A of D0; nothing built)
Status: proposal (crusader, 2026-10-28), for the owner's choice between A, B and C. Measurements: probes `boundary_report`, `driver_step_report`, `entry_step_report` on branch `change/m8-group-d`.

## What the probes show at the light-red boundary (level 15 = `ANET_TOP_LEVEL`, frame-50119; frame-41356 the same within 5 %)
| Quantity at the level-15 / level-14 edges | Value |
|---|---|
| Height step (the chunk's change at level 15, absent at level 14) | p99 1.35 m, max 1.52 m |
| The base's interpolation error between the two tile resolutions | p99 0.08 m |
| Samples on different sides of sea level | 0 of 1089 |
| Driver texels with chunk codes (level 15) against the neutral codes (level 14), 28 nodes, 2268 texels | soil depth mean 0.11 m, max 0.52 m; insolation 0.004 at most; moisture mean 0.20, p99 0.30, max 0.36 (0 to 1), 17.4 % of the texels beyond 0.25 |
- The ground colour class (moisture) switches at level 15 by 0.2 of its range on average; the height switches by at most 1.5 m at the coast and up to tens of metres on incised chunks (`m8-chunk-fade` design 6a: 58 m).

## The change
1. A blend weight by level: `w(level) = 1` for `level >= top`; for `top - K <= level < top`, `w = (level - (top - K) + 1) / (K + 1)`; 0 below. `K` levels of hand-over (3 to start: levels 12 to 14 for the apply level 15).
2. Height: for a level in the hand-over band the change is `w(level) x` the change read at the coarsest pyramid level (the pyramid's top level, the chunk's own coarsest samples), so a tile of level 14 carries a third of what level 15 carries and the step between neighbours of consecutive levels is `1 / (K + 1)` of the full change.
3. Drivers: for the same levels the driver codes are `neutral + w (chunk - neutral)` per texel (`driver_block`), read from the same pyramid level; the moisture class then moves by about 0.05 per level instead of 0.2 at once.
4. Bounds and plan: the stated bound is unchanged (a blend of the change is within it); the plan reads the chunks of a tile of level `top - K` and finer (`chunks_read_by`), ready or not (LOD-013), so the plan stays independent of the chunks; a tile in the band re-keys when a chunk under it lands (the existing keyed refresh).
5. Morph: a tile of the band morphs to its parent as today; with the change in both, the morph differences shrink by the same factor.

## Costs and risks
- Work: tiles of levels 12 to 14 under the chunks now refresh when a chunk lands (a few hundred tiles in a view; each costs 2 to 100 ms by level, window 19).
- The coarser tiles carry a smoothed copy of the change: at level 12 a tile spans a chunk; the pyramid's top level (cell about 76 m) is finer than a level-12 vertex (2400 m / 32 = 75 m): about equal; at level 14 (19 m vertices) the top pyramid level is coarser than the tile's own spacing, so the tile interpolates it (as a mip map does, STRM-047).
- Tests to write: the step between consecutive levels along an edge at the coast and on an incised chunk (the entry-step probe at each level of the band), the drivers per level, plan independence of readiness, a golden of the coast pose.
- Reversal: `K = 0` is today's behaviour.
- Does not by itself touch the held-back tiles (the 300 m widening) or the valley disconnect between levels 4 and 14 (D0 options B and C).
