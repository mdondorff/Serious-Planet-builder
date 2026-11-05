## Why
The gate of keep-pits-seams (planner inbox #73, 2026-11-05) found that the post-pass treats every cell at or below the sea level as sea: 1.07 million cells in 14,547 inland components (0.94 % of the seam region of 2/12 | 2/13) read as sea, so an inland basin below 0 m would stay a hole. Owner and planner: only cells connected to the open ocean are outlets; inland basins at or below 0 m are filled like any other pit (later lakes or salt flats).

## What Changes
- `generators::ocean::ocean_at(lattice, direction)`: whether any of the four bake vertices of the lattice cell holding the direction is ocean (the water field's wet rule).
- `layer_fillpass::run_fill_pass` takes an `ocean` closure; the outlets of a core are the cells at or below the sea level that it calls ocean; the others are inland basins, filled like any pit; the report counts them (`inland_low_cells`). `bake layer --global-dir` passes the stored field bake's ocean channel (`StoreOceanSource`); without a store every such cell is an outlet, as before.
- `FILL_KERNEL_VERSION` 3 (the fill keys change). STRM-077 amended in the keep-pits-seams delta.
- Tests: `ocean_at` on a small lattice; an inland bowl 5 m below the sea level is filled to its rim (205 m) when the mask says inland and holds no depth when it says ocean, and the report counts the 64 inland cells; a mask that differs across tile seams gives the depth of a global flood with the same outlets, code for code; another outlet mask in a filled directory is refused.

## Count on the seam region (12 tiles of 2/12 | 2/13, the gate's final heights)
- 15,345,000 cells at or below 0 m. With the ocean channel of an in-memory bake of seed 14 at 2,048 cells a face edge (4.9 km vertices; no stored field bake of seed 14 at this version exists to read): 13,915,927 on ocean vertices and 1,429,073 (9.3 %) not ocean. The connected-component count of the gate (components not touching the region's border) was 1,067,239 cells in 14,547 components.
- The 2,048-cell lattice flags a coastal strip as not ocean (a vertex is 4.9 km wide), so 1.43 M is an upper bound; the production lattice (16,384 cells, 610 m) is finer. Cells that are sea but read inland sit beside ocean cells of their own core, flood to them and gain a depth only where the bed is lower than its shallower neighbour. That depth is not drawn by the coast fade (not tested here: the fade acts on the elevation, the fill depth is added to the floor). Cost of the mask lookups per cell (a direction mapping and four reads) is not yet measured; it is a Tracked item for the planet bake.

## Impact
- Code: `generators::ocean`, `cache::layer_fillpass`, `cache::layer_fill` (kernel version), `bake_layer`; the callers in tests and the gate example pass `&|_| true`. No golden moves.
