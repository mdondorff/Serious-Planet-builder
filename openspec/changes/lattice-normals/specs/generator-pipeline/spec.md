## MODIFIED Requirements

### Requirement: GEN-003 Cross-OS tile hashes
A determinism manifest SHALL list output hashes for a committed set of tiles (height and mesh, every face, cube corners, levels 0 to 28) and reference-surface round trips, plus the generator, hash, mesh and key-format versions and the height and mesh keys of the same tiles for the planet and for a moon built through the generator. It records the mesh version, so a change of it (9 since ADR 0073) is a reviewed change of the committed manifest. It SHALL also list the global bake at 64 cells per face edge (the whole-grid hash and the hash of every block of 16 cells at level 2), the raw `f64` height of the terrain field (`cpu-terrain-v1`) and of the eroded field (`cpu-erosion-v1`) at twelve face-coordinate points on every face (the four cube corners, the four edge midpoints, the centre and three further points on and near edges and corners), the component table and the channel of the ocean connectivity of that bake (`oceantable`, `oceangrid`, GEN-022), the versions and implementation ids of the bake and both fields, and the persistent known answers of ADR 0018 (keys, node, body and query hashes, the bake's stage and block keys, an ocean block's key and the node set id; written by `app`, ADR 0023 point 9). A one-ulp change in the value behind any of the new entries SHALL change that entry. CI SHALL write it on Windows and Linux and fail when the two differ, naming every differing entry.

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

### Requirement: GEN-010 Mesh keys and known answers
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, the body parameter hash with the radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever the CPU mesh output changes for the same inputs, the compact construction (GEN-019) changes or the key composition changes; version 5 introduced the compact tile, which carries the mesh key of its tile; version 6 added the finest-level class (LOD-010: a tile at or deeper than the view's finest level had its key rehashed with the class, because the format of a finest tile was decided from its relief), which `remove-vertex-render-path` removes without a version bump (ADR 0062: no decision depends on the relief any more, so a tile's key is its base key at every level; the class key only named in-memory tiles at the cap, and the persistent pack never stores a tile at or above the cap, STRM-021, so no stored record can answer to a key whose meaning changed); version 7 adds the climate field (GEN-021): a tile carries 9 x 9 f16 texels of temperature and precipitation, and the key names the climate version word (the climate, reader and bake versions) so a change of any of them re-keys every tile, while the vertex output and the compact geometry are unchanged (the committed mesh hashes, which do not cover the climate, stay as they were under version 6); version 8 changes the compact tile to the slot of GEN-025 (wide texel, climate block and record inside the slot): every key changes (the version is a key input), the content of the CPU meshes and the committed mesh hashes stay as they were; version 9 (ADR 0073) defines the canonical normal as the lattice normal (GEN-019): every key changes, the vertex heights and positions are unchanged and the committed mesh hashes are re-recorded. The plan's keys and the compact tiles of one view SHALL agree: a tile has one key whatever the finest level of the view (the same tile under another cap is the same tile with the same key). Persisted height tiles (STRM-009) are not meshes and are unchanged by it; persistent mesh keys (kind mesh) change with the version, and repro bundles of an older mesh version are refused with the versions named (TEST-008). Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

Verify: A
Status: active
Source: ADR 0006, CON-14, LOD-010, GEN-025, ADR 0062

#### Scenario: Key follows inputs
- **WHEN** the seed, tile, grid size or radius of a mesh changes
- **THEN** its key changes, and equal inputs give equal keys

#### Scenario: Finest-level class
- **WHEN** the plan of a view is built with the cap 22 and with the cap 23
- **THEN** every draw tile has its base key under both caps (the class key is gone, ADR 0062), and the level-22 tile has the same key in both plans

#### Scenario: Known answers
- **WHEN** the committed meshes are generated
- **THEN** their content hashes equal the committed values, and the test fails if the mesh version differs from the one the values belong to

