## Why
SPIKE-105 F5: the bake's routing and carve use a uniform cell area `dx^2` and neighbour distances `dx` and `sqrt 2 dx`. On the equiangular cube face the true cell area is 0.707 to 1.000 of `dx^2` (mean 0.849) and the corner distances are 0.94 and 0.67 `dx`. The uniform metric adds 7-14 % channel cells and up to 30 % catchment area to the largest catchments, and biases slope and the stream-power number by up to 1.5x. The bias is in today's bake. Owner D1 (A, conditional, 2026-11-08): step A12 of the revised plan.

## What Changes
- `generators::bake::metric::Metric`: the exact spherical quad area of a cell and the great-circle distance between two cell centres on a window of a cube face (the `TangentWarp` plane, halo cells continue the plane).
- `Grid::with_metric`, `Grid::cell_area(p)`, `Grid::distance_at(p, d)`; without a metric they return `dx^2`, `dx` and `sqrt 2 dx` as before. `Grid` is no longer `Copy` (it holds the metric's tables).
- Routing (receiver slope, the multiple-flow-direction slope and the starting area) and the carve (stream-power distance, breach slope) read them.
- Switch: `CarveParams.cell_metric` (default false). Pass 1 (`pass1_face_rock`) builds the metric when it is on; `bake layer --cell-metric`. The layer keys carry a byte only when it is on, so no key and no output moves with the default.
- GEN-138 added (generator-pipeline).

## Impact
- Code: `generators::bake::{metric, mod, route, carve, face}`, `cache::{layer_store, layer_tiles}` (key byte, `bake_specs_with`), `bake_layer`. No golden moves.
- Pass 2's tile windows, the local carve and the deposition sweep keep the uniform metric (they do not take it yet); H0 across cube edges and H1 are the next step (GEN-139).
- Cost with the metric on: an `atan` and `sqrt` per cell and direction in the routing and carve; measured in the PR body.
