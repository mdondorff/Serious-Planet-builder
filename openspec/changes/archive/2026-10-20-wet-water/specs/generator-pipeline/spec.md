## MODIFIED Requirements

### Requirement: GEN-023 The water-level field W
The generators SHALL provide the water-level field W(dir) (ADR 0041 point 1): at a direction, either no water or a **sample** of the surface height above the reference surface in metres (`f64`), the **water-body id** (`u64`; the sea is `WaterBodyId::SEA`, ids with the top bit set are reserved for ids derived from the labels of the bake's components) and the **liquid kind** (`u32`, a row of the world's liquid table, WDEF-006). W SHALL be made by **producers** listed in a `WaterField` over a face mapping and the bake (a `LatticeSource`), most specific first, the first that reports water at a point winning; a producer is a pure function of the query (the direction, its position in the bake's lattice from `locate_direction` and the bake interpolated there by `read_direction`) and its own data. The only producer of M5 is the **sea**: a point is sea when the interpolated continentalness is at or below `SEA_LEVEL_C` **and** at least one of the lattice vertices with a non-zero weight in ADR 0027 point 2 (the lower corner always, the others when their axis weight is not zero) is **wet** (ADR 0071): an ocean vertex (GEN-022) or one whose wet floor (GEN-031) is at most `bound_code(B(W))`, the conduction bound of ATMO-019 at the sea's level (a `SeaProducer` without a floor lattice has the ocean vertices only); the channels are categorical and are not interpolated, and `wet_at` answers the vertex rule alone (a wet vertex weighs in the point), which the coastal band reads (ATMO-019); W is the sea level of the definition (`body.<n>.sea.level_m`, constant in M5), the kind the definition's sea liquid and the body `SEA`; an inland basin below sea level and land have no water. The coast is defined by the continentalness alone, whatever the level: M5 seas are at the Earth default 0, and a sea level other than 0 is a producer parameter whose coast the water pass (item 7) decides. The set of vertices SHALL be the same nodes from either face of a shared face edge (the rule is edge-symmetric), an ocean channel or a floor lattice of another resolution than the bake SHALL be an error naming both, and a missing or damaged block of the bake or of the channel SHALL be an error naming it, never wet or dry. `plan_water(dir)` SHALL give the GPU-free plan data of a view: the level and the body and kind of the water at the direction, else the reference level of the first producer that has one (the sea's) else 0. Class: all attributes are **fields** (CON-07, ADR 0035): the level, the body id and the kind are CPU values, bit-exact for every thread count and operating system, with no GPU twin; the level of the sea equals the definition's value bit for bit. The field's version (`WATER_VERSION`) and implementation id (`cpu-water-level-v2`, `WATER_VERSION` 2 since ADR 0071) are recorded for the keys of whatever stores W later; nothing stores it in M5. The **retrofit** (ADR 0041 point 7): a synthetic lake (a basin with a fixed level inside a footprint, its own body id and a lava kind with emission) from a test-only producer SHALL go through the same `WaterField` path with no change to the sea producer: its water has its own level, body and kind, the sea's answers are the same with and without it, it never replaces sea, and the plan data follows it.

Verify: A
Status: active
Source: ADR 0041 points 1 and 7, ADR 0027 points 2 and 5, ADR 0035, owner's M5 constraints 1, 2, 3 and 5, CON-07, CON-14

#### Scenario: The sea
- **WHEN** 6,000 directions of the 128-cell bake of seed 1 are queried, and the vertices themselves
- **THEN** water is found only where the interpolated continentalness is at or below the sea level and a weighted vertex is ocean, with the sea's level, body and kind, ocean vertices are wet and land vertices dry, and the answers are the same on 1, 2 and 7 threads

#### Scenario: A dry basin
- **WHEN** a basin below sea level lies one vertex behind a wall of land, and points are queried between the ocean vertex and the wall, between the wall and the basin, and in the basin
- **THEN** the first is water (an ocean vertex carries weight), the others are dry although their continentalness is at or below sea level

#### Scenario: Edge symmetry
- **WHEN** a point on a shared face edge is expressed as a lattice position of either face
- **THEN** its weighted vertices are the same nodes

#### Scenario: Synthetic lake
- **WHEN** a lake producer with level L, body 2 and the lava kind is listed before the sea
- **THEN** its centre is water at L with body 2 and kind 1 (emitting), the same directions are dry without it, the sea's answers elsewhere are unchanged, and the plan water over the lake is its level and over dry land the sea level

