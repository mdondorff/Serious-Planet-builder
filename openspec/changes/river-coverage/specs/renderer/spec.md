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
