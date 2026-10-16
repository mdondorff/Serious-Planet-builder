## MODIFIED Requirements

### Requirement: GEN-020 Batch evaluation is order independent
A height field SHALL offer a batch evaluation of a list of directions that returns, for each direction and in the caller's order, exactly the value `height` returns for it (bit for bit). A field that reads a block-structured lattice source SHALL gather the lattice cells the whole batch reads, read them from the source grouped by block, and evaluate from those cells, so that a tile decodes each block it touches once, whatever the source's cache holds, instead of once per cache miss in raster order. The tile builders (the vertex mesh and the compact tile) SHALL obtain every height they need (lattice samples, normal stencil points, the origin) through one batch call, and the tile SHALL be bit-identical in raster order and in block order, at any thread count. The batch SHALL poll its stop flag before each block group, every 64 directions of the footprint pass, every 64 cells of the gather and every 64 samples and read nothing once it is set. The batch SHALL also be available on up to `threads` threads (`heights_threaded`): the distinct blocks are dealt to the threads, each thread reading the cells of one block at a time, and the evaluation from the gathered cells is split over the threads; the values and the number of block reads SHALL NOT depend on `threads`, and a batch of two directions or more is gathered. No key, bake format, mesh version or golden changes.

Verify: A
Status: active
Source: opus-planner decision of 2026-10-15 (delegation), STRM-014, STRM-005, GEN-017, CON-14, M4 fix F3 (LOD-009)

#### Scenario: Bit identity
- **WHEN** a tile (a level-0 root, a coarse tile spanning many blocks, a fine tile, a tile on a face edge, one on a cube corner) is built in raster order and in block order, over the eroded field on an in-memory bake and on a source with a small cache (1 and 20 threads), and on a stored multi-block bake with a cache of one block against an unbounded one (the store at 1, 4 and 20 threads is also STRM-014's)
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
