# m8-routing-cost

The cold routing of an A-net chunk was a large part of its time (perf window 2026-10-26: 8.9 s of 13.5 s on the 256-cell bake scene, whose coarse segments are 8 times longer than the production run's; on the production geometry a chunk's network build is 4.4 to 5.4 s, of which the region's base heights are 2.3 s). The profile puts 85 to 88 percent of the routing in the base-field evaluation at the raster points (3 us each, flat in the batch size); the search is 7 percent.

## Changes
- GEN-077: the routing reads the region's base heights (read once before routing) and a stage-owned memo of raster heights (cap 1,048,576 points) before evaluating the base. Bit-neutral.
- GEN-078: segments that cannot reach the region are not routed. Bit-neutral.
- GEN-066: the routing raster stride 4 to 8 (64 m at 8 m cells), a stage parameter under ADR 0081 point 5. The routed paths and the chunk hashes change; the pinned known answer of the synthetic chunk is re-pinned.

## Measured (chunk_incision example, production geometry, one chunk cold, loaded machine, not a perf measurement)
| Site | Network build main (s) | stride 4 with GEN-077 and GEN-078 (s) | stride 8 (s) | Climbing links main | stride 8 |
|---|---|---|---|---|---|
| lowland | 5.51 | 4.47 | 3.65 | 0 | 0 |
| hills A | 4.58 | 4.41 | 3.62 | 7 | 0 |
| hills B | 4.37 | 3.99 | 3.37 | 0 | 0 |
| steepest | 5.43 | 5.10 | 3.84 | 519 | 411 |
