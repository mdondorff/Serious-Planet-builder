## MODIFIED Requirements

### Requirement: GEN-010 Mesh keys and known answers
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, the body parameter hash with the radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever vertex output changes for the same inputs, the compact construction (GEN-019) changes or the key composition changes; version 5 introduced the compact tile, which carries the mesh key of its tile, so a tile has one key in either format; version 6 adds the finest-level class (LOD-010): a tile at or deeper than the view's finest level has the key of its tile rehashed with the class, because the format of a finest tile is decided from its relief (GEN-019) and a resident tile is reused by its key, so the key SHALL identify what the format decision depended on; tiles below the finest level keep their base key. The plan's keys, the compact tiles and the vertex tiles of one view SHALL all carry the key of the view's finest level, and a finest tile decided as a fallback under one cap SHALL NOT share a key with the compact tile of the same tile under a deeper cap. Persisted height tiles (STRM-009) are not meshes and are unchanged by it; persistent mesh keys (kind mesh) change with the version, and repro bundles of an older mesh version are refused with the versions named (TEST-008). Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

Verify: A
Status: active
Source: ADR 0006, CON-14, LOD-010

#### Scenario: Key follows inputs
- **WHEN** the seed, tile, grid size or radius of a mesh changes
- **THEN** its key changes, and equal inputs give equal keys

#### Scenario: Finest-level class
- **WHEN** the plan of a view is built with the cap 22 and with the cap 23
- **THEN** the draw tiles at level 22 of the first plan have the class key of their base key, every tile below the cap has its base key, and the same level-22 tile has its base key under cap 23

#### Scenario: Known answers
- **WHEN** the committed meshes are generated
- **THEN** their content hashes equal the committed values, and the test fails if the mesh version differs from the one the values belong to

### Requirement: GEN-019 Compact tile
The tile builder SHALL produce, for levels 3 and finer, a compact tile (ADR 0033 point 4): one 8-byte texel per grid vertex (`(cells + 1)^2` texels: a 16-bit height between the tile's own minimum and maximum, two snorm16 octahedral normal components, 16 reserved bits equal to zero) and a 128-byte record (nine tile-local control points of a biquadratic base surface on the shell through the tile origin, the height range, the origin height, the skirt depth and the orientation flag), 8,840 bytes at cells 32. A tile SHALL have the same key in either format (the mesh key of its finest-level class, GEN-010). The stored normal SHALL equal the canonical CPU normal within 0.01 degrees (CON-14); the quantisation step SHALL be the relief over 65,535 with the range rounded outwards so no height is clamped, the height error SHALL be at most half a step plus f32 rounding, and a finest-level tile (a tile at or deeper than the LOD cap `max_level`, LOD-010) with more than 65.535 m of relief (a step above CON-13's 1 mm) SHALL be built in the vertex format instead, as SHALL every tile of levels 0-2 (the base surface's model error of about 0.018 theta^2 of the tile edge is too large there). The CPU mirror of the shader's vertex stage (position, morph target, skirts, heights, normal) SHALL agree with the canonical CPU mesh at every vertex of tiles of levels 3 to 26 within the model error, the quantisation step and f32 rounding, with the exact morph target (neighbour heights read from the texels), and neighbouring tiles of one level SHALL meet within the model error of the two (millimetres from level 12). Tiles SHALL be bit-identical across runs and thread counts, with known-answer hashes next to the mesh version they belong to. Every value in a tile SHALL be tile-local or a unit vector (CON-11).

Verify: A, B
Status: active
Source: ADR 0033 points 4 and 5, SPIKE-06, CON-11, CON-13, CON-14, LOD-010

#### Scenario: Layout and bytes
- **WHEN** a tile is built at cells 32 and cells 64
- **THEN** it has 1,089 / 4,225 texels of 8 bytes and a 128-byte record (8,712 + 128 = 8,840 B and 33,800 + 128 B), the reserved bits are zero, and its key equals the mesh key of the same tile (of its class)

#### Scenario: Normals
- **WHEN** 200,000 random directions, the axes and the octahedron's fold are encoded and decoded, and the stored normals of tiles of levels 3 to 22 are compared with the CPU mesh
- **THEN** the largest angle is at most 0.01 degrees (measured 0.0037)

#### Scenario: Height step
- **WHEN** tiles of the eroded field at levels 3 to 26 are built
- **THEN** every texel is within half a step (plus f32 rounding) of the exact height, no level is clamped, and from level 16 the step is at most 1 mm

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
