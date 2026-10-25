## ADDED Requirements

### Requirement: LOD-013 Chunk requests and bounds in the plan
The plan builder SHALL return the set `B` of chunks under its nodes at or finer than the pyramid's top level, in plan order, beside the draw set D and the residency set R. The height bounds of a chunk field SHALL widen by the field's stated bound whether or not a chunk is ready, so a plan with no chunk ready equals the plan with all ready, and no node SHALL be refused because of a chunk.

Verify: A
Status: active
Source: SPIKE-34 sections 1 and 4, STRM-016, LOD-006, LOD-008

#### Scenario: Ready or not
- **WHEN** the same position is planned with no chunk ready and with every chunk of `B` ready (stand-in field, bound 60 m)
- **THEN** the two plans are identical, `B` holds only chunks under nodes at or finer than the top level, and no node is refused

### Requirement: LOD-014 Origins of a field with chunk detail
For a field with chunk detail the plan SHALL evaluate the origin heights of the tiles its cache lacks as one batch per tile level, each as `height_at_level` of that level (GEN-054), so a tile's origin is the height the tile is drawn at; a field without chunk detail SHALL keep its one batch per plan (LOD-009). The origin heights and mesh keys the cache holds of the tiles that read a landed chunk, resident or not, SHALL be dropped (STRM-040).

Verify: A
Status: active
Source: LOD-009, GEN-054, STRM-040

#### Scenario: Landing
- **WHEN** a chunk lands after a plan of a field with chunk detail
- **THEN** exactly the cached tiles that read it lose their origin heights, and the next plan equals the plan of a fresh cache
