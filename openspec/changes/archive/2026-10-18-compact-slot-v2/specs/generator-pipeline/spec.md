## MODIFIED Requirements

### Requirement: GEN-019 Compact tile
The tile builder SHALL produce, for levels 3 and finer, a compact tile (ADR 0033 point 4): one 8-byte texel per grid vertex (`(cells + 1)^2` texels: a 32-bit height step count between the tile's own minimum and maximum, then in the second word two signed 15-bit octahedral normal components in bits 0-14 and 15-29, bits 30-31 zero), the tile's climate block (GEN-021: 81 words, 324 bytes) and a 128-byte header (nine tile-local control points of a biquadratic base surface on the shell through the tile origin, the height range `hr_min` and `hr_step`, the origin height, the skirt depth and the flags: bit 0 the orientation flip, every other bit zero), laid out as one slot of 9,168 bytes at cells 32 (GEN-025; `MESH_VERSION` 8). A compact tile keeps the minimum of 16 cells per tile edge (a build with fewer cells is refused naming the minimum; the former reason, 162 climate values in 162 texels, is gone, the minimum stays so that the cells 16 configurations and their pool arithmetic are unchanged). A tile SHALL have the same key in either format (the mesh key of its finest-level class, GEN-010). The stored normal SHALL equal the canonical CPU normal within 0.01 degrees (CON-14); the height SHALL be reconstructed as `hr_min + f32(q) * hr_step` with the step the relief over 16,777,215 (2^24 - 1 levels in the 32-bit field, so every `q` is an integer an f32 holds exactly and the CPU and the shader form the same `f32(q)`) and the range rounded outwards so no height is clamped, the height error SHALL be at most half a step plus f32 rounding of the sum (the step is at most 1 mm up to 1 mm x 16,777,215 = 16.7 km of relief in one tile; the f32 sum `hr_min + f32(q) * hr_step` adds about 2^-24 of the relief and of `|hr_min|`, so 1 mm of reconstruction error holds up to roughly 8 km of relief; the field stays 32 bits wide), and a finest-level tile (a tile at or deeper than the LOD cap `max_level`, LOD-010) with more than 65.535 m of relief SHALL still be built in the vertex format instead, as SHALL every tile of levels 0-2 (the wide texel holds CON-13's 1 mm on such a tile; the guard and the finest-level class of the key stay, so formats and keys stay as they are, until the vertex format is removed by `remove-vertex-render-path`; a builder switch turns the guard off for the CON-13 tests) (the base surface's model error of about 0.018 theta^2 of the tile edge is too large there). The CPU mirror of the shader's vertex stage (position, morph target, skirts, heights, normal) SHALL agree with the canonical CPU mesh at every vertex of tiles of levels 3 to 26 within the model error, the quantisation step and f32 rounding, with the exact morph target (neighbour heights read from the texels), and neighbouring tiles of one level SHALL meet within the model error of the two (millimetres from level 12). Tiles SHALL be bit-identical across runs and thread counts, with known-answer hashes next to the mesh version they belong to. Every value in a tile SHALL be tile-local or a unit vector (CON-11).

Verify: A, B, C
Status: planned
Source: ADR 0033 points 4 and 5, SPIKE-06, CON-11, CON-13, CON-14, LOD-010, GEN-025, ADR 0062, SPIKE-12

#### Scenario: Layout and bytes
- **WHEN** a tile is built at cells 32 and cells 64
- **THEN** it has 1,089 / 4,225 texels of 8 bytes, a 128-byte header and a 324-byte climate block (slots of 9,168 B and 34,256 B, GEN-025), no texel carries climate, and its key equals the mesh key of the same tile (of its class) under `MESH_VERSION` 8

#### Scenario: Normals
- **WHEN** 200,000 random directions, the axes and the octahedron's fold are encoded and decoded, and the stored normals of tiles of levels 3 to 22 are compared with the CPU mesh
- **THEN** the largest angle is at most 0.01 degrees (measured 0.0072 with 15 bits)

#### Scenario: Height step
- **WHEN** tiles of the eroded field at levels 3 to 26 are built
- **THEN** every texel is within half a step (plus f32 rounding) of the exact height, no level is clamped, and the step is at most 1 mm at every level for tiles up to 16.7 km of relief

#### Scenario: Format decision
- **WHEN** a tile of level 0 to 2, a finest tile of the steep stress field with more than 65.535 m of relief, a finest tile of the reference field and a coarse tile with large relief are built
- **THEN** the first two are the vertex format (the second equal to the canonical mesh, under the key of its class), the other two compact

#### Scenario: Format follows the cap
- **WHEN** the same steep tile is built with its level as the finest level and with a deeper cap
- **THEN** the first is a vertex fallback and the second compact, and their keys differ

#### Scenario: Model error and seams
- **WHEN** the CPU mirror of the shader is compared with the canonical mesh over tiles of levels 3 to 26 on the reference, eroded and steep fields, and neighbouring tiles of one level are compared along their shared edge
- **THEN** positions and morph targets are within the biquadratic bound, heights within the quantisation bound, normals within 0.01 degrees, and from level 12 neighbouring tiles meet within 3 mm

#### Scenario: Determinism
- **WHEN** the same tiles are built on 1, 2 and 4 threads
- **THEN** their content hashes are equal to each other and to the committed known answers

#### Scenario: Cliff tiles hold 1 mm
- **WHEN** finest tiles of the steep stress field (1,875 m of relief at level 9, 805 m at level 12, 142 m at level 16, 6.7 m at level 22) are built with the guard off and compared with the canonical mesh through the CPU mirror
- **THEN** the height step is at most 1 mm and the height error at most 1 mm (f32 granularity of the compared heights) at every level, and the normal error at most 0.01 degrees (CON-13, CON-14; SPIKE-12 measured 0.0071 degrees at level 9 to 12; the step is relief / 16,777,215, 0.1 mm at level 9)

#### Scenario: Image noise of the wide texel
- **WHEN** the ten standard views are drawn from wide tiles and from the previous 16-bit tiles in the relief, normals and height views (recorded once at the change, then the old texel is gone)
- **THEN** at most 1.6 % of a frame's pixels differ, almost all by one level, and at most one pixel per frame by more than 8 levels (SPIKE-12 T10: rounding noise in the sense of ADR 0017)

### Requirement: GEN-021 Climate field of a tile
A tile SHALL carry its climate (ADR 0039 point 2): the temperature in degrees C and the precipitation in mm per year at 9 x 9 points, texel `(a, b)` at tile sample `(a, b)` of a tile grid of 8 cells, two f16 values per texel, 324 bytes per tile, in both tile formats (the compact tile's climate block in its slot, GEN-025; the vertex tile's own 324-byte block, STRM-021). It is a **field** (ADR 0035 point 1): it is produced by a generator, stored and keyed with the tile and read by the renderer, so it has its own implementation id (`cpu-climate-v1`) and version, and one implementation per cache with the CPU canonical (CON-07). Its values come from the global bake read with the integer reader of ADR 0027 (`read_tile_sample`, by every field over a bake: the terrain field and the eroded field; a wrapper of a field, such as the streamer's cancellable one, delegates `climate`, `climate_version_word` and `heights_and_climate` so a tile built through it has the key and the climate of the inner field; lattice position from integers, face-edge-symmetric interpolation, `READER_VERSION`); a field without a bake (the reference generator, synthetic fields) takes the analytic global-coarse function of its seed at the same directions, a pure function of `(seed, mapping, tile)`. Both producers measure latitude in COORD's axes: +z is the north pole, so at +z the climate is that of latitude 90, at -z that of latitude -90 and at +x and +y that of the equator (the analytic function once took the latitude from the y component, which put its poles at +-y; fixed with `GLOBAL_VERSION` 3, whose bump re-keys every tile through the version word). The stored value SHALL be the f16 rounding (round to nearest, ties to even) of the reader's f32 value, computed with integer arithmetic and `libm` only, so the CPU result is the definition of the field, the implementation id `cpu-climate-v1` is part of the version word, the GPU decode (`unpack2x16float`) is exact and CON-14's CPU/GPU bound for this field is zero; against the bake the rounding costs at most 2^-11 of the value (declared quantisation: half a step, at most 0.008 degrees below 16 C and 0.016 below 32 C, 2 mm of precipitation below 2,048 mm and 4 mm below 4,096 mm; the field's range is 0 to 4,500 mm), a deviation from ADR 0039's "abs 0.01 degrees and 0.1 mm" that f16 cannot meet for large values, recorded here. The version word that enters every mesh key and persistent mesh key (`PARAM_CLIMATE_VERSION`) SHALL cover the climate version, `READER_VERSION` and `GLOBAL_VERSION`. Texels SHALL be bit-identical across runs, thread counts and operating systems, with known answers, neighbouring tiles SHALL share their border texels bit for bit (also across face edges), and an unreadable block SHALL be an error naming it (recorded like a height's, never a default).

Verify: A, B
Status: planned
Source: ADR 0039 point 2, ADR 0027, ADR 0035, ADR 0033, CON-07, CON-14, CON-05, GEN-025, ADR 0062

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
- **THEN** both carry the same climate, the compact tile's climate block is 324 bytes in its slot, its texels are 8,712 bytes and hold no climate, and a field with another climate version word has another mesh key

#### Scenario: Fields over a bake
- **WHEN** the terrain field and the eroded field are built over a bake, and a tile is built through the streamer's cancellable wrapper of each
- **THEN** their climate is the reader's (not the analytic function's), their version word is the bake's, and the wrapped tile equals the unwrapped one including its key and climate, in both formats

#### Scenario: Interpolation
- **WHEN** the texture is sampled at texel positions and between them
- **THEN** a texel position gives the texel, a midpoint the mean, and a vertex at a texel position takes the texel's value

#### Scenario: Latitude axis
- **WHEN** the climate texture is built (analytically and over a bake) for the tile at the +z pole, the tile at the -z pole and a tile on the equator at +x
- **THEN** the temperature texels at both poles are at least 30 degrees C colder than the equator's, and the version word names the current `GLOBAL_VERSION`

### Requirement: GEN-010 Mesh keys and known answers
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, the body parameter hash with the radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever vertex output changes for the same inputs, the compact construction (GEN-019) changes or the key composition changes; version 5 introduced the compact tile, which carries the mesh key of its tile, so a tile has one key in either format; version 6 adds the finest-level class (LOD-010): a tile at or deeper than the view's finest level has the key of its tile rehashed with the class, because the format of a finest tile is decided from its relief (GEN-019) and a resident tile is reused by its key, so the key SHALL identify what the format decision depended on; tiles below the finest level keep their base key; version 7 adds the climate field (GEN-021): a tile carries 9 x 9 f16 texels of temperature and precipitation, and the key names the climate version word (the climate, reader and bake versions) so a change of any of them re-keys every tile, while the vertex output and the compact geometry are unchanged (the committed vertex mesh hashes, which do not cover the climate, stay as they were under version 6); version 8 changes the compact tile to the slot of GEN-025 (wide texel, climate block and record inside the slot): every key changes (the version is a key input, vertex tiles' too), the content of the vertex tiles and the committed vertex mesh hashes stay as they were. The plan's keys, the compact tiles and the vertex tiles of one view SHALL all carry the key of the view's finest level, and a finest tile decided as a fallback under one cap SHALL NOT share a key with the compact tile of the same tile under a deeper cap. Persisted height tiles (STRM-009) are not meshes and are unchanged by it; persistent mesh keys (kind mesh) change with the version, and repro bundles of an older mesh version are refused with the versions named (TEST-008). Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

Verify: A
Status: planned
Source: ADR 0006, CON-14, LOD-010, GEN-025, ADR 0062

#### Scenario: Key follows inputs
- **WHEN** the seed, tile, grid size or radius of a mesh changes
- **THEN** its key changes, and equal inputs give equal keys

#### Scenario: Finest-level class
- **WHEN** the plan of a view is built with the cap 22 and with the cap 23
- **THEN** the draw tiles at level 22 of the first plan have the class key of their base key, every tile below the cap has its base key, and the same level-22 tile has its base key under cap 23

#### Scenario: Known answers
- **WHEN** the committed meshes are generated
- **THEN** their content hashes equal the committed values, and the test fails if the mesh version differs from the one the values belong to

## ADDED Requirements

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
Status: planned
Source: ADR 0062 point 2, ADR 0063, SPIKE-12 Q3, ADR 0033 points 3-4, GEN-019, GEN-021, CON-11, CON-14

#### Scenario: Offsets and stride
- **WHEN** a slot is built at cells 16, 32 and 64
- **THEN** the strides are 2,768, 9,168 and 34,256 bytes, every stride is a multiple of 16, and the header, climate block, pad and texels sit at the offsets of the table

#### Scenario: One byte string
- **WHEN** a tile's slot is encoded to the pack payload, decoded, uploaded to a block and read back
- **THEN** the four byte strings are equal and equal the CPU builder's, and the shader's reads of header, climate and texel words at the offsets agree with the CPU mirror

#### Scenario: Determinism and version
- **WHEN** the slots of 24 tiles are built on 1, 2 and 4 threads
- **THEN** the content hashes are equal to each other and to the committed known answers of `MESH_VERSION` 8
