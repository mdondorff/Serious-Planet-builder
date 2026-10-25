## Why
ADR 0081 point 3 and the planner's decision (2026-10-25): tributaries are "routed the same way" as the stems, from the coarse run's links, so borders agree by construction (topology and crossings come from the global run).

## What Changes
1. `AnchorLaw::river_area_m2` (GEN-068): the drainage area above which a coarse link is a river and is routed (node slide, least-cost segment, bed rule, valley law by its area); `RIVER_AREA_M2` (100 km2, the stems alone) is the default of the law and `ANetParams::spike` sets 5e7 m2 (two coarse cells). `ANET_VERSION` 7.

## Measured (shared store, input-plus-fill run of #681, base commit e4a65caf; `chunk_incision --river-area`)
- Corridor cells at 1e8 / 5e7 / 3e7 m2: lowland 2796 / 2796 / 3447, steepest river 3717 / 4027 / 4027, hillsB 1380 / 1797 / 1797. Climbing links and the off-corridor census do not move (0.10 / 0.43 / 0.34 km per km2 over 1e5 m2; none over 1e6 m2).
- 3e7 m2 gives a link with a 27.8 m drop at the lowland site: not used.
- The coarse run's own links add little: the coarse cell is 24 km2, so a link under 1e8 m2 is one to three cells. The missing tier is the fine first-order network (heads of 0.1 to 1 km2, below one coarse cell): M9 (ADR 0081 point 3, the roadmap's erosion that forms its own tributaries inside chunks).

## Known design constraint for the fine tier
Drainage accumulated inside a chunk's region depends on the region's extent: a catchment larger than the halo restarts from zero at the region edge, so a channel derived from the region's own flow ends or appears on a straight line at a chunk border (a seam, criterion 2). A per-chunk-core tier has the same border (a 1 to 100 km2 tributary is larger than the 24 km2 core). The fine tier needs the area from the global run at the chunk's edge (an inflow per entry point, as the stems have) or a block of chunks.

## Tests
- Tier A: the tributary of a fork has a corridor above the threshold only.
