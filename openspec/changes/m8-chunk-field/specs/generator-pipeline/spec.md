## ADDED Requirements

### Requirement: GEN-034 Chunk scope and key
`Scope` SHALL have a `Chunk(ChunkId)` variant, distinct from `Global` and `Tile` in the key (its own scope word), where a `ChunkId` is a face and the (i, j) of a tile of the chunk grid's level. A chunk field's stage key (STRM-011) SHALL cover the grid level, the halo in metres and the step cap, so changing any of them re-keys every chunk of the field; a regional stage with `Chunk` scope SHALL fit, and one with `Global` scope SHALL not.

Verify: A
Status: planned
Source: ADR 0077 points 2 and 4, ADR 0022, STRM-011, CON-08, CON-14

#### Scenario: Re-keying
- **WHEN** a chunk field's grid level, halo (bits) or step cap changes, or a tile and a chunk have the same raw numeric id
- **THEN** exactly the keys of that field's chunks change, and the tile and the chunk never share a key

### Requirement: GEN-035 Chunk grid and apply level
Chunks SHALL lie on a fixed grid of the tiles of level 11 (whole coarse cells, about 5 km), and a chunk field SHALL state its raster cell size; its apply level SHALL be the coarsest tile level whose vertex spacing is at most twice that cell size (level 15 for 8 m cells, level 16 for 4 m), computed from the raster and never set by hand. The chunk pyramid SHALL reach from the apply level up to a top level, by default the chunk grid level (11); a tile above the top level SHALL be identity on the base field and key as the base (STRM-029), the global run carrying the large scale. A tile between the top level and the apply level reads a box-filtered pyramid level (GEN-037).

Verify: A
Status: planned
Source: ADR 0077 point 5, SPIKE-34 (conditions 1 and 3), STRM-029

#### Scenario: Apply level
- **WHEN** a chunk field has 8 m cells, then 4 m cells
- **THEN** its apply level is 15, then 16, and a tile of level 10 returns the base height bit for bit at every vertex with every chunk ready, while a tile of level 11 to 14 reads the pyramid

### Requirement: GEN-036 Level-aware height
The height capabilities of a field SHALL include `height_at_level(dir, level)`; for a field without chunk detail it SHALL equal `height(dir)` at every level, and for a chunk field it SHALL be the base plus the chunk change of the pyramid level whose spacing matches `level` at or above the apply level, and the base below it. `height` SHALL remain position-only and SHALL equal `height_at_level` at the field's finest level. Tile generation SHALL read `height_at_level` with its own level.

Verify: A
Status: planned
Source: SPIKE-34 section 3, ADR 0077 point 5, GEN-020

#### Scenario: Default
- **WHEN** every existing field is sampled with `height_at_level` at levels 0 to 22
- **THEN** each value equals `height(dir)` bit for bit

#### Scenario: Chunk field
- **WHEN** a chunk field with a ready chunk is sampled at levels 14 and 15 over the chunk
- **THEN** level 14 equals the base and level 15 equals the base plus the chunk's level-15 pyramid change

### Requirement: GEN-037 Chunk pyramid steps
A chunk's change SHALL be stored with a pyramid down to the apply level, each level box-filtered from the level below it (never from a coarser run). A tile at or above the apply level SHALL differ from its parent, at the parent's vertices, by at most 0.25 m at p99 for the stand-in compute.

Verify: A
Status: planned
Source: SPIKE-34 section 3, M8 plan criterion 3

#### Scenario: Step against the parent
- **WHEN** the stand-in chunk (30 m amplitude, 8 m cells) is ready and every tile of levels 14 and 15 over it is generated
- **THEN** the height difference between a level-15 tile and its level-14 parent at the parent's vertices has p99 at most 0.25 m, and the test prints the p99, the maximum and the tile ids of the worst vertex on failure

### Requirement: GEN-038 Chunk border agreement and determinism
Neighbouring chunks SHALL agree on every shared border sample, across cube-face edges and at corners, and a chunk's result SHALL be bit-identical for 1, 2 and 8 threads and on Windows and Linux (fixed order, `libm`). A chunk field SHALL state its halo in metres as at least its step cap times its reach per step.

Verify: A
Status: planned
Source: ADR 0077 points 3 and 4, GEN-004, GEN-002, GEN-003, CON-14, M8 plan criterion 2

#### Scenario: Borders
- **WHEN** two neighbouring stand-in chunks are computed with the stated halo, in the middle of a face, across a face edge and at a pole
- **THEN** their border rows agree to the bit, and with a halo below the rule the test reports the largest difference in metres and the chunk ids

#### Scenario: Threads and systems
- **WHEN** a chunk is computed on 1, 2 and 8 threads
- **THEN** its hash is identical and equals the recorded known answer, which CI checks on Windows and Linux

### Requirement: GEN-039 The chunk stage's position per layer
A layer SHALL have a `chunk_stage` setting, `OnTop` by default: the chain is base, chunk, stamps, so an edit never invalidates a chunk. `Under` (base, stamps, chunk) SHALL be representable in the key but refused with an error naming the layer until the editor supports it (M8-7, after the owner's confirmation of D3).

Verify: A
Status: planned
Source: M8 plan D3 (provisional), ADR 0077 point 7

#### Scenario: Default and refusal
- **WHEN** a world has a layer without the setting and one set to `Under`
- **THEN** the first builds with `OnTop`, and the second is refused with an error naming the layer
