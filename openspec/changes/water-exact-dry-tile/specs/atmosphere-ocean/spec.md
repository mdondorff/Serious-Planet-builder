## MODIFIED Requirements

### Requirement: ATMO-012 Water tiles are chosen by a per-tile CPU query at plan time
The nodes of a frame that carry water SHALL be chosen on the CPU (CON-03) by asking the water field W (GEN-023) for the `(4 + 1)^2` sample directions of each node's tile, memoised per tile (`WaterTileCache`): a tile carries water when any sample reports water or by the coastal band (ATMO-019), and its water is its most frequent sample's body (ties: the lower body id), so a tile holds one level and one liquid; nothing is stored per tile and `MESH_VERSION` and the compact slot pins are unchanged (ADR 0051 point 6, option 1). The answer of a tile SHALL be computed in the tile's worker job (the lit path of a world with water) and kept in a `WaterBook`, which the plan's cache reads first (ADR 0056): a plan whose tiles were loaded by their jobs asks the field nothing and reads no store block on the frame thread; a tile with no entry is computed on the plan thread and counted (`queries`, `tile_misses`, and `band_queries` for the band's reads). A cancelled job computes none. The book holds one entry per tile loaded (a few bytes, never evicted; a wet entry also holds the lowest height of the tile's own mesh vertices, left by the same job, ATMO-020) and a job of a tile below the block level spawns up to eight short threads to load the tile's store blocks in parallel. A tile at or above the bake's block level (a tile whose area is at least one stored block's: levels 0 to 4 of a 16384-cell bake in blocks of 1024 cells) SHALL be answered from the coarse level of W (GEN-024) instead of its sample directions: its job reads no sample block and no ocean block, whatever its level, and the answer is the union over the cells the tile touches (a superset of the point query's, ATMO-019); a tile below the block level keeps the point query and its answers bit for bit, and reads at most 2 x 2 sample blocks. The answer is a pure function of the tile and the field, so it does not depend on the thread or the order (CON-14); an in-memory bake builds the same summary from its own fields (in blocks of at most 1024 cells, the store's layout), so both sources select by one rule; a field without a summary answers every tile point by point. A coarse answer is neither a miss nor a query: it reads nothing and is not counted. The plan SHALL carry, per water node, the terrain node's index, the level (rounded once to f32), the liquid slot and the waves' reduced phases, and the liquids' appearance (at most four per frame); an unknown liquid, more than four liquids and a level that is not a finite height within 1e5 m SHALL be refused naming the id, the numbers or the tile. A plan without water SHALL keep its text and hash; the text form of one with water SHALL round-trip exactly. Known limit of option 1: a dry basin below the level inside a tile that also holds sea is drawn flooded, and a lake smaller than the sample spacing of a tile below the block level can be missed at that tile's level (a coarse tile misses none, GEN-024). Nodes whose tile carries water are the candidates of the water node list; ATMO-020 cuts those that show no water from the camera when the request names a height field. Class: the selection is plan data (no GPU twin); the level, the body and the kind are fields read from W.

Cost of a query on the plan thread, measured ad hoc on an in-memory bake (not a performance verdict, CON-20): 3.6 us per new tile (0.14 us per field query, 25 queries per tile) for the 7,536 tiles of the scripted views at bake sizes 256 and 1024, 0.01 us per tile for a warm plan; over a stored bake a tile of level 0 to 3 of a 16384-cell bake touched up to 25 blocks (before the coarse level, GEN-024) of about 20 ms each (window 6: 8 s frames), which is why the jobs do it.

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


### Requirement: ATMO-020 The water nodes are cut by the horizon of their level and by their terrain's height
When the request names the terrain's height field, the plan SHALL leave out of the water node list a node that can show no water from the camera.
The cut SHALL act on the node list only; the tile's answer (ATMO-012), the cache's query counters and every key are unchanged.
Definitions (f64, `libm`, CON-11, CON-14):
- `R`: the body's radius; `W`: the tile's level rounded to f32; `A`: the sum of the wave amplitudes of the node's liquid; `S = A (1 + choppy)`.
- `s = (2 d)^2 / (8 (R + W))`, `d` the sample spacing of the node's level: the sag of the drawn surface under the sphere.
- `c`: the chord radius of the tile's bounding sphere on the sphere `R`; `q`: the tile's centre direction times `R + W`.
- `m`: the lowest height of the tile's own `(cells + 1)^2` mesh vertices (f32, as the vertex stage forms it from the tile's texels or vertices), the exact minimum of the drawn terrain: the mesh is linear between its vertices, the morph targets are averages of own vertices (heights inside `[m, max]`), and a skirt vertex repeats its border vertex's height, so no drawn height is below `m`.
Horizon:
- `h = |camera| - (R + W)`; with `h` not above 0 (camera at or under the level, a lake above the camera) the node SHALL be kept.
- Otherwise the node SHALL be cut when `|camera - q| - c (R + W) / R - S > sqrt(2 (R + W - s) (h + s) + (h + s)^2) + sqrt(2 (R + W - s) (A + s) + (A + s)^2)`.
Height:
- The node SHALL be cut when the lower height bound of its tile is at or above `W + A + 4 eps_f32 max(1, |W|, |bound|)`.
- The bound is `HeightField::height_bounds` at the tile's centre direction and angular radius, the values of the frustum cut; the morph's heights lie inside it.
- The node SHALL also be cut when `m` is at or above `W + A + 4 eps_f32 max(1, |W|, |m|)`.
- `m` SHALL be computed in the tile's worker job from the mesh it built (the vertex format's vertices, the compact format's lowest texel; a pre-filled tile's likewise) and kept in the `WaterBook` beside a wet entry of the tile (an f32 of RAM, not stored, `MESH_VERSION` and every key unchanged); it is a pure function of the tile and the field (CON-14). A cancelled job and a tile whose job did not run (a root, a tile not yet loaded) hold none, and the plan SHALL read it from the book without computing it and keep the node by the loose bound's verdict until the book holds it.
Result:
- The cut SHALL be a pure function of the camera, the tile, `W`, the look and the book's content: the same for every thread count, cache state and order of questions.
- The kept nodes SHALL be the uncut plan's nodes bit for bit, in the same order; a plan whose nodes are all cut is a water plan with no node.
- The CPU-ray count of sea pixels inside water nodes SHALL be equal for the cut and the uncut plan, and the class view and the lit view pixel-identical.
- The cache SHALL count the nodes cut by the horizon (`cut_horizon`), by the height tests (`cut_height`) and, of those, by `m` (`cut_exact`); recorded only.

