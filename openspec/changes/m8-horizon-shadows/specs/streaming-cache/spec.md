## ADDED Requirements

### Requirement: STRM-045 The horizon maps are stored and follow their chunk
The coarse horizon (GEN-045) SHALL be stored as a `TileChannel` of its own kind in blocks of the canonical layout of the coupled run's lattice, keyed like STRM-043 plus the horizon parameters (azimuths, range, step law), so a changed value gives every block a new key. The chunk's horizon map (GEN-044) SHALL be an output of the chunk, stored on the disk cache under the chunk key plus the horizon parameters, evicted with its chunk, and refreshed when its chunk lands again. A tile whose chunk horizon is not ready SHALL draw with the coarse horizon alone (CON-22); an edit that invalidates a chunk invalidates its horizon map.

Verify: A
Status: planned
Source: opus-planner (2026-10-24, M8-5), STRM-038 to STRM-043, CON-22

#### Scenario: Keys and stand-in
- **WHEN** a chunk's horizon map is written and read back, and a horizon parameter changes
- **THEN** the map is equal bit for bit, every key differs after the change, and a plan whose chunk horizon is missing reads the coarse horizon alone and asks for the chunk's map
