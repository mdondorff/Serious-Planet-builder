## Why
Inbox #101 and briefing item 9 (A3 section 6.6, G-SCALE): the rounds on the whole prototype landmass at the erosion grid, with the peak memory measured against the 12 GB world-build budget (ADR 0115 point 7), and the threshold at which the resident route is chosen. The masked cube grid holds arrays over all 906e6 cells of the sphere at 12,288 cells and does not fit, so the measurement asks for a compact layout (A3 section 6.6).

## What Changes
- `bake::compact`: `ActiveIndex` (a bitset of the active cube ids with a rank table and the table of cube ids) and `Grid::compact_cube`: a grid of only the landmass's cells, indexed by rank. Neighbours, areas, distances and the global ids are the cube grid's; a solve gives the bits of the same solve on the masked cube grid.
- GEN-147 added (generator-pipeline).
- `examples/landmass_scale`: the prototype landmass at 3,072 cells times `k` (k = 4 is the erosion grid): found from the 3,072-cell cache, refined, a compact grid, 4 rounds on the resident route, the process's peak working set after each phase, and an optional H1 pass.

## Impact
- Code: `generators::bake::{compact (new), mod}`, `examples/landmass_scale`. No call site in a pass of the bake; nothing in today's output moves.
- Not in this change: a non-resident partitioned route at landmass scale (the resident route fits the budget on this landmass, so it is not needed to meet it); cache eviction (the run uses the resident solver, which has no cache); the tile-partitioned route on a compact grid (the partitioned route needs the rectangles of a face).
