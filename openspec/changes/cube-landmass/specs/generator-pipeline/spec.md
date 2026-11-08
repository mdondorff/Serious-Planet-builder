## ADDED Requirements
### Requirement: GEN-144 The kernels route the six cube faces as one grid
The kernels SHALL run on `Grid::cube`: the six faces as one grid whose neighbours join across the cube edges as in H0 (GEN-139), with the sphere's areas and distances.
- A catchment that crosses a cube edge SHALL be one catchment; with the clipped area the outlets SHALL hold the area of the sphere (relative error at most 1e-9).
- A cell SHALL have its four axis neighbours (across an edge when it leaves its face) and its diagonals only inside its face; the neighbour relation of the axis neighbours and their distances SHALL be symmetric.
- The partitioned route and the round solver SHALL equal the resident route and the whole-region carve bit for bit on the cube grid and on a landmass of it (a mask with the coast ring as outlets).

- The full-size run (`examples/landmass_region`, 3,072 cells a face edge) is a measurement, not a gate; the gates are the tier-A tests.

Verify: A
Status: planned
Source: A3 sections 4.1, 6.1 and 6.2, inbox #98 step 2, GEN-139, ADR 0112 (proposed)

#### Scenario: a cube sphere
- **WHEN** a sphere of 24 cells a face edge with a land sloping to a sea cap across several faces is routed with the round rules, resident and in tiles of 6, 8 and 12 cells, and carved for 2 rounds
- **THEN** receivers cross cube edges, the outlets hold the area of the sphere, the partitioned route equals the resident route, the round solver equals the whole-region carve on 1 and 4 threads, and the same holds on the landmass with the open sea masked out (the outlets hold the area of the active cells)
