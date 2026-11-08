## Context
- SPIKE-105 F6: a planet-wide route over six faces at 3,072 cells takes about 7 s once heights are in memory (flood 5.3 s, receivers 1.2 s, accumulation 0.6 s); 264,345 catchments, 896 spanning two or more faces and holding 26.6 % of land; the largest is 5.9e6 km^2 across faces 0 and 3.
- GEN-138 gives the true area and distance of a cell of one face.

## Decisions
1. One id space: `id = face n^2 + j n + i`; the graph is implicit (neighbours computed on demand), so memory is the arrays the caller holds.
2. Across a cube edge only the axis neighbour exists. The edge cell's neighbour is found by mapping a point just past the edge at the cell's tangential centre into the face it lands in (`direction_to_face`); the equiangular coordinate along a shared edge is the same in both faces, so the cells along an edge pair 1:1 and the relation is symmetric (tested). Diagonals across an edge or a corner are not edges of the graph (a corner cell has two cross-edge neighbours, not three).
3. Flood and routing as GEN-100's, D8 receivers, on the graph: priority flood from the outlets with `epsilon` over flats, pop order by height then id, the receiver the steepest descent on the filled surface by true distance, the area summed down the reversed flood order. Single-threaded and fixed-order (CON-14).
4. The area is the true area (GEN-138's `Metric` per face), so the area at the outlets is the area of the sphere (tested to 1e-9).
5. Not in this PR: reading the stored field into the heights (a lattice read, SPIKE-105 F6's 130 s decode), multiple-flow-direction accumulation (H0 stays D8: 3 km cells and trunk rivers), joining kept pits across tiles (the spill graph of GEN-121 at H0's scale), watershed domains (prototype step).
6. Cost, measured (`examples/h0_cost`, release, one thread, synthetic surface): 1.4 s at 1,024 cells a face edge and 16.2 s at 3,072 (6.0 s and an extrapolated 55 s before the speed-up; F6's prototype: 7 s). The speed-up: directions from per-axis tangent tables and per-face frames (no trigonometry per call), neighbour ids without distances in the flood, and the distance only for lower neighbours in the receiver pass. A cached neighbour table (24 bytes a cell) is the next step if H0 is re-routed on every edit (L1 target 10 s).

## Earth-specific values (ADR 0108 point 4)
| Name | Value | Unit | Kind | Parameter |
|---|---|---|---|---|
| planet radius | caller's | m | planet state | `GlobalGrid::new(n, radius_m)` |
| flood epsilon | caller's (1e-3 in tests) | m | numeric | `route(.., epsilon)` |
| `PAST` | 1e-6 | plane units | numeric tolerance (how far past an edge the neighbour probe lands; far under a cell at any supported n) | `h0::axis_neighbour` |
No calibration constant is introduced. The cost figures are indicative (a synthetic surface, not a perf-window measurement).