### Requirement: GEN-019 Compact tile
The tile builder SHALL produce, for levels 3 and finer, a compact tile (ADR 0033 point 4): one 8-byte texel per grid vertex (`(cells + 1)^2` texels: a 32-bit height step count between the tile's own minimum and maximum, then in the second word two signed 15-bit octahedral normal components in bits 0-14 and 15-29, bits 30-31 zero), the tile's climate block (GEN-021: 81 words, 324 bytes) and a 128-byte header (nine tile-local control points of a biquadratic base surface on the shell through the tile origin, the height range `hr_min` and `hr_step`, the origin height, the skirt depth and the flags: bit 0 the orientation flip, every other bit zero), laid out as one slot of 9,168 bytes at cells 32 (GEN-025; `MESH_VERSION` 9). A compact tile keeps the minimum of 16 cells per tile edge (a build with fewer cells is refused naming the minimum; the former reason, 162 climate values in 162 texels, is gone, the minimum stays so that the cells 16 configurations and their pool arithmetic are unchanged). A tile's key SHALL be the mesh key of its tile (GEN-010). The canonical normal of a vertex (ADR 0073) SHALL be the cross product of the centred differences of the positions of its lattice neighbours, `n = (q(k+1, l) - q(k-1, l)) x (q(k, l+1) - q(k, l-1))` normalised with `q = d (R + h)` in f64, where `d` is the neighbour's unit direction and `h` its height; every face has `U x V` outward, so `n` is outward without a flip, and a flip is an assertion failure (except at the eight cube corners, below). The neighbours are the tile's own lattice points and a one-sample ring around them (without its four corners); a ring point across a tile edge inside a face is the next tile's lattice point, and one beyond a face edge is the neighbour tile's own lattice direction one cell inside it, on the row that continues the shared edge, found from the exact integer cube point and never by projecting the extended plane back (`TileId::ring_direction`). Two tiles sharing a vertex therefore read the same directions and compute bit-identical normals, inside a face and across the twelve cube edges (a cross product negates exactly when both operands or their order change sign). At a cube corner the three tiles read the same three edge points, order them canonically by the pair of faces that hold each (`TileId::corner_stencil`), form `(p1 - p0) x (p2 - p0)` from the same operands and orient it outward; their normals are bit-identical. The CPU mesh and the compact tile use this one definition. The stored normal SHALL equal the canonical CPU normal within 0.01 degrees (CON-14); the height SHALL be reconstructed as `hr_min + f32(q) * hr_step` with the step the relief over 16,777,215 (2^24 - 1 levels in the 32-bit field, so every `q` is an integer an f32 holds exactly and the CPU and the shader form the same `f32(q)`) and the range rounded outwards so no height is clamped, the height error SHALL be at most half a step plus f32 rounding of the sum (the step is at most 1 mm up to 1 mm x 16,777,215 = 16.7 km of relief in one tile; the f32 sum `hr_min + f32(q) * hr_step` adds about 2^-24 of the relief and of `|hr_min|`, so 1 mm of reconstruction error holds up to roughly 8 km of relief; the field stays 32 bits wide), and every tile SHALL be built compact whatever its relief (ADR 0062: there is no vertex fallback; the wide texel holds CON-13's 1 mm on steep finest tiles, SPIKE-12). The bound of the wide texel is the relief of one tile: the step is relief / 16,777,215, so it is at most 1 mm up to 16.7 km of relief in one tile and grows in proportion above (about 1.19 mm at 20 km, 2.4 mm at 40 km). The builder SHALL NOT refuse a tile for its relief: no spec carries a level-relevant tolerance that a larger step would break (the highest relief of a tile in the specs is the steep stress field's 1,875 m at level 9, and the planet's total relief is below 20 km), so the bound is documented and tested (16.7 km and 20 km), not enforced. Tiles of levels 0-2 are not drawn (LOD-011) and not stored (STRM-021). The CPU mirror of the shader's vertex stage (position, morph target, skirts, heights, normal) SHALL agree with the canonical CPU mesh at every vertex of tiles of levels 3 to 26 within the model error, the quantisation step and f32 rounding, with the exact morph target (neighbour heights read from the texels), and neighbouring tiles of one level SHALL meet within the model error of the two (millimetres from level 12). Tiles SHALL be bit-identical across runs and thread counts, with known-answer hashes next to the mesh version they belong to. Every value in a tile SHALL be tile-local or a unit vector (CON-11).

Verify: A, B, C
Status: active
Source: ADR 0033 points 4 and 5, SPIKE-06, CON-11, CON-13, CON-14, LOD-010, GEN-025, ADR 0062, SPIKE-12

#### Scenario: Layout and bytes
- **WHEN** a tile is built at cells 32 and cells 64
- **THEN** it has 1,089 / 4,225 texels of 8 bytes, a 128-byte header and a 324-byte climate block (slots of 9,168 B and 34,256 B, GEN-025), no texel carries climate, and its key equals the mesh key of the same tile under `MESH_VERSION` 9

#### Scenario: Normals
- **WHEN** 200,000 random directions, the axes and the octahedron's fold are encoded and decoded, and the stored normals of tiles of levels 3 to 22 are compared with the CPU mesh
- **THEN** the largest angle is at most 0.01 degrees (measured 0.0072 with 15 bits)

