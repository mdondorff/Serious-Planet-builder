## ADDED Requirements

### Requirement: STRM-014 Concurrent store source
A stored global bake read as a lattice source (STRM-012) SHALL return, for every vertex, the same bits whatever the number of threads reading it, the order of the reads and the state of its block cache, and SHALL let concurrent readers of already-decoded blocks proceed without a lock on the read path (measured ad hoc, not asserted: the 20-thread rate over the store is at least 80 % of the in-memory rate). Its decoded blocks SHALL be bounded by a byte budget (default 512 MiB, at least one block): blocks resident in the shared cache (including blocks being decoded) SHALL never exceed it. Threads that want the same block that is not resident SHALL share one decode. A block that cannot be read SHALL be reported to every reader that wanted it, SHALL not be cached, and SHALL be read again by readers that arrive after the failed read (readers already waiting for it get its error).

Verify: A
Status: active
Source: STRM-012, CON-14, CON-20, ADR 0027 point 7, ADR 0033, M4 item 7b

#### Scenario: Identical at any thread count
- **WHEN** tiles of the eroded field are generated over a stored bake at 1, 4 and 20 threads
- **THEN** every tile is bit-identical to the others and to the tile over an in-memory bake of the same grid

#### Scenario: One decode per block
- **WHEN** many threads read the same blocks of a cold source whose budget holds them
- **THEN** every block is decoded exactly once

#### Scenario: Budget
- **WHEN** reads touch more blocks than the budget holds
- **THEN** resident bytes never exceed the budget and every read still returns the stored sample

#### Scenario: Missing block
- **WHEN** a block is missing or damaged
- **THEN** every reader gets the error naming the block, its key and why, and nothing is cached
