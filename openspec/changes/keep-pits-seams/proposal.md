## Why
The keep-pits preview (inbox #66, seed 14, production grid) found the tile-local fill depth of #1192 seam-inconsistent in the mountains: across the seam 2/12 | 2/13 the fill adds a median 152 m (max 255 m) to the height step at pit cells, while the unfilled surfaces agree. A basin wider than the 128-cell halo is closed in one tile's window and drains through the window's border (an outlet) in the next. Pass 1 has no pit at those cells, so no larger-scale input repairs it. ADR 0101 needs a seam-consistent fill of the deep mountain basins (options (b) and (c) leave exactly those open or only move the seam).

## What Changes
Option (a) of the report, split in three changes of one branch each (planner, 2026-11-05):
- **PR 1 (this one): GEN-121, the kernel.** `generators::bake::spill`: `flood_labelled` (a tile's priority flood from its outlets and its border cells with territory labels), `SpillGraph` (the spill graph over tiles, tile edges, cube edges and corners named by the caller; the planet-wide level of every seed), `final_level`. Tested against a global minimax flood as the oracle. No store, no format, no bake behaviour changes.
- PR 2: STRM-077. Pass 2 writes a flood record (labels and the quantised final heights) as bake scratch; the post-pass `bake fill` solves the graph and writes kind 17 from the merged levels; the depth code is a key parameter (u16 option).
- PR 3: the gate and preview (the seam pairs 2/12 | 2/13 and a cube-edge pair with a deep basin; pit share per class unchanged).

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-121 added (planned)

## Impact
- Code: `generators::bake::spill` (new). Tests: `generators/tests/bake_spill.rs`.
- Tracked until PR 2 lands: kind 17 on main is the tile-local fill of #1192 (not seam-consistent for basins wider than the halo); no planet bake runs (inbox #67).