Class (ADR 0035): plan data (no GPU twin, no stored data). Bound: exact; `m` is the minimum of the f32 heights the vertex stage reads, so the f32 rounding of the vertex stage's `h_origin + hr` and of the water pass's `W - h` is inside the 4 eps margin.
Known limit: the blocking surface is the sphere `R + W` less `s`, so dry ground below `W` between the camera and the tile (a valley under a lake, a dry basin below the sea) can hide nothing and the tile is cut where it is visible; the waves' troughs are taken to be hidden by their crests; skirts add `skirt depth^2 / (2 c)` to a tile's reach when `S` is 0. The terrain between the mesh's vertices is the mesh's, not the field's: a field detail below the lattice spacing that dips under `W` between dry vertices is not drawn and not covered.

Verify: A, B, C
Status: active
Source: ADR 0056, ADR 0058, ATMO-012, ATMO-016, ATMO-019, docs/perf/water-attribution.md, CON-11

#### Scenario: Exact minimum at its bound
- **WHEN** a tile's job left a minimum at the first f32 at or above the bound, one f32 below it, and far below it, for levels from -140 m to 9,000 m and a camera at 1 m
- **THEN** the node is cut at the bound and kept one f32 below it, every other node of the plan is the loose cut's bit for bit, and `cut_exact` counts the cut

#### Scenario: Minimum from a real job
- **WHEN** the jobs of wet and dry tiles run in both tile formats over a world with water
- **THEN** a wet tile's minimum equals the minimum recomputed from the mesh it built, a dry or cancelled tile has none, and the minimum of tiles of levels 3 to 20 bounds every vertex height, morphed height and skirt vertex height of the tile, in the vertex and compact formats over the reference and the steep field

#### Scenario: Pictures
- **WHEN** an aerial, a low, a beach and an inland view of the 1,024-cell world are drawn in the class view and the lit view with the full selection, the loose cut and the exact cut
- **THEN** the HDR frames are equal, and the exact cut keeps fewer nodes than the loose cut over the four views together (1,159 to 657 at the beach, 1,470 to 322 inland)
