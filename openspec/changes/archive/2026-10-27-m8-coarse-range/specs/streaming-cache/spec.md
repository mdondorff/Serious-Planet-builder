## MODIFIED Requirements

### Requirement: STRM-045 The coarse horizon is stored as a channel
The coarse horizon (GEN-045) SHALL be stored as a `TileChannel` of kind 9 in blocks of the canonical layout of its observer lattice (`GlobalWorld::layout(m)`): per observer cell the record of GEN-045 (`2 a + 2` bytes for `a` azimuths), one `B x B` block per tile, as an LZ4 block with the record length in its header. The key SHALL carry the observer lattice size, every parameter of the march and its version, and the whole coupled run it is computed from (STRM-043's spec and inputs hash), so a changed value gives every block a new key and no key equals a coupled-run block's.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), STRM-043

Detail: A block read back SHALL equal what was written bit for bit; a block of another `B`, another azimuth count, a short or damaged payload, or one missing under the current key SHALL be an error naming the block. A world with a store SHALL read the stored channel on a thread of its own when it gets its chunk stage and never wait for it on the frame path (CON-22): a world without a stored map shades with the chunk horizon alone and says once how to make it (`planet generate --coarse-horizon <store dir> --global-res N`, which reads the stored coupled run, marches and stores the channel, printing a progress line and its wall time); a stored map that the renderer cannot read (not 16 azimuths, fewer than 2 cells a face edge) is a failure naming the counts.

#### Scenario: Round trip and keys
- **WHEN** the coarse horizon of a small run is written and read back, and a march parameter, the observer lattice or the coupled run changes
- **THEN** the map is equal bit for bit, the block of a tile holds the observer cells under it, every key differs from the first one's and from the coupled run's, and a payload cut short, of another `B` or azimuth count, or missing is refused naming the block
