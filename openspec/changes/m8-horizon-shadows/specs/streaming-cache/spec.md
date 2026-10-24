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
The chunk's horizon map (GEN-044) SHALL be a second job of the chunk that lands after its heights, stored on the disk cache under the chunk key plus the horizon parameters, evicted with its chunk and refreshed when its chunk lands again. A tile whose chunk horizon is not ready SHALL draw with the coarse horizon alone (CON-22) and the chunk horizon SHALL fade in when it lands; an edit that invalidates a chunk invalidates its horizon map, and an edit re-queues the horizon job of every chunk whose halo it touches (precise handling in M8-7).

Verify: A
Status: planned
Source: opus-planner (2026-10-24, M8-5), STRM-038 to STRM-041, CON-22

#### Scenario: Stand-in and landing
- **WHEN** a plan's chunk has its heights and no horizon map yet, and then the map lands
- **THEN** the tile reads the coarse horizon alone and the chunk's map is requested, then the chunk's map is used with a fade, and an edit in the halo re-queues the job
