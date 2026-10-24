## ADDED Requirements

### Requirement: STRM-043 The coupled run is stored as a channel
The result of the global coupled run (GEN-040) SHALL be stored as a `TileChannel` of kind 7 in blocks of the canonical layout of its lattice (`GlobalWorld::layout(n)`): per cell the height, the drainage area in m2 and the receiver's global cell index, one `B x B` block per tile, as byte-shuffled planes in an LZ4 block. The key SHALL carry the lattice size, the steps, every run parameter, the base resolution, the algorithm version and a hash of the run's inputs, so a changed value gives every block a new key. A block read back SHALL equal what was written bit for bit; a block of another `B`, a short or damaged payload SHALL be an error naming the block.

Verify: A
Status: active
Source: opus-planner (2026-10-23, M8-1 part c), ADR 0077, R5

#### Scenario: Round trip and keys
- **WHEN** a run on a small lattice is written and read back, and a run parameter, the steps, the base resolution or an input changes
- **THEN** the arrays are equal bit for bit, every key differs from the first run's, and a payload cut short or of another `B` is refused naming the block

### Requirement: STRM-044 The world loads the stored coupled run or makes it
A world SHALL take its coupled run (GEN-040) from the stored channel (STRM-043) beside its bake when that holds the run, and otherwise make the run and write it there; a world over an in-memory bake makes it and stores nothing. The run's input is the bake's elevation at every cell centre of the lattice (the bake's resolution) with a constant uplift and erodibility, and its spec (lattice, steps, bake resolution, parameters, hash of the input) is the key of STRM-043, so another input, step count or parameter is another run and never reads the stored one. The load or run SHALL happen once on a thread, never in a frame (CON-22); until it is ready the fields stay plain, and a failure is a message naming the cause, never a default.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 part c), ADR 0077

#### Scenario: Made once, then loaded
- **WHEN** the run of a small world is asked for twice from a store directory, then once with another step count
- **THEN** the first call makes and stores it, the second reads it back with identical maps, the third makes another run with another identifier, and a world without a store makes the run in memory
