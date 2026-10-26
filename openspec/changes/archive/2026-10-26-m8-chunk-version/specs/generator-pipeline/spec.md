## MODIFIED Requirements

### Requirement: GEN-039 The chunk stage's position per layer
A layer SHALL have a `chunk_stage` setting, `Under` by default for a world with a chunk stage: the chain is base, stamps, chunk, and an edit changes the version of the chunks whose read set (GEN-081) it touches (GEN-082). `OnTop` (base, chunk, stamps) SHALL stay representable in the key; an edit then never invalidates a chunk.

Verify: A
Status: active
Source: M8 plan D3 (owner, 2026-10-26), ADR 0077 point 7, m8-chunk-version

#### Scenario: Default and refusal
- **WHEN** a world with a chunk stage and no setting is built
- **THEN** its layer's chain is base, stamps, chunk (nothing is refused), and `OnTop` set explicitly gives base, chunk, stamps

## ADDED Requirements

### Requirement: GEN-081 The read set of a chunk
`chunk_read_set` SHALL return the discs outside which no item can change a chunk's bits: the chunk's region (its square plus the halo of GEN-049) and the routed network it reads, that is the segments the pruning of GEN-078 keeps (one `region_segments`, one `segments_reaching`) and those upstream of them along the main stem up to the law's window, each widened by `routing_reach_m` along its chord. The set SHALL be a function of the chunk key, the global run and the stage parameters only, never of routed paths or items, so it cannot depend on the edits it selects.

Verify: A
Status: active
Source: m8-chunk-version, GEN-049, GEN-059, GEN-078

#### Scenario: The set covers what the network reads
- **WHEN** an item's reach touches only the routing reach of a kept segment, outside the halo
- **THEN** it is inside the read set and the chunk's bits change with it; an item outside every disc changes no bit, and the set takes no item as input

### Requirement: GEN-082 The version of a chunk
A chunk's version SHALL be the hash of the items of the `Under` chain whose reach touches its read set (GEN-081), the empty hash for none. It SHALL be part of the chunk's disk key. A chunk re-solved after an edit SHALL equal, bit for bit, a cold solve of the same version (the solve itself does not depend on the thread count, GEN-077).

Verify: A
Status: active
Source: m8-chunk-version, GEN-039, CON-14

#### Scenario: A re-solve equals a cold solve
- **WHEN** a chunk is solved, a hill is added inside the halo, and the chunk is solved again
- **THEN** its version differs, and its bits equal those of a cold solve of the new definition; with the hill outside the read set the version and bits are those of the first solve
