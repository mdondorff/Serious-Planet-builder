## ADDED Requirements
### Requirement: GEN-121 The fill level is the planet-wide spill level of the final surface
The fill depth of GEN-120 SHALL be a function of the whole surface and not of a tile's window: the fill level of a cell is its planet-wide spill level `L(c)`, the lowest value of the highest height along a path from the cell to an outlet (the sea), and the depth is `L(c) - z(c)`. The level SHALL be computed in the form of Barnes et al.'s parallel priority-flood (2016), minimax, with no store and no base:
- stage 1, per tile: a priority flood from the tile's outlets and from every non-outlet cell of its border (each a seed with its own label, the index along the perimeter), in order of height with ties by the cell index; every cell takes the label of the seed that reaches it first and `level`, the highest height on its path from that seed; the tile returns the territories' adjacency, `(a, b, w)` with `w` the lowest `max(level, level)` over the neighbouring cell pairs of two territories (8 neighbours), sorted;
- stage 2: a graph whose nodes are the sea and the seeds of every tile; edges are the tiles' territory edges and the pairs of neighbouring border cells of two tiles with `w = max(z, z)`; the caller names the pairs, so a cube edge, a corner and a rotated gluing are pair lists like a planar seam; the planet-wide level `G` of a seed is the lowest highest-edge over the paths from the sea (Dijkstra with `max` for the sum; `+inf` where no path reaches);
- stage 3: the level of a cell is `max(level(c), G(label(c)))`, and `level(c)` for a cell reached from an outlet of its own tile;
- the result SHALL equal the minimax level of the whole surface bit for bit (the tests' oracle is a global flood over the same neighbour lists), and the same bits on every run (the kernels are single-threaded with a fixed order and no thread-count input, CON-14); heights that are not finite SHALL be refused.
The level does not change GEN-100's routing (a face does not route across its cube edges): the fill level is not a route, and the pairs the caller names are where basins join. GEN-121 replaced GEN-120's pass-2 tile rule (the depth below the window's flood); the depth of GEN-120 is taken against the exact base and clamped at 0, and the kernel's `L(c) - z(c)` is against the heights it is given. The code's cap and its step stay with the caller (STRM-077); the level has no epsilon and no minimum depth: a dent of any size is a pit.
The kernel holds no store, no tile geometry and no quantisation: The post-pass (STRM-077) feeds it the tiles' final heights (f32, exact).

Verify: A
Status: active
Source: Owner and planner 2026-11-05 (inbox #67: the keep-pits preview's seam failure; option (a), the post-pass); SPIKE-102 section 6; GEN-120; Barnes, Lehman and Mulla 2016 (parallel priority-flood)

#### Scenario: The tiled level equals the global flood
- **WHEN** 12 random terrains of 3 x 3 tiles of 10 cells with basins across tile edges and tile corners, a bowl of a flat 60 m floor across three tiles with a notch at 70 m to the sea, two tiles glued with a rotation (the east column of one against the top row of the other, 8-connected) with a bowl centred on the glued corner, nested basins on a plateau of equal heights, a tile of sea and a world with no outlet are levelled tile by tile and by one global flood
- **THEN** every cell's level is the same bits in both, the wide bowl's three tiles all read 70 m, a world with no outlet reads `+inf` at every cell, the same input gives the same bits and a height that is not finite is refused; a border cell that is an outlet is the sea through `seed_of`, an edge weight that is not finite is refused, and -0.0 and 0.0 are one height

#### Scenario: The perimeter
- **WHEN** the perimeter index of every border cell of tiles of 3 x 3 to 10 x 12 cells is taken and inverted
- **THEN** each border cell has one index, an inner cell has none, and the inverse returns the cell
