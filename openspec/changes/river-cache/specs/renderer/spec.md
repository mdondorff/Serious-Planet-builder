## ADDED Requirements
### Requirement: REND-064 The river coverage of a node is cached by its tile
The coverage of a terrain node SHALL be a function of its tile, the river request and the channels alone, kept in a byte-bounded least-recently-used cache, so that a frame's river plan is a lookup and equals the fresh plan bit for bit.
- Negative results (a node with no river) are cached; a tile larger than the bound is not kept.
- The cache is cleared when the source's identity, the grid, the width law, the limits or the node size change.
- A lookup-only plan lists the tiles not built yet and counts the nodes not ready; such a node draws no river until it is built.

Verify: A
Status: active
Source: Architect 2026-11-12 (river-flight smoke), REND-059, CON-30

#### Scenario: A flight
- **WHEN** a flight's nodes are planned through the cache, again after evictions, and by lookup then fill
- **THEN** every plan equals the fresh plan, the second pass builds nothing and the byte bound holds
