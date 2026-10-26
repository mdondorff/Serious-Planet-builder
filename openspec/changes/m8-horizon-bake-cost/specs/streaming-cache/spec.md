## ADDED Requirements

### Requirement: STRM-059 Chunk horizons share height tiles
The chunk horizon baker (STRM-049) SHALL assemble the height raster of a chunk from chunk-sized blocks of heights that chunks share in a bounded cache, and the map SHALL be byte-identical to one baked from a raster evaluated whole.

Verify: A
Status: active
Source: opus-planner (2026-10-26, criterion 6), STRM-048, STRM-049, GEN-085, ADR 0083 point 4

Detail: Blocks inside the face are kept; a block past a face edge is evaluated each time. The cache is bounded (`HEIGHT_TILES_MAX`, 64: 96.5 MB at 614 cells) and evicts the least recently used. A block is kept under a key of everything its heights depend on: the bake version, `ANET_VERSION`, the chunk stage version, the lattice, the world's baked input, the definition apart from its items, the coupled lens and its readiness, the items that touch the block with the stamp stage's halo, and the hash of its ready result. A block whose chunk result changed while the field was built SHALL NOT be kept.

#### Scenario: Neighbours and edits
- **WHEN** a chunk and then its eastern neighbour are baked, a hill is put in the neighbour, and the chunk is baked again
- **THEN** the second bake evaluates the three blocks it lacks and reads six, the third evaluates the blocks the hill reaches and no others, and every map equals the uncached one
