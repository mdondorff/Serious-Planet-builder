## ADDED Requirements

### Requirement: STRM-059 Chunk horizons share height tiles
The chunk horizon baker (STRM-049) SHALL assemble the height raster of a chunk from chunk-sized blocks of heights that chunks share in a bounded cache, and the map SHALL be byte-identical to one baked from a raster evaluated whole.

Verify: A
Status: active
Source: opus-planner (2026-10-26, criterion 6), STRM-048, STRM-049, GEN-085, ADR 0083 point 4

Detail: Blocks inside the face are kept; a block past a face edge is evaluated each time. The cache is bounded (`HEIGHT_TILES_MAX`, 64: 96.5 MB at 614 cells) and evicts the least recently used. A block is kept under a key of everything its heights depend on: the bake version, `ANET_VERSION`, the chunk stage version, the lattice, the world's baked input, the definition apart from its items, the coupled lens and its readiness, the items that touch the block with the stamp stage's halo, and the hash of its ready result. A block whose chunk result changed while the field was built SHALL NOT be kept. The result part of the key is that of every chunk the block's heights read (its own and the neighbours whose cells the spline reads across its edge). Limit: the position of the chunk stage of a layer outside `LAYER_STACK` is not in the key.

#### Scenario: Neighbours and edits
- **WHEN** a chunk and then its eastern neighbour are baked, a hill is put in the neighbour, and the chunk is baked again
- **THEN** the second bake evaluates the three blocks it lacks and reads six, the third evaluates the blocks the hill reaches and no others, and every map equals the uncached one

## MODIFIED Requirements

### Requirement: STRM-033 The RAM share of terrain is capped by counts
Terrain's RAM share SHALL be 3 GiB of the 8 GiB of CON-21. Its parts SHALL be capped by counts asserted at tier A (CON-20): the bake blocks alive at most 0.5 GB (20 blocks of 1024 cells while baking; the runtime reader's cache holds at most 16 blocks), the tile pack index at most 1 GB, staged and in-flight tile bytes at most 0.1 GB, plan caches at most 0.1 GB, the chunk horizon's height tiles at most 64 (96.5 MB at 614 cells, STRM-059), and generated but deferred tiles at most 2,048. The process working set is measured in the dive perf scene (TEST-019), not in a test.

Verify: A
Status: planned
Source: ADR 0033 point 8 (accepted 2026-10-16), opus-planner (2026-10-22, M7 H2: a cap a test asserts becomes active; the rest stay planned), STRM-059

#### Scenario: A cap
- **WHEN** a part of the share reaches its cap
- **THEN** the count stops at the cap (blocks evicted, tiles deferred or refused) and the test names the part, its count and its cap
