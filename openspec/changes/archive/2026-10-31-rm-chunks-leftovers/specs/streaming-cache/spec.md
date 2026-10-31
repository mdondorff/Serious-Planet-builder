## MODIFIED Requirements

### Requirement: STRM-045 The coarse horizon is stored as a channel
The coarse horizon (GEN-045) SHALL be stored as a `TileChannel` of kind 9 in blocks of the canonical layout of its observer lattice (`GlobalWorld::layout(m)`): per observer cell the record of GEN-045 (`2 a + 2` bytes for `a` azimuths), one `B x B` block per tile, as an LZ4 block with the record length in its header. The key SHALL carry the observer lattice size, every parameter of the march and its version, and the whole coupled run it is computed from (STRM-043's spec and inputs hash), so a changed value gives every block a new key and no key equals a coupled-run block's.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), STRM-043

Detail: A block read back SHALL equal what was written bit for bit; a block of another `B`, another azimuth count, a short or damaged payload, or one missing under the current key SHALL be an error naming the block. A world with a store SHALL read the stored channel on a thread of its own when it gets its horizon jobs and never wait for it on the frame path (CON-22): a world without a stored map shades with the chunk horizon alone and says once how to make it (`planet generate --coarse-horizon <store dir> --global-res N`, which reads the stored coupled run, marches and stores the channel, printing a progress line and its wall time); a stored map that the renderer cannot read (not 16 azimuths, fewer than 2 cells a face edge) is a failure naming the counts.

#### Scenario: Round trip and keys
- **WHEN** the coarse horizon of a small run is written and read back, and a march parameter, the observer lattice or the coupled run changes
- **THEN** the map is equal bit for bit, the block of a tile holds the observer cells under it, every key differs from the first one's and from the coupled run's, and a payload cut short, of another `B` or azimuth count, or missing is refused naming the block

### Requirement: STRM-049 The world bakes the chunk horizon
A world with a chunk lattice (one that reads a bake) SHALL be able to enable the horizon jobs (`WorldTerrain::enable_horizon`, refused without the lattice) with a baker that samples the world's own field over the chunk and a halo and bakes its horizon (GEN-044). `horizon_frame` SHALL run one frame of the jobs for the chunks the plan's tiles read, nearest first, and `horizon_map` SHALL return the landed map or none. The same world SHALL give the same map.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), STRM-048, STRM-026

Detail: A world with a chunk lattice SHALL be able to enable the horizon jobs (`WorldTerrain::enable_horizon`, refused without the lattice), with a baker that samples the world's own field (the base and the stamps) at the cell directions of the chunk lattice over the chunk and a halo of `ceil(range / cell) + 2` cells (past a face edge on the extended cube plane), bakes the horizon of the chunk's cells (GEN-044) and reads the chunk and the chunks around it, nine in all and eight at a cube corner (STRM-048's state word), which holds only while the halo fits in one chunk edge: `enable_horizon` refuses a range whose halo is longer, and a march the bake would refuse, naming the cause (`chunk_horizon_params` gives the world's own, the range cut to a chunk edge less two cells). A world that is gone or a field that cannot be built gives a map of zeros of the right length, never a short one. `horizon_frame` SHALL run one frame of the jobs for the chunks the plan's tiles read, nearest first, and `horizon_map` SHALL return the landed map or none. The march's hash SHALL carry the lattice and the bake version; the edits enter through the chunk's edit word (STRM-057). The same world SHALL give the same map. The cell is the chunk lattice's (8 m at the default).

#### Scenario: A flight
- **WHEN** a world over the base terrain with a small lattice flies 12 s on a fake clock, then stands still, then gets an edit
- **THEN** every ready chunk has its map after its heights, the maps are one byte per azimuth per cell, the same world gives the same maps, the re-bakes stay within two per chunk, an unchanged world bakes nothing more, and an edit at the chunks bakes the maps it touches again

### Requirement: STRM-046 The viewer draws the base terrain only
No app code computes detail on top of the base terrain (ADR 0086), so every tile the viewer, the editor and the scenes draw SHALL be the base. No frame SHALL wait for a bake (CON-22).

Verify: A
Status: active
Source: D7 (runtime generation, disk cache), ADR 0077 point 7, STRM-043, owner fallback 2026-10-28, ADR 0086

#### Scenario: Off by default
- **WHEN** the window opens a world with a stored coupled channel
- **THEN** it starts no job but the tile and horizon jobs and computes nothing on top of the base

### Requirement: STRM-059 Chunk horizons share height tiles
The chunk horizon baker (STRM-049) SHALL assemble the height raster of a chunk from chunk-sized blocks of heights that chunks share in a bounded cache, and the map SHALL be byte-identical to one baked from a raster evaluated whole.

Verify: A
Status: active
Source: opus-planner (2026-10-26, criterion 6), STRM-048, STRM-049, GEN-085, ADR 0083 point 4

Detail: Blocks inside the face are kept; a block past a face edge is evaluated each time. The cache is bounded (`HEIGHT_TILES_MAX`, 64: 96.5 MB at 614 cells) and evicts the least recently used. A block is kept under a key of everything its heights depend on: the bake version, the two version numbers kept from the removed code (10 and 1, so no stored map is lost), the lattice, the world's baked input, the definition apart from its items, the coupled lens and its readiness, and the items that touch the block with the stamp stage's halo. Limit: the position setting of a layer outside `LAYER_STACK` is not in the key.

#### Scenario: Neighbours and edits
- **WHEN** a chunk and then its eastern neighbour are baked, a hill is put in the neighbour, and the chunk is baked again
- **THEN** the second bake evaluates the three blocks it lacks and reads six, the third evaluates the blocks the hill reaches and no others, and every map equals the uncached one

### Requirement: STRM-064 The horizon maps are baked from the base terrain
The horizon jobs SHALL bake the maps of the chunks of the plan's tiles (chunk-grid level 11 and finer) from the base terrain and the stamps, in the format, march and slots of STRM-048, and SHALL bake a map again when an edit or a change of the world word reaches it.

Verify: A
Status: active
Source: owner fallback 2026-10-28, opus-planner M8-F step 2, STRM-048, STRM-060, finding 16 of the owner test 2026-10-27

Detail: Every chunk counts as ready with no change. The bakes run on a pool of their own in the idle class, at most 8 at once (half the generation workers) and one thread a bake: a map takes about 6 s on one thread and the march scales poorly inside a map (4 threads: 1.5x) and linearly across maps; bakes that need the same height tile at once wait for the first to evaluate it. The result does not depend on how many bakes run at once (CON-14). The jobs keep the maps of the wanted chunks (at most 24) and, of the others, the most recently wanted up to the configured number beyond them. An edit that reaches a chunk or the halo of its march, a neighbour included, changes its edit word; a change of the world word (the coupled lens, the base of the definition, the stage) changes every word; the map is baked again once its word has been unchanged for the settle window. A height tile of the bake is kept only if no edit was published since the field it was evaluated from was built.

#### Scenario: Base-only maps
- **WHEN** the horizon jobs run over the base terrain and two chunks are wanted
- **THEN** both start at once, land, and are baked again after an edit word change and the settle window; maps of chunks no longer wanted go beyond the cap, the oldest first

#### Scenario: Window without a stage
- **WHEN** the window runs lit over a store
- **THEN** `chunk jobs` stays `None` and the horizon jobs bake at least one map (judged by the real-store gate `cargo xtask gate`; the small-world run reports it, because a software adapter is too slow to wait for a bake)
