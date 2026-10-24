## ADDED Requirements

### Requirement: STRM-045 The coarse horizon is stored as a channel
The coarse horizon (GEN-045) SHALL be stored as a `TileChannel` of kind 9 in blocks of the canonical layout of its observer lattice (`GlobalWorld::layout(m)`): per observer cell one byte per azimuth, one `B x B` block per tile, as an LZ4 block with the azimuth count in its header. The key SHALL carry the observer lattice size, every parameter of the march and its version, and the whole coupled run it is computed from (STRM-043's spec and inputs hash), so a changed value gives every block a new key and no key equals a coupled-run block's. A block read back SHALL equal what was written bit for bit; a block of another `B`, another azimuth count, a short or damaged payload, or one missing under the current key SHALL be an error naming the block.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), STRM-043

#### Scenario: Round trip and keys
- **WHEN** the coarse horizon of a small run is written and read back, and a march parameter, the observer lattice or the coupled run changes
- **THEN** the map is equal bit for bit, the block of a tile holds the observer cells under it, every key differs from the first one's and from the coupled run's, and a payload cut short, of another `B` or azimuth count, or missing is refused naming the block

### Requirement: STRM-048 The chunk horizon follows its chunk
`streaming::chunks::HorizonJobs` SHALL run the horizon map of a chunk (GEN-044) as a second job that starts only once the chunk's heights are ready, on the injected spawner, at most `workers` at once, nearest first, and never waits for one (CON-22): until its map lands a tile reads the coarse horizon alone (`horizon(key)` is `None`). Each map carries a state word, the hash of the march and of the results of the chunk and of every chunk its bake reads (0 for one that is not ready). A map whose word differs from the current one is baked again, with the state word checked at dispatch and not at each landing, and only once the word has been unchanged for `settle_ms` of the injected clock, so the landing of eight neighbours one after another costs one re-bake, not eight; the number of re-bakes is in the stats. A map is dropped when its chunk leaves RAM. With a disk cache a map is looked up by its tile and its state word before it is baked, and stored after: a map made against other neighbours, or another march, is a miss, and a failing entry is a logged miss.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5; coalescing added the same day), STRM-038 to STRM-041, CON-22

#### Scenario: Stand-in, landing and coalescing
- **WHEN** a chunk has its heights, then its horizon job runs, then its eight neighbours land one every 100 ms, then the jobs are asked after the settle window
- **THEN** no job starts before the heights, the update returns with the job waiting, the tile reads no map until it lands, the neighbours' landings start no bake until the neighbourhood has settled, and then one re-bake reads all nine chunks (two bakes, one re-bake in all)

#### Scenario: Eviction, the march and the disk
- **WHEN** a chunk is evicted, the march changes, and a second world starts from the same disk cache
- **THEN** the chunk's map is dropped with it, a map of another march is not taken for the first, the same chunk with the same neighbourhood reads its map from the disk with no bake, and one with another neighbourhood is a miss

### Requirement: STRM-049 The world bakes the chunk horizon and keeps it on the pack
The world SHALL give `HorizonJobs` a baker that samples the chunk field's heights (the base and the changes of the chunks that are ready) over the chunk and its halo on the chunk lattice and bakes the horizon (GEN-044), and a disk cache on the tile pack keyed by tile and state word; the landing of a map refreshes the tiles of its chunk (STRM-026) and the lookup of REND-038 reads it. An edit re-queues the horizon job of every chunk whose halo it touches (precise handling in M8-7).

Verify: A
Status: planned
Source: opus-planner (2026-10-24, M8-5), STRM-048, STRM-026

#### Scenario: A flight
- **WHEN** a scripted flight lands the chunks around the camera on a fake clock with the stand-in stage
- **THEN** every chunk's map lands after its heights, the re-bake count stays at one or two per chunk, and a chunk read back from the pack in a second run bakes nothing
