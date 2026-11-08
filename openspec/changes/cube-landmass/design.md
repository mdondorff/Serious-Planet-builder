## Context
- A3 section 6.1: a region is a connected landmass routed whole; its border is the sea. H0 (GEN-139) joins the six faces as one graph, but its routing is a separate kernel. The round kernels (GEN-141), the round solver (GEN-142) and the partitioned route (GEN-143) run on `Grid`.

## Decisions
1. `Grid::cube(n, radius)` is the six faces stacked as an `n x 6n` grid: the cell id is H0's (`face n^2 + j n + i`), so the kernels' row-major indexing and tiles still apply, and a tile that divides `n` never straddles two faces.
2. Neighbours: slots as `NEIGHBOURS`. Inside a face as on the plane. Across an edge the axis slot leads to H0's axis neighbour (a table of `24 n` links built from `GlobalGrid::axis_neighbour`); a diagonal across an edge or corner is missing. Areas and distances are the sphere's: `GlobalGrid::area` and the great circle between the centres.
3. Across an edge the faces are turned, so the direction in which a cell reaches back is not the opposite slot. The partitioned route looks the reverse direction up on a cube grid (a plane keeps the opposite-slot table).
4. A landmass is a mask (`with_active`): the active cells are the land cells connected through the cube graph and the sea cells that touch them; the coast ring is the outlets. The partitioned route runs on a mask (the mask is part of the memo key); `round::cut_in` leaves the inactive cells out of the catchments.
5. The region is found at the working resolution from the region's centre. The H0 coarse landmass as the definition is not used here.
6. The jitter stream is one for the whole sphere (the face stream of `FaceSpec::stream` is per face); the cube grid's global id is the cell id.

## Measurements (`examples/landmass_region`, release, 8 threads, 3,072 cells a face edge, round rules, seed 14, one machine)
| Value | Result |
|---|---|
| Landmass of the region's centre | 9,308,154 land cells (84,197,668 km^2), 9,408,756 cells with the coast ring; land cells a face: 292,703 / 2,271,819 / 2,131,086 / 0 / 0 / 4,612,546 |
| Resident route | 19.5 s; 100,602 catchments, largest 389,051 cells; 2,481 land receivers cross a cube edge |
| A_d at the outlets | 85,111,408 km^2 = land plus the coast ring's own area |
| G-PART, tiles of 256 | 864 tiles, 6 sweeps, 667 tile floods, 47.4 s; against the resident route 0 receivers, 0 `A_d` differ |
| E2 pass (2031, 311) lowered by 100 m | 3 of 667 tile floods (0.4 %); tiles changed: filled 1, receivers 1, `A_d` 2; 60.6 s; 0 receivers, 0 `A_d` differ against the resident route |
| 4 rounds of 5 steps | 83.6 s; rounds route 100,602 catchments each, largest 389,016 to 389,079 cells |
| Region land change (n 16,730) | p5 -566.7 m, p50 -133.2 m, p95 -1.6 m, max +14.6 m |
- The window runs (GEN-142, an outlet at the window border): p5 -565.4 m, p50 -132.4 m, p95 -1.7 m, max +16.0 m. Difference at p50: 0.8 m.
- The edit time (60.6 s) is dominated by what is not memoised (A3 section 6.5, GEN-143).
- Memory is not measured: the arrays are resident over all 56.6e6 cells of the sphere (the mask does not shrink them).

## Earth-specific values (ADR 0108 point 4)
| Value | Earth (example) | Rule |
|---|---|---|
The values below are the example's, not a world definition's; the library reads the grid and `CarveParams` only.
| Value | Earth (example) | Parameter |
|---|---|---|
| Cells a face edge | 3,072 (3.26 km) | the example's `n` (the prototype's working resolution) |
| Tile | 256 cells | the example's `tile`; it divides the face edge |
| Seed cell | face 1, (2090, 267) | `SEED_CELL`: the centre of region 1 (results.md) |
| E2 pass | face 1, (2031, 311), -100 m | results.md E2 |
| Radius | 6,371,000 m | `Grid::cube` radius |
| Seed | 14 | the world's seed |
| Epsilon | 1 mm | `CarveParams::epsilon` |
