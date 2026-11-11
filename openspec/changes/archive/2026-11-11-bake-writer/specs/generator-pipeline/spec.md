## ADDED Requirements
### Requirement: GEN-167 The lake water mask of a tile
The bake SHALL decide for each cell of a tile of L which lake's water it is, on the final surface `B + L`, as a pure function of the tile's window, and SHALL store the result as runs in the tile record's water section.
- A cell is core wet when its H1 parent is a water cell of a lake and its height is under the lake's level.
- A cell is ring wet when its height is under the level and it lies within 8 steps (8-neighbours) of a core wet cell, entering only cells whose H1 parent is in the lake's ring (draining into the lake, in no other basin).
- A cell is in one lake only; the labels are an index into the tile's list of lake ids (at most 255).
- The window reaches at least 8 cells beyond the core, so that tiles that meet decide the cells next to the edge alike in any order and on any thread count (CON-14).

Verify: A
Status: planned
Source: Architect D17/D18/D20, design section 13

#### Scenario: Bowl, valley and hollow
- **WHEN** a lake with a sill, a valley below the sill and another hollow under the level are masked
- **THEN** only the lake's own water and its ring are wet; the valley, its sides and the hollow are dry
