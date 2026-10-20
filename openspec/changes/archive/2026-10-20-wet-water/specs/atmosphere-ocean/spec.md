## MODIFIED Requirements

### Requirement: ATMO-012 Water tiles are chosen by a per-tile CPU query at plan time
The nodes of a frame that carry water SHALL be chosen on the CPU (CON-03) by asking the water field W (GEN-023) for the `(4 + 1)^2` sample directions of each node's tile, memoised per tile (`WaterTileCache`): a tile carries water when any sample reports water or by the coastal band (ATMO-019), and its water is its most frequent sample's body (ties: the lower body id), so a tile holds one level and one liquid; nothing is stored per tile and `MESH_VERSION` and the compact slot pins are unchanged (ADR 0051 point 6, option 1). The answer of a tile SHALL be computed in the tile's worker job (the lit path of a world with water) and kept in a `WaterBook`, which the plan's cache reads first (ADR 0056): a plan whose tiles were loaded by their jobs asks the field nothing and reads no store block on the frame thread; a tile with no entry is computed on the plan thread and counted (`queries`, `tile_misses`, and `band_queries` for the band's reads). A cancelled job computes none. The book holds one entry per tile loaded (a few bytes, never evicted; a wet entry also holds the lowest height of the tile's own mesh vertices, left by the same job, ATMO-020) and a job of a tile below the block level spawns up to eight short threads to load the tile's store blocks in parallel. A tile at or above the bake's block level (a tile whose area is at least one stored block's: levels 0 to 4 of a 16384-cell bake in blocks of 1024 cells) SHALL be answered from the coarse level of W (GEN-024) instead of its sample directions: its job reads no sample block, no ocean block and no floor block, whatever its level, and the answer is the union over the cells the tile touches (a superset of the point query's, ATMO-019); a tile below the block level keeps the point query and its answers bit for bit, and reads at most 2 x 2 sample blocks. The answer is a pure function of the tile and the field, so it does not depend on the thread or the order (CON-14); an in-memory bake builds the same summary from its own fields (in blocks of at most 1024 cells, the store's layout), so both sources select by one rule; a field without a summary answers every tile point by point. A coarse answer is neither a miss nor a query: it reads nothing and is not counted. The plan SHALL carry, per water node, the terrain node's index, the level (rounded once to f32), the liquid slot and the waves' reduced phases, and the liquids' appearance (at most four per frame); an unknown liquid, more than four liquids and a level that is not a finite height within 1e5 m SHALL be refused naming the id, the numbers or the tile. A plan without water SHALL keep its text and hash; the text form of one with water SHALL round-trip exactly. Known limit of option 1: a dry basin below the level inside a tile that also holds sea is drawn flooded, and a lake smaller than the sample spacing of a tile below the block level can be missed at that tile's level (a coarse tile misses none, GEN-024). Nodes whose tile carries water are the candidates of the water node list; ATMO-020 cuts those that show no water from the camera when the request names a height field. Class: the selection is plan data (no GPU twin); the level, the body and the kind are fields read from W.

Cost of a query on the plan thread, measured ad hoc before ADR 0071 (the band now costs a `wet_at` per band sample and no probe grid) on an in-memory bake (not a performance verdict, CON-20): 3.6 us per new tile (0.14 us per field query, 25 queries per tile) for the 7,536 tiles of the scripted views at bake sizes 256 and 1024, 0.01 us per tile for a warm plan; over a stored bake a tile of level 0 to 3 of a 16384-cell bake touched up to 25 blocks (before the coarse level, GEN-024) of about 20 ms each (window 6: 8 s frames), which is why the jobs do it.

Verify: A
Status: active
Source: ADR 0041 point 8, ADR 0051 point 6, ADR 0058, CON-03, CON-04

#### Scenario: Selection
- **WHEN** a plan of a coastal view is made over the reference sea
- **THEN** the water nodes are exactly the nodes with a wet sample (less those cut under ATMO-020, none without a height field), the first plan asks 25 queries per tile and a second plan asks none and is identical, and a later time moves the phases and nothing else

#### Scenario: Tile jobs fill the book
- **WHEN** the jobs of a set of tiles ran over a world with water on the lit path, and the plan's cache then answers the same tiles
- **THEN** it makes no field query and no miss (every tile is a book hit), the answers equal those of a cache with no book, a cancelled job leaves no entry, and a world not on the lit path computes none

#### Scenario: Coarse tile jobs read no block
- **WHEN** the jobs of all 510 tiles of levels 0 to 3 of a stored bake whose blocks are level-3 tiles run on one and on four threads, and then the jobs of tiles of level 4
- **THEN** the sample blocks and ocean blocks decoded by the coarse jobs number 0, the answers are equal on one and on four threads, the sea is the definition's, and a sample of level-4 jobs reads at most 4 sample blocks each

#### Scenario: Two bodies, a missing liquid
- **WHEN** a tile's samples hold two bodies, a liquid the table does not define, five liquids, or a level of 6,000 km
- **THEN** the tile takes the more frequent body (a tie the lower id), and the others are refused naming the liquid id, the numbers or the tile

#### Scenario: Text form
- **WHEN** a plan with water is written to text and read back
- **THEN** it equals the original, a plan without water has no water line, and a damaged line is refused

### Requirement: ATMO-019 The coastal band: the sea covers the terrain below the level that the ocean reaches
A tile with no sea of its own SHALL carry the sea when one of its 25 samples is band and a vertex that weighs in that sample's interpolation (GEN-023) is wet (ADR 0071). A sample is band when the bake calls it land (continentalness above `SEA_LEVEL_C`) and its bake elevation is at most `B(W)`, the highest elevation at which flat ground can still be cut below the level: the fixed point of `E = W + 0.75 A(E)`, `A(E) = (80 + 900 smoothstep(-200, 300, E)) x 0.15` (the erosion's amplitudes at the carve floor of flat ground, 0.75 the lower bound of the crease sum); `B(0)` is about 64 m (63.9), and `B` rises with `W` faster than `W`. Steep ground (slope factor above the floor) is not covered and can stay dry where it dips below the level. A vertex is wet when it is an ocean vertex or its wet floor (GEN-031) is at most the code of `B(W)`, the conduction bound `C(W)`; the point query and the summary make this one comparison on the stored floor (rounded down, so a superset of the true wet set). So the sea covers the band ground that is connected to the ocean through ground at most `B(W)` high, over any distance, and does not cover ground behind a ridge higher than `B(W)`, however near; steep ground that is cut below the level at a higher bake elevation does not conduct (the limit above). `SeaProducer::water_at` reports water for a sample that is sea by its continentalness (GEN-023) when a weighted vertex is ocean or wet. A tile at or above the bake's block level (GEN-024) has no per-sample query: it carries the sea when a cell it touches has a lowest floor at most the code of `B(W)` and may hold sea (a corner not above `SEA_LEVEL_C`) or band (a land corner and a lowest corner elevation at most `B(W)`); such an answer is a superset of the point query's, a parent is wet wherever its children are (tested over real bakes for every tile of levels 0 to 4, the tiles below the block level against their own point-query answers), and a tile below the block level keeps its exact answer. The wet tiles only grow with `W`. An inland basin enclosed on every side by ground above `B(W)` has no wet vertex and stays dry (owner constraint 2); a lagoon behind a barrier at most `B(W)` high is wet. The pass floods per vertex where `W - h_morph > 0` (ATMO-004). The coast views of the lit scenes and the capture SHALL stand at a beach found by the terrain's height (sea 2 km out with the plan carrying the sea and the terrain at least 5 m under the level, land 500 m in at least 3 m over it, the crossing of the level bisected, `find_coast`). Class: plan data (no GPU twin), a function of the bake, the level and the tile (CON-14).

