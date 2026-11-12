## ADDED Requirements

### Requirement: REND-058 The water field answers a commit's lakes
When the viewer draws a commit (`--erosion`), its water field SHALL list a lake producer after the sea: at a direction whose local cell the commit's lake mask has wet, W is that lake's level from the commit's basin table. The masks and the tables SHALL be read under the commit the layer and the rivers are read under. A lake's body id SHALL be `(1 << 60) | (face << 32) | index`, the face and index of its basin table entry: never the sea's id, never with bit 63 (the bake's labels). The liquid is the sea's.

Verify: A
Status: active
Source: Architect 2026-11-11 (item 15 PR 4 decisions), lead 2026-11-12 (E1), CON-22

#### Scenario: A fixture commit with one lake
- **WHEN** every core cell of the commit's tiles is read
- **THEN** a wet cell of the mask answers the lake's face, index and level, every other cell, a tile the commit does not hold and a cell outside the face are dry

#### Scenario: Body ids
- **WHEN** a lake body is formed for any face and index
- **THEN** it differs from the sea's, has bit 63 clear and gives back its face and index
