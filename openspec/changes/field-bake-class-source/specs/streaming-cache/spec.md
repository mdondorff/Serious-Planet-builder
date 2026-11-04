## ADDED Requirements

### Requirement: STRM-073 A store records the field source it was baked from
A bake from the class source (GEN-115) SHALL be another bake than the legacy one:
- the class source's identity (`BakeInputs::id()`, covering the class source's and class list's versions, the lattice size and the class tables) and the seed enter the bake's read set as a query entry (kind 6; its parameters are the identity then the seed, little-endian, its result zero; a read-set entry that is not a node of the definition, STRM-011), so the stage key and every key sourced from it differ from the legacy world's and from another seed's;
- a world that does not ask for the class source keeps every key it had;
- `GlobalWorld::check_source(store, res)` SHALL report `Ok(true)` when the store holds the first block of the bake under this world's key, `Ok(false)` when it holds no block of that tile, and an error naming the expected key, the key found and the source this world samples when it holds the tile under another key (another source, seed or version): refused, never mixed (ADR 0006);

Verify: A
Status: planned
Source: owner 2026-11-04 (planner inbox, change `field-bake-class-source`); STRM-065, ADR 0006

#### Scenario: Keys
- **WHEN** the legacy world, the class world and another seed's class world are asked for their stage and block keys
- **THEN** the three differ, the same world asked twice gives the same key, and the legacy world's stage key is the pinned value it has always had

#### Scenario: Stores
- **WHEN** a class store is built on 1 and 4 threads and read with the legacy world and with another seed's class world, and a store with no blocks is read
- **THEN** the packs are equal on both thread counts, the stored block is the class field's, the readers of another source or seed get an error naming both keys, and the empty store reads as absent

#### Scenario: The command
- **WHEN** `generate --global-dir --class-source` is run twice and `--verify-global` with and without `--class-source`
- **THEN** the second run writes no block, the verify with it matches every entry, and the verify without it compares nothing
