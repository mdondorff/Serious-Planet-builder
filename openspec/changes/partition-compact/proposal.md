## Why
Briefing item 9 (inbox #104, A3 section 6.6, G-SCALE): the partitioned route is the normal path for large landmasses (owner, 2026-11-08) and did not run on a compact grid (GEN-147 refused it). The rounds at landmass scale on the partitioned route, with their peak memory against the 12 GB budget, were not measured.

## What Changes
- `Grid::layout`, `Grid::xy`, `Grid::at_xy`: the cells of a grid in a rectangle (a compact cube grid: the six faces stacked, `n` wide and `6 n` high).
- `partitioned_route` tiles that layout: a tile is a square of a face holding the active cells inside it; a slot off the landmass is skipped. The refusal of a compact grid is removed.
- `PartCache::without_memory`, `RoundCache::without_memory`: caches that keep nothing between calls (no flood memo, no previous result, no catchment memo), for a solve that is not edited again.
- GEN-149 added (generator-pipeline).
- `examples/landmass_scale part | both`: the rounds on the partitioned route on the compact landmass (`both` compares the bits with the resident rounds), with the process's peak working set.

## Impact
- Code: `generators::bake::{mod, partition, round}`, `examples/landmass_scale`, `tests/bake_compact`. No call site in a pass of the bake; nothing in today's output moves.
- Not in this change: item 9 stays open. The partitioned route keeps its per-cell arrays resident (filled, receivers, area, order and the round's catchment results); tiles bound what each step touches, not the memory. A form that holds these arrays tile-wise on disk is not built. H1 at 12,288 cells and the whole build's peak are not measured.
