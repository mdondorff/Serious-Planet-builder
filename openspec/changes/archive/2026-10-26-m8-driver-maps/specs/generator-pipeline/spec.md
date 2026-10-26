## ADDED Requirements

### Requirement: GEN-070 Slope, curvature, wetness and moisture of a chunk
`generators::drivers` SHALL compute, for every core cell of a chunk raster from the cell and its eight neighbours at their true distances (the lattice is equal-angle, and across a face edge the neighbour found is the real cell), the slope, the mean curvature and the wetness `ln(A / (w tan s))` with `tan s` floored at 0.01 and the drainage area `A` clamped to 1e8 m2, and the moisture `clamp((W - 4) / 12, 0, 1)`. A non-finite height or area SHALL be refused. The functions SHALL be pure, use `libm` only and no threads.

Verify: A
Status: active
Source: ADR 0076 points 2 and 3, SPIKE-30, planner decision 2026-10-26

#### Scenario: Known rasters
- **WHEN** the drivers are computed on a plane, a bowl, a saddle, a cone and a hollow whose area exceeds the clamp
- **THEN** the slope and curvature equal the geometry's, a flat gives the wetness of the floor, the hollow the maximum, a plane on cells 0.7 as wide gives the same drivers, a ramp on a cube-face edge gives the same codes at the edge as two cells in, and a second run on another thread gives the same bits

