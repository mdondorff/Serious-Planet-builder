## ADDED Requirements
### Requirement: REND-062 The viewer plans the rivers of every lit frame from its river source
When the stored world holds a river source (STRM-086), the viewer SHALL set the lit plan's `rivers` each frame from `plan_rivers` over the terrain plan's nodes, with the river law of the bake (`WidthLaw::STANDARD`), the minimum and full samples `MIN_RIVER_SAMPLES` and `FULL_RIVER_SAMPLES`, the layer count `RIVER_LAYERS` and the node's distance from the camera; a world without a river source SHALL leave `rivers` empty.
- A plan error ends the frame with the error's text (never a silent drop).

Verify: A
Status: planned
Source: Architect 2026-11-12 (item 15 PR 8), REND-059, STRM-086

#### Scenario: A world with a graph
- **WHEN** the viewer plans a lit frame over a world whose commit holds a channel graph
- **THEN** the plan's rivers hold the nodes with a river in range

### Requirement: TEST-123 The flight over a graph world stays within the reader's cache bound
A flight over a small one-face world baked with a channel graph SHALL plan rivers through the reader at levels 6 to 14 along the longest channel, twice, and the reader's cache SHALL hold at most its byte bound at every step; the flight reports the peak bytes and the decodes.

Verify: A
Status: planned
Source: Architect 2026-11-12, STRM-085

#### Scenario: The cache peak
- **WHEN** the flight runs twice over the same course
- **THEN** some node carries a river and the cache never exceeds its bound
