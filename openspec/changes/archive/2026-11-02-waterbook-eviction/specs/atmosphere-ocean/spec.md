## MODIFIED Requirements

### Requirement: ATMO-012 Water tiles are chosen by a per-tile CPU query at plan time
The nodes of a frame that carry water SHALL be chosen on the CPU (CON-03) by asking the water field W (GEN-023) for the `(4 + 1)^2` sample directions of each node's tile, memoised per tile (`WaterTileCache`): a tile carries water when any sample reports water or by the coastal band (ATMO-019), and its water is its most frequent sample's body (ties: the lower body id), so a tile holds one level and one liquid; nothing is stored per tile and `MESH_VERSION` and the compact slot pins are unchanged (ADR 0051 point 6, option 1). The answer of a tile SHALL be computed in the tile's worker job (the lit path of a world with water) and kept in a `WaterBook`, which the plan's cache reads first (ADR 0056): a plan whose tiles were loaded by their jobs asks the field nothing and reads no store block on the frame thread; a tile with no entry is computed on the plan thread and counted (`queries`, `tile_misses`, and `band_queries` for the band's reads). A cancelled job computes none. The book holds one entry per tile loaded (bounded by `WATER_BOOK_CAP`; a wet entry also holds the lowest height of the tile's own mesh vertices, left by the same job, ATMO-020) and a job of a tile below the block level spawns up to eight short threads to load the tile's store blocks in parallel. A tile at or above the bake's block level (a tile whose area is at least one stored block's: levels 0 to 4 of a 16384-cell bake in blocks of 1024 cells) SHALL be answered from the coarse level of W (GEN-024) instead of its sample directions: its job reads no sample block, no ocean block and no floor block, whatever its level, and the answer is the union over the cells the tile touches (a superset of the point query's, ATMO-019); a tile below the block level keeps the point query and its answers bit for bit, and reads at most 2 x 2 sample blocks. The answer is a pure function of the tile and the field, so it does not depend on the thread or the order (CON-14); an in-memory bake builds the same summary from its own fields (in blocks of at most 1024 cells, the store's layout), so both sources select by one rule; a field without a summary answers every tile point by point. A coarse answer is neither a miss nor a query: it reads nothing and is not counted. The plan SHALL carry, per water node, the terrain node's index, the level (rounded once to f32), the liquid slot and the waves' reduced phases, and the liquids' appearance (at most four per frame); an unknown liquid, more than four liquids and a level that is not a finite height within 1e5 m SHALL be refused naming the id, the numbers or the tile. A plan without water SHALL keep its text and hash; the text form of one with water SHALL round-trip exactly. Known limit of option 1: a dry basin below the level inside a tile that also holds sea is drawn flooded, and a lake smaller than the sample spacing of a tile below the block level can be missed at that tile's level (a coarse tile misses none, GEN-024). Nodes whose tile carries water are the candidates of the water node list; ATMO-020 cuts those that show no water from the camera when the request names a height field. Class: the selection is plan data (no GPU twin); the level, the body and the kind are fields read from W.

Cost of a query on the plan thread, measured ad hoc before ADR 0071 (the band now costs a `wet_at` per band sample and no probe grid) on an in-memory bake (not a performance verdict, CON-20): 3.6 us per new tile (0.14 us per field query, 25 queries per tile) for the 7,536 tiles of the scripted views at bake sizes 256 and 1024, 0.01 us per tile for a warm plan; over a stored bake a tile of level 0 to 3 of a 16384-cell bake touched up to 25 blocks (before the coarse level, GEN-024) of about 20 ms each (window 6: 8 s frames), which is why the jobs do it.

Verify: A
Status: active
Source: ADR 0041 point 8, ADR 0051 point 6, ADR 0058, CON-03, CON-04

#### Scenario: Selection
- **WHEN** a plan of a coastal view is made over the reference sea
- **THEN** the water nodes are exactly the nodes with a wet sample (less those cut under ATMO-020, none without a height field), the first plan asks 25 queries per tile and a second plan asks none and is identical, and a later time moves the phases and nothing else

#### Scenario: The book is bounded
- **WHEN** a book of 32 tiles is filled by a synthetic flight of 600 tiles while one tile is asked for every step, and a wet tile with a minimum height is evicted by the flight and filled again
- **THEN** the book never holds more than 32 tiles (the default is 262,144, about 26 MB), the least recently used eighth goes past the cap, the tile in use is never evicted, the evicted entry and its minimum are gone, and the second answer equals the first to the bit

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

#### Scenario: Known limits of the cache (ADR 0053)
- **WHEN** the field or the sea level changes within a run
- **THEN** `WaterTileCache`, which is keyed by tile only and never shrinks, needs `clear`
