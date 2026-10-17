## MODIFIED Requirements

### Requirement: GEN-003 Cross-OS tile hashes
A determinism manifest SHALL list output hashes for a committed set of tiles (height and mesh, every face, cube corners, levels 0 to 28) and reference-surface round trips, plus the generator, hash, mesh and key-format versions and the height and mesh keys of the same tiles for the planet and for a moon built through the generator. It SHALL also list the global bake at 64 cells per face edge (the whole-grid hash and the hash of every block of 16 cells at level 2), the raw `f64` height of the terrain field (`cpu-terrain-v1`) and of the eroded field (`cpu-erosion-v1`) at twelve face-coordinate points on every face (the four cube corners, the four edge midpoints, the centre and three further points on and near edges and corners), the component table and the channel of the ocean connectivity of that bake (`oceantable`, `oceangrid`, GEN-022), the versions and implementation ids of the bake and both fields, and the persistent known answers of ADR 0018 (keys, node, body and query hashes, the bake's stage and block keys, an ocean block's key and the node set id; written by `app`, ADR 0023 point 9). A one-ulp change in the value behind any of the new entries SHALL change that entry. CI SHALL write it on Windows and Linux and fail when the two differ, naming every differing entry.

Verify: A
Status: active
Source: CON-14, ADR 0010, ADR 0023

#### Scenario: CI comparison
- **WHEN** the determinism job diffs the Windows and Linux manifests
- **THEN** they are equal, and a difference names the kind, the tile or point and both values

#### Scenario: Mutation of a new entry
- **WHEN** the value behind any bake, block, terrain or eroded entry is changed by one ulp
- **THEN** exactly that line of the manifest changes

#### Scenario: Different builds
- **WHEN** the manifests carry different version lines
- **THEN** the report lists the version lines first and says the manifests come from different builds

