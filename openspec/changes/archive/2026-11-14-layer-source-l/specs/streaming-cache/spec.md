## ADDED Requirements
### Requirement: STRM-083 The runtime reads L through a commit and draws one commit at a time
The runtime SHALL read the L tiles of the new chain through the commit their bake wrote, as the surface change `L` with nothing added back, and SHALL swap the drawn commit only when every needed part of the new one is ready.
- A tile the commit does not name is a tile with no land; a named tile the store lacks, a key other than the commit spec's, a damaged record or a change outside the layer's range is an error naming the tile.
- Tiles are decoded once while they stay in a bounded cache; the layer's identity is the commit id.
- A new commit becomes the drawn one when the tiles the caller needs of it read and its basin tables are in the store; until then the previous commit is drawn, and a frame reads one commit only.

Verify: A
Status: planned
Source: Architect 2026-11-10, A3 section 7.4, CON-22

#### Scenario: Swap
- **WHEN** a new commit is offered with a table or a tile not yet in the store, and then complete
- **THEN** the previous commit stays drawn until the new one is complete, and a frame in flight keeps reading the previous commit
