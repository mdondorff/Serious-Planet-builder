## ADDED Requirements

### Requirement: LOD-013 Chunk requests and bounds in the plan
The plan builder SHALL return the set `B` of chunks under its nodes at or above the apply level, in plan order, beside the draw set D and the residency set R. The height bounds of a chunk field SHALL widen by the field's stated bound whether or not a chunk is ready, so a plan with no chunk ready equals the plan with all ready, and no node SHALL be refused because of a chunk.

Verify: A
Status: planned
Source: SPIKE-34 sections 1 and 4, STRM-016, LOD-006, LOD-008

#### Scenario: Ready or not
- **WHEN** the same position is planned with no chunk ready and with every chunk of `B` ready (stand-in field, bound 60 m)
- **THEN** the two plans are identical, `B` holds only chunks under nodes at or above the apply level, and no node is refused
