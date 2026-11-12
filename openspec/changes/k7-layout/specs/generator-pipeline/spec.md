## ADDED Requirements
### Requirement: GEN-182 K7's tile layout is part of its definition
K7 SHALL be defined over a fixed tile grid of each cube face (ADR 0119): the face's `n x n` cell lattice cut into square tiles of `tile` cells an edge, tile `(ti, tj)` covering the cells `[ti x tile, (ti + 1) x tile) x [tj x tile, (tj + 1) x tile)` (the grid origin is the face's cell `(0, 0)`), each solved in a window of its core plus a halo of `halo` cells on every side. The layout SHALL be a pure function of the face lattice `n`, of `k` and of the profile, never of the request order, the region asked for, the thread count or the schedule:
- `tile` divides `n`, is a multiple of the base stride (4) and is more than twice `halo`; `halo` is at most `n / 8` and, for a tile with channels, at least `DISC_RADIUS_MAX` plus the smallest profile carry (GEN-169), so that the halo holds the trench disc and the profile's carry; K7 SHALL refuse a layout that breaks any of these, naming the parameter;
- the layout's values (current value: `TileGrid` and `K7Params` as built; 128 halo cells at the decision) SHALL be inputs of K7's key, with the width law and the flood rules; a different layout is a different generator definition, a new key and no reuse of results; a change of tile size or halo is a generator change with a re-bake, never a free setting;
- for one layout, input set and version, K7's result SHALL be bit-identical under any thread count, schedule, order and worker assignment (CON-14, CON-30), and no tile's result SHALL depend on which other tiles were asked for.

At a region edge: a region SHALL be the union of whole tiles of the canonical grid, and K7's result for a tile SHALL be the result the same tile has in any other region or in the planet; a region edge adds no tile border, no halo of its own and no outlet. At a coast: cells at or below the sea level (a key input) are the outlets of a window, are neither lowered nor carry (GEN-169, GEN-176), and a window that holds sea runs the same rules as one that does not. At the window's border: the border is an outlet of the window's flood and the cells within `halo` of it are not part of the tile's result. At a cube-face edge: the halo past the face edge is the extended plane of the face (as built); how the tiles of two faces agree there is NOT yet defined and SHALL be settled with the face-seam work (REND-063) before any planet bake; the architect decides whether K7 results at face edges are canonical before then.

K7 is not validated against an unpartitioned reference (ADR 0119 point 4): CON-08's partition-invariance and border-agreement checks are replaced, for K7, by the tier B execution test (TEST-130) and the seam gate G-SEAM.

Verify: B
Status: planned
Source: ADR 0119 (owner, 2026-11-12); architect D119 item 1

#### Scenario: One layout, many schedules
- **WHEN** K7 runs on one layout with 1, 3 and 8 threads and two schedules
- **THEN** every tile's result is the same bits

#### Scenario: A region
- **WHEN** a region is baked for a set of whole tiles and the same tiles are baked as part of a larger region
- **THEN** the results of the shared tiles are the same bits

#### Scenario: A different layout
- **WHEN** the halo or the tile size changes
- **THEN** the key changes and no stored result of the other layout is reused
