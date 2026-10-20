## Why
ADR 0071 (accepted 2026-10-20): coastal water is decided by connectivity through low ground, not by a 100 km reach. The owner's captures showed the sea ending at a straight edge on a plain connected to the ocean for 120 to 132 km (SPIKE-17 bug 2). `wet-floor-store` (#456) added the wet floor and its store blocks; this change makes the water field read it.

## What changes
- `SeaProducer::with_floor`: a vertex is wet when it is ocean or its floor is at most `bound_code(B(W))`; `water_at` takes `ocean || wet`, the new `wet_at` (trait method, `WaterField`, `WaterSource`) replaces `ocean_bit_at`.
- Frame `band_water`: a tile with no sea of its own carries it when a sample is band and a vertex of its interpolation stencil is wet. `COAST_REACH_M`, `REACH_PROBES` and the reach arithmetic are removed.
- `WaterSummary`: per cell the SEA flag, the lowest floor and the band elevation (the OCEAN and WET flags go); codec version 2, `cpu-water-summary-v2`, key including the floor implementation id; `tile_water` is a comparison on the cells with no distance. `summarize_block` takes the block's floors; `water_summary_of_store` reads the floor blocks.
- `WATER_VERSION` 2 and `cpu-water-level-v2` (the answers change). A viewer refuses a store with the ocean channel and no floor blocks, with the command that writes them.
- `BakeProbe::wet_vertex`; the coverage oracle of `gpu_water_coverage` decides connectivity by it, and its inland view stands inland of the plain coast (the beach's inland has no terrain under the level).
- The SPIKE-17 test `water_reach` (the three tiles beyond the old reach) is part of tier A. `docs/learnings.md`: `B(W)` bounds flat-ground incision only.

Expected golden changes:
- none (the goldens render synthetic and fixed worlds whose plans carry their water; no golden moved in a full `test gpu` run on WARP)