#### Scenario: The lattice normal
- **WHEN** the lattice normals of tiles over a smooth wave of 20 km are compared with a converged normal (centred differences of 1 m in a tangent frame), inside a face and at the face edges, and a flat field is drawn
- **THEN** inside a face they agree within 0.0002 degrees, at a face edge (the ring point comes from the neighbour face) and at a cube corner within 0.015 degrees, and a sphere's normals are radial within 0.01 degrees

#### Scenario: Seams
- **WHEN** the tiles at the four corners of every face, and tiles on every face edge with their four neighbours, at levels 3, 12 and 22, are built on one and on three threads over the eroded and the steep field
- **THEN** every pair of vertices with the same direction bits, inside a face, across the twelve cube edges and at the eight cube corners (three tiles each), has the same compact word, the same CPU mesh normal and the same f64 normal bit for bit, and the thread count changes nothing

#### Scenario: The ring
- **WHEN** the ring points of tiles at face edges are compared with the neighbour tile's lattice, and the stencils of the corner tiles at the eight corners at levels 3, 12 and 22 are compared
- **THEN** every ring point beyond a face edge is a lattice point of the neighbour inside it and not on the shared edge, and the three tiles at a corner list the same three points in the same order

#### Scenario: Height step
- **WHEN** tiles of the eroded field at levels 3 to 26 are built
- **THEN** every texel is within half a step (plus f32 rounding) of the exact height, no level is clamped, and the step is at most 1 mm at every level for tiles up to 16.7 km of relief

#### Scenario: Format decision
- **WHEN** a finest tile of the steep stress field with more than 65.535 m of relief, a finest tile of the reference field and a coarse tile with large relief are built
- **THEN** all are compact tiles (there is no other form to decide on, ADR 0062), equal to the canonical mesh through the CPU mirror within the model error

#### Scenario: Format follows the cap
- **WHEN** the same steep tile is built with its level as the finest level and with a deeper cap
- **THEN** both are the same compact tile with the same key

