## ADDED Requirements
### Requirement: STRM-086 The viewer's stored world gives the frame its river source
A stored world opened with the new chain's commit for `--erosion` SHALL hold a river source over the runtime reader of the commit the layer is read under when the commit holds a channel graph, and none when it does not; the source SHALL give the frame's river plan the grid of the commit's channels (the face edge and the L tile edge in cells, the nominal cell size of the K7 grid) and the channels of an L tile through the reader's bounded cache.
- The erosion note says whether the commit has a channel graph.
- The source reads under the same commit as the layer's tiles (CON-22).

Verify: A
Status: planned
Source: Architect 2026-11-12 (item 15 PR 8), STRM-085, REND-059

#### Scenario: A commit without a graph
- **WHEN** a stored world is opened with a commit that holds no channel graph
- **THEN** it has no river source and its note says "no channel graph"
