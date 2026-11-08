## ADDED Requirements
### Requirement: GEN-139 The coarse hydrology routes the whole planet as one graph across the cube edges
H0 SHALL route the six cube faces as one graph of cells: a cell inside a face has its 8 neighbours, a cell on a cube edge has the axis neighbour of the face it meets (the cells along a shared edge pairing 1:1 and the relation symmetric), and no diagonal crosses an edge or a corner.
- The routing SHALL be one priority flood from the outlets over the whole graph (pop order by height then cell id), the receiver of each cell the steepest descent on the filled surface by great-circle distance, and the drainage area the true cell areas (GEN-138) summed along the receivers; the area at the outlets SHALL equal the area of the sphere (relative error under 1e-9) and every cell SHALL drain to an outlet.
- A catchment that reaches a cube edge SHALL continue on the face it meets as one catchment; the result SHALL be the same bits on every run and thread count (CON-14).

Verify: A
Status: planned
Source: SPIKE-105 §12 F6, inbox #97 item 2 (owner D1, 2026-11-08), GEN-138

#### Scenario: H0 across the cube edges
- **WHEN** graphs of 4, 12 and 33 cells a face edge are walked and a planet that falls away from a sea cap is routed twice
- **THEN** the graph is connected and symmetric, exactly the cells on a cube edge have a neighbour on another face, the areas sum to the sphere, the outlets hold the whole area, no cell drains nowhere, the widest catchment spans at least four faces, and the two runs are the same bits
