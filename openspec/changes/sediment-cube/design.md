## Decisions
- On a cube grid the cell area is `Grid::cell_area(p)` and the distance to a neighbour `Grid::distance_at(p, d)`; on a plane grid `dx * dx` and `dx` or `dx sqrt 2`, as before (a metric plane grid keeps `dx`).
- Deposition: the load is in cubic metres already; every term that turns metres into cubic metres uses the area of the cell it belongs to.
- Pit spreading on a cube grid has its own function; a cell with a neighbour off the grid is left; each neighbour falls by `rise * area(p) / (8 * area(q))`.
- Thermal: the giver loses `k * excess` of height; the receiver gains it times `area(giver) / area(receiver)`. The factor is 1 on a plane grid and is not applied there, so plane bits are unchanged.

## Measurements
- Tier A at 24 cells a face edge (corner cap): volumes kept to 1e-9, compact = masked bits, active set = full pass. No landmass-scale run.

## Not done
- No call site: K7, the tile pass and the rounds do not use these on the cube grid yet.
- The breach-lowering departure in K7's deposition is not tested (see g-surf).
- G-SED per catchment and per round on the prototype region is not measured.
- Earth-specific values (ADR 0108): none in the spec; the test's 4,000 m body is an example.
