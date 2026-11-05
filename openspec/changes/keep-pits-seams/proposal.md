## Why
The keep-pits preview (inbox #66, seed 14, production grid) found the tile-local fill depth of #1192 seam-inconsistent in the mountains: across the seam 2/12 | 2/13 the fill adds a median 152 m (max 255 m) to the height step at pit cells, while the unfilled surfaces agree. A basin wider than the 128-cell halo is closed in one tile's window and drains through the window's border (an outlet) in the next. Pass 1 has no pit at those cells, so no larger-scale input repairs it. ADR 0101 needs a seam-consistent fill of the deep mountain basins (options (b) and (c) leave exactly those open or only move the seam).

## What Changes
Option (a) of the report, split in three changes of one branch each (planner, 2026-11-05):
- **PR 1 (merged, #1197): GEN-121, the kernel.** `generators::bake::spill`: `flood_labelled` (a tile's priority flood from its outlets and its border cells with territory labels), `SpillGraph` (the spill graph over tiles, tile edges, cube edges and corners named by the caller; the planet-wide level of every seed), `final_level`. Tested against a global minimax flood as the oracle. No store, no format, no bake behaviour changes.
- PR 2 (this one): STRM-077 and the amendments of GEN-120 (the depth is the planet-wide level) and STRM-076 (kind 17 is 16-bit codes at 0.25 m, written by the post-pass; kind 16 is not used). Pass 2 leaves its final heights (f32, exact: the scratch is deleted anyway) as bake scratch (kind 18); `layer_fillpass::run_fill_pass` floods each core, joins the tiles over tile edges, cube edges and corners (`bake::fillgeom`), solves the spill graph and writes kind 17; `bake layer` runs it after pass 2 and deletes the scratch. Pass 2's window flood and pass 1's face fill are removed. `BAKE_KERNEL_VERSION` and `PASS2_KERNEL_VERSION` do not move (the stored changes are the same bits); `CHANNELS_VERSION` 3 re-keys kinds 14 and 15 as well (a store from before is refused, not rebaked; #1192 never went into a bake), and the post-pass kernels have their own `FILL_KERNEL_VERSION` in the fill and scratch keys. The 0.25 m depth code and the exact-base depth are the owner's decision (2026-11-05).
- PR 3: the gate and preview (the seam pairs 2/12 | 2/13 and a cube-edge pair with a deep basin; pit share per class unchanged).

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-121 added (planned); GEN-120 amended (in `keep-pits`)
- `streaming-cache`: STRM-077 added (planned); STRM-076 amended (in `keep-pits`)

## Impact
- Code: `generators::bake::spill`, `fillgeom` (new), `tile` and `face` (pass 2's window flood and pass 1's face fill removed), `cache::layer_fill`, `layer_fillpass` (new), `layer_tiles`, `layer_store`, `key` (kind 18), `bake_layer`. Tests: `generators/tests/bake_spill.rs`, `bake_fillgeom.rs`, `bake_tile.rs`, `cache/tests/layer_fill.rs`.
- The Tracked item of PR 1 (kind 17 tile-local) is removed here.