Verify: A, C
Status: active
Source: ADR 0056, ADR 0058, ADR 0041 point 2, ADR 0053, window 6 captures

#### Scenario: Coverage oracle on a real world
- **WHEN** an aerial (2 km up) and a low (30 m) view from the sea towards a coastal plain below the level, and a view at a beach, of the in-memory eroded world of 1,024 cells are drawn in the class view, and every pixel whose CPU ray meets the sea sphere where the terrain is under -12 m and a wet vertex weighs in the point is compared
- **THEN** at most 1 % of them are not water (measured 13,121 of 15,458 on the selection of ADR 0053, 0 of 15,458 aerial and 0 of 2,016 at the beach on the wet floor of ADR 0071; the inland view stands inland of the plain coast), and at most 1 % of the pixels over terrain above +12 m are water

#### Scenario: Coarse tiles
- **WHEN** every tile of levels 0 to 5 of three real bakes of 256 cells in blocks of 32 (the block level is 3) is asked by the point query and by the coarse level
- **THEN** every tile of levels 0 to 3 that the point query finds wet is wet by the coarse level, every wet tile of levels 1 to 4 has a wet parent, and every tile of levels 4 and 5 has the same answer by both

#### Scenario: Band, basin and level
- **WHEN** tiles next to an ocean, beyond the band, in a basin with no ocean vertex and behind a ridge are asked, and the level rises
- **THEN** the first carries the sea at the sea's level, the others are dry (the one behind the ridge although it is nearer than the old reach), the set of wet tiles only grows with the level, and the answer does not depend on the order of the questions

#### Scenario: Beyond the old reach
- **WHEN** the three tiles of a plain under the sea level, connected to the ocean through band ground for 120 to 132 km, in the editor's default world (seed 1, 256 cells) are asked
- **THEN** all three carry the sea

#### Scenario: The dip
- **WHEN** the dip is computed
- **THEN** `B(0)` is 63.9 m within 0.5, and for levels from -300 m to 3,000 m `B` is a fixed point (the cut at `B` reaches the level), ground just above it stays above the level, and `B` rises with the level

#### Scenario: Beach
- **WHEN** a world has a plain below the level east of its sea and a beach further on
- **THEN** the coast is at the beach, where the terrain crosses the level, and the plain's edge is no coast where the sea does not reach 2 km out
