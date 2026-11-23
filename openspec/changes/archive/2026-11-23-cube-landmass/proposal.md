## Why
A3 (docs/design/terrain-target.md) section 6.1: a region is a connected landmass with its sea outlets, routed whole, with no artificial cut. The prototype region's landmass crosses cube edges (inbox #98 step 2, GEN-139's graph). This change gives the kernels the cube topology so that rounds, the resident route and the partitioned route run on the landmass.

## What Changes
- `Grid::cube(n, radius)`: the six faces as one `n x 6n` grid (cell `face n^2 + j n + i`, the id of `GlobalGrid`), neighbours joined across the cube edges as in H0 (the axis neighbour across an edge, no diagonal across an edge or corner), the sphere's areas and great-circle distances. `bake::cube` holds the topology.
- The partitioned route finds the reverse direction of a neighbour by lookup on a cube grid (faces are turned across an edge), runs on a masked grid (a landmass) and requires tiles that divide a face edge; the round solver cuts catchments over the active cells (`round::cut_in`).
- GEN-144 added (generator-pipeline).
- `examples/landmass_region`: the landmass of the prototype region (3,072 cells a face edge) from the region's centre through the cube graph, its coast ring as outlets; resident route, G-PART, E2, 4 rounds of 5 steps and the region's change.
- ADR 0112 (proposed) point 4 corrected to the local drop of the foreign MFD share (A3 section 6.4, GEN-141).

## Impact
- Code: `generators::bake::{cube (new), mod, partition, round, h0}`, `examples/landmass_region`. Plane grids are unchanged; no pass of the bake builds a cube grid.
- Not in this change: the cube grid in pass 1 or pass 2; a non-resident cube route; the H0 coarse landmass as the region definition (the example finds the landmass at the working resolution).
