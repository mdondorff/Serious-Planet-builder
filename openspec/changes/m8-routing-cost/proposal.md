# m8-routing-cost

The cold routing of an A-net chunk is two thirds of its time at 8 m (8.9 s of 13.5 s, perf window 2026-10-26; GEN-052 budget 4 s). The profile (capture-stills method, not a measurement) puts 85 to 88 percent of the routing in the base-field evaluation at the raster points of the least-cost search (5.9 M directions for four chunks, 3 us each); the search itself is 7 percent.

## Changes
- GEN-077: the routing reads the base heights of the region's cells (read once before the routing) at raster points inside the region, and a stage-owned memo of raster heights by lattice point (bounded count, cleared at the cap) at the others; only points in neither are evaluated. The values are the base field's heights at the points' directions, so every polyline, bed and chunk hash is unchanged.
- No change to the routing law, the raster or the hash.
