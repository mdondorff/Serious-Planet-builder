## ADDED Requirements
### Requirement: REND-059 The river coverage of a tile is a signed distance to the river edge
The plan SHALL give each tile with rivers a coverage of its cell centres: the signed distance in cells to the nearest river edge, coded `128 + 32 d` clamped to 0..255 (+-4 cells, 1/32 cell a step), and the renderer SHALL draw river water where the interpolated code is at or under 128.
- The river is the stored channels' polylines at the burn's width: `max(min(w, 32 cells), 1 cell)` from the one function `trench_width_cells`, the discharge interpolated along each segment.
- The distance is a function of the channels alone: any window of the same channels gives the same texels (CON-30).
- The plan carries the coverage of a terrain node at its own samples (`res + 1` a side, the face coordinate `s` at the cell-index position `(s + 1) cells / 2 - 1/2`), only for nodes whose samples are at most a parameter of L cells apart and that have a river within range; the channels come from the runtime reader under the commit of the tiles.
- A texel farther than 4 cells from every edge has the far-outside code (255).

Verify: A, C
Status: planned
Source: Architect 2026-11-11 (item 15 PR 8), GEN-165, GEN-168

#### Scenario: Smooth banks
- **WHEN** a 45-degree river is coded
- **THEN** the distance changes by cos 45 degrees a cell along a row, with no staircase, and a river under a cell is one cell wide

#### Scenario: A widening river
- **WHEN** a segment widens from one cell to 32 cells and is burnt and coded
- **THEN** every burnt cell is within 0.75 cell of the coded edge and every cell half a cell inside the edge is burnt (the edge is the envelope of the burn's discs)

### Requirement: REND-061 The plan chooses which nodes get a river layer
The terrain plan SHALL carry the river coverage as an optional part (`rivers`): `None` keeps the plan's text and hash as they were. The renderer SHALL hold the coverages in one 8-bit texture array of at most 256 layers (a parameter), a node's layer index in its record, and the plan SHALL choose the nodes that get a layer when more have a river: the nearest by node distance first, ties by node index, in node order; the number of nodes dropped SHALL be in the plan's text and the frame's stats, never silent.
- The scale of a code is in cells, not metres (CON-11).
- Neighbouring nodes of one level hold the same codes on their shared edge samples; across a level boundary the shared samples agree and the largest difference of an in-between sample is reported.
- Sampling clamps inside a layer: no bleed between layers.
- With no river node every lit golden is bit-identical.

Verify: A, C
Status: planned
Source: Architect 2026-11-12 (REND-059 part C), REND-059, CON-03, CON-11

#### Scenario: Layer overflow
- **WHEN** six nodes have a river and the array holds three layers
- **THEN** the three nearest keep theirs, the text says three were dropped, and the same plan results from any input order

### Requirement: REND-060 River water is the ground at the burned bed
River water SHALL have no geometry of its own: where the coverage is wet the terrain's own ground is drawn as water, and that ground is the trench K7 burned to the segment's profile, so the water level is the burned bed (GEN-165). A frame with rivers SHALL submit the same draws, triangles and node records as the frame without. A visible depth (a surface above the bed) is a look value, decided later (architect, 2026-11-12).
- The cells the coverage calls water are cells the burn lowered, to within the line's rasterisation.

Verify: A, C
Status: planned
Source: Architect 2026-11-12 (REND-059 part C), GEN-165, REND-059

#### Scenario: No geometry
- **WHEN** a frame is drawn with and without a river plan
- **THEN** the draws, triangles, vertex bytes and node records are equal
