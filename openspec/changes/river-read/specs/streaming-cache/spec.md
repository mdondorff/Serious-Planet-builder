## ADDED Requirements
### Requirement: STRM-085 The runtime reads the channels of a tile through the commit and reports a failed read
The runtime SHALL read the channels that can touch an L tile (the tile's core widened by the widest trench) through the same commit as the L tiles and the basin tables, decode them once while they stay in a least-recently-used cache bounded in bytes, and return an error naming the tile and the commit for a block the commit names and the store lacks or that is damaged; it SHALL NOT stop the process.
- A commit without a channel graph has no reader, and the refusal says so.
- The cache bound is a byte count, a parameter of the reader (a profile value, default 64 MiB), evicting the least recently used tile; the newest tile is always kept.
- An evicted tile read again gives the same channels.

Verify: A
Status: planned
Source: Architect 2026-11-10, item 15 PR 8; A3 section 7.4, CON-22

#### Scenario: A missing block
- **WHEN** the commit names a graph block whose record the store lacks
- **THEN** the read of a tile that block meets returns an error naming the tile and the missing record
