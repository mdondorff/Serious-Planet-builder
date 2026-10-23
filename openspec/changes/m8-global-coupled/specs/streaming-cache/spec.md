## ADDED Requirements

### Requirement: STRM-043 The coupled run is stored as a channel
The result of the global coupled run (GEN-040) SHALL be stored as a `TileChannel` of kind 7 in blocks of the canonical layout of its lattice (`GlobalWorld::layout(n)`): per cell the height, the drainage area in m2 and the receiver's global cell index, one block per tile, `B x B` cells, as byte-shuffled planes in an LZ4 block. The key SHALL carry the lattice size, the steps, every run parameter, the base resolution, the algorithm version and a hash of the run's inputs, so a changed value gives every block a new key. A block read back SHALL equal what was written bit for bit; a block of another `B`, a short or damaged payload SHALL be an error naming the block.

Verify: A
Status: active
Source: opus-planner (2026-10-23, M8-1 part c), ADR 0077, R5

#### Scenario: Round trip and keys
- **WHEN** a run on a small lattice is written and read back, and a run parameter, the steps, the base resolution or an input changes
- **THEN** the arrays are equal bit for bit, every key differs from the first run's, and a payload cut short or of another `B` is refused naming the block
