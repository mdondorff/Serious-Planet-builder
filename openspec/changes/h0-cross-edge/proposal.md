## Why
SPIKE-105 §12 F6 and §13.6 (owner D1 = A, conditional; inbox #97 item 2): the bake routes one cube face at a time with the face edge as an outlet or a halo, so a catchment that reaches a cube edge is truncated or joined cosmetically; about 30 % of large catchments reach a face edge. The planet-wide coarse hydrology H0 (K4) must be one connected drainage structure.

## What Changes
- `generators::bake::h0::GlobalGrid`: the six faces as one graph of `6 n^2` cells, neighbours joined across the cube edges (the cells along a shared edge pair 1:1), true cell areas (GEN-138's metric), great-circle distances.
- `GlobalGrid::route`: one priority flood from the sea over the whole graph, steepest receivers on the filled surface, the true drainage area, the outlet each cell drains to.
- GEN-139 added (generator-pipeline), planned.
- Stacked on `change/cell-metric` (GEN-138).

## Impact
- Code: `generators::bake::h0` only; nothing in the bake's output moves (no caller yet).
- The read of the stored field into the heights (F6 read 130 s for six faces at 3,072 cells, decode-bound) and the join with the spill graph for kept pits are the next steps; this PR takes heights as an input array.
