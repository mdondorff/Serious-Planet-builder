## ADDED Requirements

### Requirement: GEN-024 The coarse level of the water field W
The generators SHALL provide a coarse level of W (ADR 0058), the `WaterSummary` of a bake of `res` cells per face edge in blocks of `B` cells: one cell per `d x d` bake cells, `d = min(B, 64)`, so `6 (res / d)^2` cells (face, row, column, x fastest), each inside one block. A cell holds two flags and a number, functions of the bake and its ocean channel alone (the sea level and the liquid of the definition are no input):
- WET: some lattice cell of the summary cell has an ocean vertex among its four corners and a corner whose continentalness is not above `SEA_LEVEL_C` (a NaN counts as not above), the necessary condition for any point of that lattice cell to be sea (GEN-023);
- OCEAN: some vertex of the summary cell is ocean;
- band elevation: the lowest corner elevation among the lattice cells that have a corner above `SEA_LEVEL_C`, infinity when none has, the necessary condition for a band point (ATMO-019).

`summarize_block` SHALL compute the cells of one block from its samples and ocean bits, and a summary SHALL be the same whatever the order of the blocks, the thread count or the block size `B` (beyond `d`). A tile at or above the bake's block level `log2(res / B)` (a tile whose area is at least one block's) SHALL be answered by `WaterSummary::tile_water`; a tile of a finer level is not answered (`None`: the field is asked point by point, GEN-023). The answer is the sea (the definition's level, the body `SEA`, its liquid) when any cell the tile's lattice rectangle touches, widened by one lattice cell on the low side (a border sample can lie a rounding error below the tile's edge and weigh the vertices there), is WET; else, when any such cell has a band elevation at most `B(W)` (ATMO-019) and a cell with the OCEAN flag has its centre within `100 km + 0.82` of the tile's edge `+ 1` summary cell of the tile's centre (0.82 is above `2/sqrt(6)`, the least with which a parent's reach contains a child's at every level, including beside a cube corner) (the angle counted on the sphere, one `libm` cosine); else nothing. A `SeaProducer` with a summary SHALL answer for the tiles of its summary, a `WaterField` answers when every producer does, and a summary of another resolution than the ocean channel SHALL be an error naming both. Class (ADR 0035): a **field** (CON-07) with no GPU twin; bound: exact, bit-exact for every thread count and operating system (CON-14); a coarse answer is a superset of the point query's, so it never misses water the 25 samples find, and a parent is wet wherever its children are (ATMO-019). Known limit: a sample that the direction mapping places on the neighbouring face of a face edge can weigh a vertex of that face that no cell of the tile's face holds; the ocean channel's edge connectivity covers it except for a NaN continentalness. A summary with more than `6 * 1024^2` cells SHALL NOT be made (refused before allocating).

Verify: A
Status: active
Source: ADR 0058, ADR 0041 point 2, ADR 0056, GEN-022, GEN-023, ATMO-019, CON-07, CON-14

#### Scenario: A hand-made block
- **WHEN** a block of 2 cells is summarised with no ocean vertex, with an ocean vertex among land corners, with an ocean vertex beside a sea corner, with the sea corner in another lattice cell, and with a NaN corner, and a block of 128 cells with one ocean vertex and one sea corner
- **THEN** the flags and band elevations are the documented ones, the 128-cell block gives 2 x 2 cells and the cell holding column 5 and row 100 is WET and OCEAN

#### Scenario: Known tiles
- **WHEN** the six faces of a one-cell-per-face summary are asked, with sea on one, band on two and a high cell on a fourth
- **THEN** the sea face is wet, the band face next to it is wet by the reach, the band face opposite it is dry, the high cell is dry, a face with no cell is dry, the band rises with the sea level, the sea is at the definition's level, and a tile below the block level is not answered

#### Scenario: A real bake
- **WHEN** the summary of a bake of 256 cells is made in blocks of 32, 64 and 256 cells on one and on four threads, from blocks in order and in reverse, and compared with a summary read off the whole faces cell by cell
- **THEN** all agree bit for bit, and the summaries of the 64 and 256 blocks (both with `d` 64) are equal
