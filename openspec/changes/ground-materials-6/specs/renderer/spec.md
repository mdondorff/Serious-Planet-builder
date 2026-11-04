## ADDED Requirements
### Requirement: REND-056 Scree and wet ground as material slots
The ground materials (REND-034, LOOK-008) SHALL be six slots, rock, sand, grass, soil, scree and wet ground; the weights SHALL be normalised over all six, the roughness (LOOK-010) and the octave coefficients (LOOK-013) SHALL each have an entry for the two new slots, and the look file SHALL take their curve tables and their `scree` and `wet` octave entries (look schema version 8). The Earth default SHALL have both new slots off, and a look file of an older version SHALL read with them off.
With both slots off the picture SHALL be what it was, bit for bit.

Verify: A, C
Status: planned
Source: SPIKE-100 D1, planner 2026-11-04 (I8a slice 2a, step b), REND-034, LOOK-013

#### Scenario: Six slots
- **WHEN** a look file gives scree and wet curves and octave entries, a bad number, a wrong count and an unknown key, a build that knows version 7 reads the version 8 fixture, and the version 7 fixture is read; and scenes with only scree, only wet ground and only soil are drawn, each octave entry is raised in turn, and the goldens of the lit ground are drawn
- **THEN** the table reads and writes back equal, the bad ones are refused naming the key, the older build refuses the file as a newer schema, the version 7 file reads with both slots off, the three scenes differ from each other and from the plain picture, wet ground is darker than scree, each raised entry changes the picture, and the goldens need no candidate