### Requirement: GEN-075 Soil depth of a chunk
The soil depth of a core cell SHALL be `soil_base_m` (2.5 m) times a slope factor `exp(-slope / 18 degrees)`, a curvature factor `1 + 0.5 tanh(100 m kappa)`, a flow factor from 0.5 to 1 over the drainage area 1e2 to 1e8 m2, a lithology factor `1 + 0.5 n` and an altitude factor falling to 0 from 2,500 to 4,500 m, clamped to 0 to 8 m and stored square-root coded in 8 bits. The lithology `n` SHALL be `height::layer_noise` at the cell's f64 unit direction times 400, seeded with the world's seed (`drivers.seed`, set by the stage's owner). The constants SHALL be part of the stage's definition hash.

Verify: A
Status: active
Source: ADR 0076, SPIKE-30, planner decision 2026-10-26, CON-11

#### Scenario: Factors
- **WHEN** the soil depth is evaluated on a paraboloid of curvature 0.01 per metre, on steep ground, with more flow, high up and for the two ends of the lithology
- **THEN** the curvature factor is 1.3808, steep and high ground are thin, more flow and a hollow are deeper, and the codes round trip within half a code step

### Requirement: GEN-076 Annual-mean insolation of a chunk
The insolation of a core cell SHALL be the annual-mean beam irradiance on a tilted plane (Duffie and Beckman's cosine, every fifth day of the year in 96 steps of the day, the sun above the horizon only, no shadowing by terrain, never the current sun) in units of the flat plane at the equator, read bilinearly from a 17 x 17 table over slope (0 to 60 degrees, steeper cells read 60) and aspect at the latitude of the chunk's centre.

Verify: A
Status: active
Source: ADR 0076, SPIKE-30, planner decision 2026-10-26

#### Scenario: Against a brute force
- **WHEN** the insolation is compared with a one-minute integration over every day of the year at the equator and at 45 degrees north up to 60 degrees of slope
- **THEN** it is within 1 %, and the table reads the function it tabulates and wraps the aspect

### Requirement: GEN-071 Driver channels of a chunk and their blend
A chunk result SHALL carry the soil depth, the moisture and the insolation as three 8-bit channels at the raster with box-filtered pyramid levels, in the chunk's disk record. A consumer SHALL read a driver as `neutral + w (chunk - neutral)` with `w` the chunk weight that fades the height; the neutral values are soil 1 m, moisture 0.5 and the insolation of the latitude alone.

Verify: A
Status: active
Source: ADR 0076, GEN-037, planner decision 2026-10-26

#### Scenario: The range edge
- **WHEN** the weight of a chunk goes from 0 to 1
- **THEN** the driver goes from the neutral value to the chunk's value, monotone, with no step

### Requirement: GEN-072 The materials read the drivers
`MaterialParams::weights` SHALL read the soil depth, the moisture and the insolation from the blended drivers in its reserved inputs, by the same function and the same input vector as for the other inputs.

Verify: A
Status: active
Source: LOOK-008, ADR 0076 point 5

#### Scenario: A hollow
- **WHEN** a look gives soil a curve on the moisture input and the drivers describe a wet hollow
- **THEN** soil takes weight in the hollow and none on the dry flank

### Requirement: GEN-073 The default look ignores the drivers
The default look SHALL give no curve to the soil-depth, moisture and insolation inputs, so its material weights and albedo SHALL be bit-identical for every driver value, and the images of every committed golden SHALL be unchanged with no candidate; the neutral values matter only once a look gives those inputs curves.

Verify: A, C
Status: active
Source: planner decision 2026-10-26, ADR 0017

#### Scenario: Today's goldens
- **WHEN** the default look's weights are evaluated for several driver vectors and the committed goldens are rendered
- **THEN** the weights are equal bit for bit and every image equals its golden

### Requirement: GEN-074 The tile driver block and the driver views
A compact tile SHALL carry a driver block of 9 x 9 words (GEN-025): at each climate texel the soil depth, moisture and insolation codes of the ready chunk under it as the tile's level reads them, or the neutral codes where none is ready, and the neutral insolation of the texel's latitude in the top byte. The fragment stage SHALL read the block bilinearly, blend it as `neutral + w (chunk - neutral)` with `w` one minus the node's fade, and fill the reserved material inputs (GEN-072). The debug views `soil`, `moisture` and `insolation` (`ShadeView` 7 to 9, reached by J) SHALL show the blended drivers in false colour.

Verify: A, C
Status: active
Source: ADR 0076 point 5, GEN-071, GEN-072, GEN-025

#### Scenario: Under a chunk and away from one
- **WHEN** compact tiles are built over a ready chunk with drivers and away from every chunk
- **THEN** the first hold the chunk's codes in every texel, the others the neutral codes, a tile with chunk drivers has another content hash, and a tile under a chunk that moved no height gets a plane of zeros so its drivers fade in

#### Scenario: The views
- **WHEN** a compact tile whose driver block is uniform, and one whose block is a ramp, are drawn in each driver view
- **THEN** every covered pixel has the colour the CPU mirror gives for those codes (within 0.003 for the uniform block), and a node with no fade plane reads the chunk's values at full weight

#### Scenario: Constants
- **WHEN** the block is decoded
- **THEN** the soil code uses the default `soil_max_m` of 8 m in the builder and the shader alike

## MODIFIED Requirements

### Requirement: GEN-019 Compact tile
The tile builder SHALL produce, for levels 3 and finer, a compact tile (ADR 0033 point 4): one 8-byte texel per grid vertex (`(cells + 1)^2` texels: a 32-bit height step count between the tile's own minimum and maximum, then in the second word two signed 15-bit octahedral normal components in bits 0-14 and 15-29, bits 30-31 zero), the tile's climate block (GEN-021: 81 words, 324 bytes), its driver block (GEN-074: 81 words, 324 bytes) and a 128-byte header (nine tile-local control points of a biquadratic base surface on the shell through the tile origin, the height range `hr_min` and `hr_step`, the origin height, the skirt depth and the flags: bit 0 the orientation flip, every other bit zero), laid out as one slot of 9,488 bytes at cells 32 (GEN-025; `MESH_VERSION` 10). A compact tile keeps the minimum of 16 cells per tile edge (a build with fewer cells is refused naming the minimum; the former reason, 162 climate values in 162 texels, is gone, the minimum stays so that the cells 16 configurations and their pool arithmetic are unchanged). A tile's key SHALL be the mesh key of its tile (GEN-010). The canonical normal of a vertex (ADR 0073) SHALL be the cross product of the centred differences of the positions of its lattice neighbours, `n = (q(k+1, l) - q(k-1, l)) x (q(k, l+1) - q(k, l-1))` normalised with `q = d (R + h)` in f64, where `d` is the neighbour's unit direction and `h` its height; every face has `U x V` outward, so `n` is outward without a flip, and a flip is an assertion failure (except at the eight cube corners, below). The neighbours are the tile's own lattice points and a one-sample ring around them (without its four corners); a ring point across a tile edge inside a face is the next tile's lattice point, and one beyond a face edge is the neighbour tile's own lattice direction one cell inside it, on the row that continues the shared edge, found from the exact integer cube point and never by projecting the extended plane back (`TileId::ring_direction`). Two tiles sharing a vertex therefore read the same directions and compute bit-identical normals, inside a face and across the twelve cube edges (a cross product negates exactly when both operands or their order change sign). At a cube corner the three tiles read the same three edge points, order them canonically by the pair of faces that hold each (`TileId::corner_stencil`), form `(p1 - p0) x (p2 - p0)` from the same operands and orient it outward; their normals are bit-identical. The CPU mesh and the compact tile use this one definition. The stored normal SHALL equal the canonical CPU normal within 0.01 degrees (CON-14); the height SHALL be reconstructed as `hr_min + f32(q) * hr_step` with the step the relief over 16,777,215 (2^24 - 1 levels in the 32-bit field, so every `q` is an integer an f32 holds exactly and the CPU and the shader form the same `f32(q)`) and the range rounded outwards so no height is clamped, the height error SHALL be at most half a step plus f32 rounding of the sum (the step is at most 1 mm up to 1 mm x 16,777,215 = 16.7 km of relief in one tile; the f32 sum `hr_min + f32(q) * hr_step` adds about 2^-24 of the relief and of `|hr_min|`, so 1 mm of reconstruction error holds up to roughly 8 km of relief; the field stays 32 bits wide), and every tile SHALL be built compact whatever its relief (ADR 0062: there is no vertex fallback; the wide texel holds CON-13's 1 mm on steep finest tiles, SPIKE-12). The bound of the wide texel is the relief of one tile: the step is relief / 16,777,215, so it is at most 1 mm up to 16.7 km of relief in one tile and grows in proportion above (about 1.19 mm at 20 km, 2.4 mm at 40 km). The builder SHALL NOT refuse a tile for its relief: no spec carries a level-relevant tolerance that a larger step would break, so the bound is documented and tested (16.7 km and 20 km), not enforced. Tiles of levels 0-2 are not drawn (LOD-011) and not stored (STRM-021). The CPU mirror of the shader's vertex stage (position, morph target, skirts, heights, normal) SHALL agree with the canonical CPU mesh at every vertex of tiles of levels 3 to 26 within the model error, the quantisation step and f32 rounding, with the exact morph target (neighbour heights read from the texels), and neighbouring tiles of one level SHALL meet within the model error of the two (millimetres from level 12). Tiles SHALL be bit-identical across runs and thread counts, with known-answer hashes next to the mesh version they belong to. Every value in a tile SHALL be tile-local or a unit vector (CON-11).

Verify: A, B, C
Status: active
Source: ADR 0033 points 4 and 5, SPIKE-06, CON-11, CON-13, CON-14, LOD-010, GEN-025, ADR 0062, SPIKE-12

#### Scenario: Layout and bytes
- **WHEN** a tile is built at cells 32 and cells 64
- **THEN** it has 1,089 / 4,225 texels of 8 bytes, a 128-byte header and a 324-byte climate block and a 324-byte driver block (slots of 9,488 B and 34,576 B, GEN-025), no texel carries climate, and its key equals the mesh key of the same tile under `MESH_VERSION` 10

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

### Requirement: GEN-025 Compact slot layout
A compact tile SHALL be one slot of contiguous bytes, the same in the CPU tile, the pack payload (STRM-021), the pool block (REND-016) and the shader's reads (ADR 0062 point 2). The stride SHALL be `776 + 8 (cells + 1)^2` bytes, a multiple of 16 for every even cell count, with these offsets as constants of the layout version, and `MESH_VERSION` SHALL be 10 (payload version 4):

| offset | bytes | content |
|---|---|---|
| 0 | 128 | header: nine control points, `hr_min`, `hr_step`, origin height, skirt depth, flags |
| 128 | 324 | climate block (GEN-021): 81 words, temperature f16 low, precipitation f16 high |
| 452 | 324 | driver block (GEN-074): 81 words, soil depth, moisture, insolation, neutral insolation |
| 776 | 8 (cells + 1)^2 | texels (GEN-019), row-major |

The slot SHALL be bit-identical across runs, thread counts and operating systems, with every value tile-local or a unit vector (CON-11).

Verify: A, B
Status: active
Source: ADR 0062 point 2, ADR 0063, ADR 0076, SPIKE-12 Q3, ADR 0033 points 3-4, GEN-019, GEN-021, GEN-074, CON-11, CON-14

#### Scenario: Offsets and stride
- **WHEN** a slot is built at cells 16, 32 and 64
- **THEN** the strides are 3,088, 9,488 and 34,576 bytes (8,712 B of texels at cells 32), every stride is a multiple of 16, and the header, climate block, driver block and texels sit at the offsets of the table

#### Scenario: One byte string
- **WHEN** a tile's slot is encoded to the pack payload, decoded, uploaded to a block and read back
- **THEN** the four byte strings are equal and equal the CPU builder's, and the shader's reads of header, climate, driver and texel words at the offsets agree with the CPU mirror

#### Scenario: Determinism and version
- **WHEN** the slots of 24 tiles are built on 1, 2 and 4 threads
- **THEN** the content hashes are equal to each other and to the committed known answers of `MESH_VERSION` 10; the driver block moved the texels from byte 456 to 776 and each key and persistent key changes with the version