#### Scenario: The relief bound of the wide texel
- **WHEN** a tile over a ramp field of 1,000, 16,700, 20,000 and 40,000 m of relief is built
- **THEN** it is built, not refused, and its height step is relief / 16,777,215: 1 mm or less up to 16.7 km, 1.19 mm at 20 km (GEN-019's documented bound)

#### Scenario: Model error and seams
- **WHEN** the CPU mirror of the shader is compared with the canonical mesh over tiles of levels 3 to 26 on the reference, eroded and steep fields, and neighbouring tiles of one level are compared along their shared edge
- **THEN** positions and morph targets are within the biquadratic bound, heights within the quantisation bound, normals within 0.01 degrees, and from level 12 neighbouring tiles meet within 3 mm

#### Scenario: Determinism
- **WHEN** the same tiles are built on 1, 2 and 4 threads
- **THEN** their content hashes are equal to each other and to the committed known answers

#### Scenario: Cliff tiles hold 1 mm
- **WHEN** finest tiles of the steep stress field (1,875 m of relief at level 9, 805 m at level 12, 142 m at level 16, 6.7 m at level 22) are built and compared with the canonical mesh through the CPU mirror
- **THEN** the height step is at most 1 mm and the height error at most 1 mm (f32 granularity of the compared heights) at every level, and the normal error at most 0.01 degrees (CON-13, CON-14; SPIKE-12 measured 0.0071 degrees at level 9 to 12; the step is relief / 16,777,215, 0.1 mm at level 9)

#### Scenario: Image noise of the wide texel
- **WHEN** the ten standard views are drawn from wide tiles and from the previous 16-bit tiles in the relief, normals and height views (recorded once at the change, then the old texel is gone)
- **THEN** at most 1.6 % of a frame's pixels differ, almost all by one level, and at most one pixel per frame by more than 8 levels (SPIKE-12 T10: rounding noise in the sense of ADR 0017)

### Requirement: GEN-020 Batch evaluation is order independent
A height field SHALL offer a batch evaluation of a list of directions that returns, for each direction and in the caller's order, exactly the value `height` returns for it (bit for bit). A field that reads a block-structured lattice source SHALL gather the lattice cells the whole batch reads, read them from the source grouped by block, and evaluate from those cells, so that a tile decodes each block it touches once, whatever the source's cache holds, instead of once per cache miss in raster order. The tile builders (the vertex mesh and the compact tile) SHALL obtain every height they need (lattice samples, the one-sample ring around them without its four corners, the origin: 1,222 directions at 32 cells against the 5,446 of the former four-point stencil, ADR 0073) through one batch call, together with the tile's climate (GEN-021: `heights_and_climate` gathers the cells of both in one pass, so a tile still decodes each block once however small the cache), and the tile SHALL be bit-identical in raster order and in block order, at any thread count. The batch SHALL poll its stop flag before each block group, every 64 directions of the footprint pass, every 64 cells of the gather and every 64 samples and read nothing once it is set. The batch SHALL also be available on up to `threads` threads (`heights_threaded`): the distinct blocks are dealt to the threads, each thread reading the cells of one block at a time, and the evaluation from the gathered cells is split over the threads; the values and the number of block reads SHALL NOT depend on `threads`, and a batch of two directions or more is gathered. No key, bake format, mesh version or golden changes by the batch itself (ADR 0073 changes the points it asks for).

Verify: A
Status: active
Source: opus-planner decision of 2026-10-15 (delegation), STRM-014, STRM-005, GEN-017, CON-14, M4 fix F3 (LOD-009)

#### Scenario: Bit identity
- **WHEN** a tile (a level-0 root, a coarse tile spanning many blocks, a fine tile, a tile on a face edge, one on a cube corner) is built in raster order and in block order, over the eroded field on an in-memory bake and on a source with a small cache (1 and 20 threads), and on a stored multi-block bake with a cache of one block against an unbounded one (the store at 1, 4 and 20 threads is also STRM-014's; tier A runs 1 and 3 threads, the full sweep is tier B, `oracle_concurrent_source`)
- **THEN** every vertex bit and the mesh key are equal

#### Scenario: Decodes against distinct blocks
- **WHEN** a coarse tile spanning many blocks is built over a source with a cache of a few blocks
- **THEN** the decodes are at most the distinct blocks the tile touches (also with a cache of one block), raster order many times that

#### Scenario: Cancellation
- **WHEN** the stop flag becomes set during a batch
- **THEN** the batch stops within 64 samples and reads nothing more

#### Scenario: Threaded batch
- **WHEN** a batch of 3, 52 and 300 directions is evaluated on 0, 1, 2, 5 and 20 threads
- **THEN** every value is bit for bit `height` of its direction and every distinct block is read exactly once; a set stop flag reads nothing, and a flag set during the reads stops every thread within 64 reads of its own

### Requirement: GEN-025 Compact slot layout
A compact tile SHALL be one slot of contiguous bytes, the same bytes in the CPU tile, the pack payload (STRM-021), the block of the pool (REND-016) and the shader's reads, so no second resource per tile exists (ADR 0062 point 2). At cells 32 the slot is 9,168 bytes, a multiple of 16:

| offset | bytes | content |
|---|---|---|
| 0 | 128 | header: nine control points (108 B), `hr_min`, `hr_step`, origin height, skirt depth (16 B), flags (4 B) |
| 128 | 324 | climate block (GEN-021): 81 words, temperature f16 in the low half, precipitation f16 in the high half |
| 452 | 4 | pad |
| 456 | 8 (cells + 1)^2 | texels (GEN-019), row-major, 8,712 B at cells 32 |

The stride SHALL be `456 + 8 (cells + 1)^2` bytes (9,168 B at cells 32, 2,768 B at cells 16, 34,256 B at cells 64: a multiple of 16 for every even cell count), and the offsets SHALL be constants of the layout version; a later per-tile block is appended after the climate block and moves the texel offset by its size without changing any other offset. `MESH_VERSION` SHALL be 8: every key and persistent key of a compact tile changes, the pack payload version is 3 (STRM-021), and the known-answer hashes of GEN-019 and GEN-021 are re-recorded. The slot SHALL be bit-identical across runs, thread counts and operating systems, and every value in it SHALL be tile-local or a unit vector (CON-11).

Verify: A, B
Status: active
Source: ADR 0062 point 2, ADR 0063, SPIKE-12 Q3, ADR 0033 points 3-4, GEN-019, GEN-021, CON-11, CON-14

#### Scenario: Offsets and stride
- **WHEN** a slot is built at cells 16, 32 and 64
- **THEN** the strides are 2,768, 9,168 and 34,256 bytes, every stride is a multiple of 16, and the header, climate block, pad and texels sit at the offsets of the table

#### Scenario: One byte string
- **WHEN** a tile's slot is encoded to the pack payload, decoded, uploaded to a block and read back
- **THEN** the four byte strings are equal and equal the CPU builder's, and the shader's reads of header, climate and texel words at the offsets agree with the CPU mirror

#### Scenario: Determinism and version
- **WHEN** the slots of 24 tiles are built on 1, 2 and 4 threads
- **THEN** the content hashes are equal to each other and to the committed known answers of `MESH_VERSION` 9