#### Scenario: Over the store
- **WHEN** the field reads a stored bake and its stored ocean channel (STRM-023)
- **THEN** it gives the answers of the in-memory computation, with the sea level and liquid of the definition, and an ocean channel of another resolution is an error naming both

### Requirement: GEN-024 The coarse level of the water field W
The generators SHALL provide a coarse level of W (ADR 0058, ADR 0071), the `WaterSummary` of a bake of `res` cells per face edge in blocks of `B` cells: one cell per `d x d` bake cells, `d = min(B, 64)`, so `6 (res / d)^2` cells (face, row, column, x fastest), each inside one block. A cell holds a flag and two numbers, functions of the bake and its wet floor (GEN-031) alone (the sea level and the liquid of the definition are no input):
- SEA: some lattice cell of the summary cell has a corner whose continentalness is not above `SEA_LEVEL_C` (a NaN counts as not above), the necessary condition for any point of that lattice cell to be sea (GEN-023);
- floor: the lowest wet floor (`u16`, 0 where an ocean vertex is, 0xFFFF where the sea reaches none) over the corners of the lattice cells of the summary cell;
- band elevation: the lowest corner elevation among the lattice cells that have a corner above `SEA_LEVEL_C`, infinity when none has, the necessary condition for a band point (ATMO-019).

`summarize_block` SHALL compute the cells of one block from its samples and its floors, and a summary SHALL be the same whatever the order of the blocks, the thread count or the block size `B` (beyond `d`). A tile at or above the bake's block level `log2(res / B)` (a tile whose area is at least one block's) SHALL be answered by `WaterSummary::tile_water`; a tile of a finer level is not answered (`None`: the field is asked point by point, GEN-023). The answer is the sea (the definition's level, the body `SEA`, its liquid) when any cell the tile's lattice rectangle touches, widened by one lattice cell on the low side (a border sample can lie a rounding error below the tile's edge and weigh the vertices there), has a floor at most `bound_code(B(W))` (ATMO-019) and has the SEA flag or a band elevation at most `B(W)`; else nothing. No distance, face mapping or radius enters the rule. A `SeaProducer` with a summary SHALL answer for the tiles of its summary, a `WaterField` answers when every producer does, and a summary of another resolution than the ocean channel SHALL be an error naming both. Class (ADR 0035): a **field** (CON-07) with no GPU twin; bound: exact, bit-exact for every thread count and operating system (CON-14); a coarse answer is a superset of the point query's, so it never misses water the 25 samples find, and a parent is wet wherever its children are (ATMO-019). Known limit: a sample that the direction mapping places on the neighbouring face of a face edge can weigh a vertex of that face that no cell of the tile's face holds; the floor's edge connectivity (GEN-031) covers it except for a NaN continentalness. A summary with more than `6 * 1024^2` cells SHALL NOT be made (refused before allocating). `SUMMARY_IMPLEMENTATION_ID` is `cpu-water-summary-v2`; the version-1 summary (ocean flags, no floor) is not read.

Verify: A
Status: active
Source: ADR 0058, ADR 0071, ADR 0041 point 2, ADR 0056, GEN-022, GEN-023, GEN-031, ATMO-019, CON-07, CON-14

#### Scenario: A hand-made block
- **WHEN** a block of 2 cells is summarised with no floor reached, with floors among its corners, with a sea corner and with a NaN corner, and a block of 128 cells with one sea corner and one floor
- **THEN** the flags, floors and band elevations are the documented ones (the floor is the lowest over every corner), the 128-cell block gives 2 x 2 cells and the cell holding column 5 and row 100 has SEA and that floor, and a block of the wrong size is refused

#### Scenario: Known tiles
- **WHEN** the six faces of a one-cell-per-face summary are asked, with sea at floor 0 on one, band reached on one, band not reached on one, band behind ground one code above the bound, and a high cell
- **THEN** the sea face and the reached band face are wet, the others are dry whatever their distance, the high cell is wet only at a level whose band covers it, the sea is at the definition's level, and a tile below the block level is not answered

#### Scenario: A real bake
- **WHEN** the summary of a bake of 256 cells is made in blocks of 32, 64 and 256 cells on one and on four threads, from blocks in order and in reverse, and compared with a summary read off the whole faces cell by cell from the serial floors
- **THEN** all agree bit for bit, and the summaries of the 64 and 256 blocks (both with `d` 64) are equal

#### Scenario: Parents
- **WHEN** every tile of the levels from 1 to the block level of a real bake is asked
- **THEN** the parent of every wet tile is wet