### Requirement: GEN-021 Climate field of a tile
A tile SHALL carry its climate (ADR 0039 point 2): the temperature in degrees C and the precipitation in mm per year at 9 x 9 points, texel `(a, b)` at tile sample `(a, b)` of a tile grid of 8 cells, two f16 values per texel, 324 bytes per tile, in both tile formats (the compact tile's spare channel, GEN-019; the vertex tile's own 324-byte block, STRM-021). It is a **field** (ADR 0035 point 1): it is produced by a generator, stored and keyed with the tile and read by the renderer, so it has its own implementation id (`cpu-climate-v1`) and version, and one implementation per cache with the CPU canonical (CON-07). Its values come from the global bake read with the integer reader of ADR 0027 (`read_tile_sample`, by every field over a bake: the terrain field and the eroded field; a wrapper of a field, such as the streamer's cancellable one, delegates `climate`, `climate_version_word` and `heights_and_climate` so a tile built through it has the key and the climate of the inner field; lattice position from integers, face-edge-symmetric interpolation, `READER_VERSION`); a field without a bake (the reference generator, synthetic fields) takes the analytic global-coarse function of its seed at the same directions, a pure function of `(seed, mapping, tile)`. Both producers measure latitude in COORD's axes: +z is the north pole, so at +z the climate is that of latitude 90, at -z that of latitude -90 and at +x and +y that of the equator (the analytic function once took the latitude from the y component, which put its poles at +-y; fixed with `GLOBAL_VERSION` 3, whose bump re-keys every tile through the version word). The stored value SHALL be the f16 rounding (round to nearest, ties to even) of the reader's f32 value, computed with integer arithmetic and `libm` only, so the CPU result is the definition of the field, the implementation id `cpu-climate-v1` is part of the version word, the GPU decode (`unpack2x16float`) is exact and CON-14's CPU/GPU bound for this field is zero; against the bake the rounding costs at most 2^-11 of the value (declared quantisation: half a step, at most 0.008 degrees below 16 C and 0.016 below 32 C, 2 mm of precipitation below 2,048 mm and 4 mm below 4,096 mm; the field's range is 0 to 4,500 mm), a deviation from ADR 0039's "abs 0.01 degrees and 0.1 mm" that f16 cannot meet for large values, recorded here. The version word that enters every mesh key and persistent mesh key (`PARAM_CLIMATE_VERSION`) SHALL cover the climate version, `READER_VERSION` and `GLOBAL_VERSION`. Texels SHALL be bit-identical across runs, thread counts and operating systems, with known answers, neighbouring tiles SHALL share their border texels bit for bit (also across face edges), and an unreadable block SHALL be an error naming it (recorded like a height's, never a default).

Verify: A, B
Status: active
Source: ADR 0039 point 2, ADR 0027, ADR 0035, ADR 0033, CON-07, CON-14, CON-05

#### Scenario: f16 rounding
- **WHEN** every finite f16 is decoded and re-encoded, and values at ties, at the largest finite value, at the subnormal range and beyond it are encoded
- **THEN** every round trip is exact, ties go to even, overflow gives infinity and the error of a normal value is at most 2^-11 relative

#### Scenario: Values from the bake
- **WHEN** the climate of tiles at several levels and faces is built over a bake and compared with `read_tile_sample` at the 81 positions
- **THEN** each texel is the f16 rounding of the reader's temperature and precipitation

#### Scenario: Shared borders
- **WHEN** neighbouring tiles of one face and tiles of different faces that share a border are built
- **THEN** texels at bit-equal directions are bit-equal

#### Scenario: Determinism
- **WHEN** the climate of 24 tiles is built on 1, 5 and 24 threads
- **THEN** the content hashes are equal to each other and to the committed known answers, and the version word equals its committed value

#### Scenario: Storage
- **WHEN** a compact tile and a vertex tile of one field and tile are built
- **THEN** both carry the same climate, the compact tile's texels are still 8,712 bytes, and a field with another climate version word has another mesh key

#### Scenario: Fields over a bake
- **WHEN** the terrain field and the eroded field are built over a bake, and a tile is built through the streamer's cancellable wrapper of each
- **THEN** their climate is the reader's (not the analytic function's), their version word is the bake's, and the wrapped tile equals the unwrapped one including its key and climate, in both formats

#### Scenario: Interpolation
- **WHEN** the texture is sampled at texel positions and between them
- **THEN** a texel position gives the texel, a midpoint the mean, and a vertex at a texel position takes the texel's value

#### Scenario: Latitude axis
- **WHEN** the climate texture is built (analytically and over a bake) for the tile at the +z pole, the tile at the -z pole and a tile on the equator at +x
- **THEN** the temperature texels at both poles are at least 30 degrees C colder than the equator's, and the version word names the current `GLOBAL_VERSION`

## ADDED Requirements

### Requirement: GEN-022 Ocean connectivity in the global bake
The global stage SHALL provide an **ocean-connectivity channel** (ADR 0041 point 2): one bit per vertex of the bake, set where the vertex belongs to the ocean. A vertex is **sea** when its stored continentalness satisfies `f64::from(continent) <= SEA_LEVEL_C` (the complement of the land rule); the sea vertices form a graph under 4-connectivity on the six cube-face grids in which the copies of a vertex on a shared face edge are **one node** (two copies on an edge, three at each of the 8 cube corners, found by exact integer cube points, never by floating point); its connected components are labelled by their smallest node id (a node's id is the smallest raw vertex id `face (res + 1)^2 + j (res + 1) + i` among its copies; future lake ids derive from the label); the **area** of a component is the sum over the copies of its nodes of the solid angle of the dual cell of the vertex clipped to its face (a half cell on an edge, a quarter at a corner; the closed form of a rectangle of the tangent plane, `libm::atan`), in fixed point at 2^-48 of the unit sphere, so that sums are integers and the six faces add up to the sphere to the rounding of each vertex; a component is **ocean** when its area reaches the **area threshold**, a fraction of the body's surface that is body data (`body.<n>.sea.min_area_fraction`, WDEF-006; the Earth default is 1e6 km^2, about 0.196 %), truncated to fixed point; every other sea component, an inland basin below sea level, is dry. The components SHALL be found by a **tiled union-find with one tile per bake block, during the bake**, never by recursion: each block is analysed while it is in memory (a scanline union-find with minimal roots; the nodes and the area of the vertices the block owns, a partition of every face in which a shared border vertex is counted once; the unions across the block's border as one record per maximal run of sea along a side and one per sea corner), the reports are merged in a global union-find over node ids with minimal roots, and a second pass labels each block again and sets the bits. The result (the table of components with label, nodes, area and ocean flag, and every bit) SHALL be bit-identical for every thread count, block size, block order and operating system (CON-14: a set function of the bake, integer sums, IEEE `+ - * /`, `sqrt` and `libm::atan`), and equal to a serial explicit-queue fill over the node graph (the CPU reference, which shares only the vertex-area formula, itself tested against the sphere and against the geometric area of a mid-face cell). Memory SHALL be bounded by the bake's batch: about 10 bytes per vertex beyond a block's samples while it is analysed, and one record per local component and per border run in the merger. Class: **field** (categorical, one bit per vertex, never interpolated); bound: exact, no tolerance (CPU only; there is no GPU twin). A bake with this channel is `GLOBAL_VERSION` 4; its known answers (the table hash and the channel hash of a small bake) SHALL be pinned and equal on every platform and in the determinism manifest (GEN-003). The area formula, the sea rule and the threshold are part of the version: a change of any of them bumps it.

Verify: A
Status: active
Source: ADR 0041 point 2, SPIKE-08, ADR 0026, ADR 0027, ADR 0035, CON-07, CON-14

#### Scenario: Against the serial reference
- **WHEN** real bakes (seeds 1 and 7 at 64 and 128 cells) and random site-percolation masks (res 1 to 32, 55 to 70 % sea, shared nodes equal) are computed tiled with blocks of 1 to the whole face and 1, 2, 3 or 4 threads, at thresholds 0, small, the Earth default, large and 1
- **THEN** the components (label, nodes, area, ocean flag) and every bit equal the serial reference's, and the block reports can be merged in any order with the same table

#### Scenario: Nodes and areas
- **WHEN** the copies of every vertex of grids of 1 to 16 cells are enumerated, and the vertex areas of grids of 1 to 64 cells are summed
- **THEN** every copy names the same node (the smallest raw id), a grid has `6 (res - 1)^2 + 12 (res - 1) + 8` nodes with 12 (res - 1) edge nodes of two copies and 8 corner nodes of three, and the six faces add up to the sphere to the rounding of each vertex

#### Scenario: A dry basin one vertex from the ocean
- **WHEN** a basin below sea level sits behind a wall of land one vertex wide next to the open sea, with a threshold between them
- **THEN** the sea is one ocean component whose bits reach across the face edge to the copies on the neighbouring face, the basin and a one-vertex pond are separate components and stay dry, and the wall is not ocean

#### Scenario: Known answers
- **WHEN** the 128-cell bake of seed 1 is computed at the Earth threshold
- **THEN** its table has 5 components and its table hash and channel hash equal the pinned values on every platform

### Requirement: GEN-023 The water-level field W
The generators SHALL provide the water-level field W(dir) (ADR 0041 point 1): at a direction, either no water or a **sample** of the surface height above the reference surface in metres (`f64`), the **water-body id** (`u64`; the sea is `WaterBodyId::SEA`, ids with the top bit set are reserved for ids derived from the labels of the bake's components) and the **liquid kind** (`u32`, a row of the world's liquid table, WDEF-006). W SHALL be made by **producers** listed in a `WaterField` over a face mapping and the bake (a `LatticeSource`), most specific first, the first that reports water at a point winning; a producer is a pure function of the query (the direction, its position in the bake's lattice from `locate_direction` and the bake interpolated there by `read_direction`) and its own data. The only producer of M5 is the **sea**: a point is sea when the interpolated continentalness is at or below `SEA_LEVEL_C` **and** at least one of the lattice vertices with a non-zero weight in ADR 0027 point 2 (the lower corner always, the others when their axis weight is not zero) is ocean-connected (GEN-022); the channel is categorical and is not interpolated; W is the sea level of the definition (`body.<n>.sea.level_m`, constant in M5), the kind the definition's sea liquid and the body `SEA`; an inland basin below sea level and land have no water. The coast is defined by the continentalness alone, whatever the level: M5 seas are at the Earth default 0, and a sea level other than 0 is a producer parameter whose coast the water pass (item 7) decides. The set of vertices SHALL be the same nodes from either face of a shared face edge (the rule is edge-symmetric), an ocean channel of another resolution than the bake SHALL be an error naming both, and a missing or damaged block of the bake or of the channel SHALL be an error naming it, never wet or dry. `plan_water(dir)` SHALL give the GPU-free plan data of a view: the level and the body and kind of the water at the direction, else the reference level of the first producer that has one (the sea's) else 0. Class: all attributes are **fields** (CON-07, ADR 0035): the level, the body id and the kind are CPU values, bit-exact for every thread count and operating system, with no GPU twin; the level of the sea equals the definition's value bit for bit. The field's version (`WATER_VERSION`) and implementation id (`cpu-water-level-v1`) are recorded for the keys of whatever stores W later; nothing stores it in M5. The **retrofit** (ADR 0041 point 7): a synthetic lake (a basin with a fixed level inside a footprint, its own body id and a lava kind with emission) from a test-only producer SHALL go through the same `WaterField` path with no change to the sea producer: its water has its own level, body and kind, the sea's answers are the same with and without it, it never replaces sea, and the plan data follows it.

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
