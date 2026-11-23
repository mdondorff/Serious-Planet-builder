## ADDED Requirements
### Requirement: GEN-147 A landmass of the cube is a compact grid
A landmass of the cube sphere SHALL be usable as a compact grid: only its cells, indexed by their rank among the active cells in the order of the cube's cell ids.
- The neighbours, areas, distances and global ids of a cell SHALL be those of the cube grid.
- The routing and the rounds on the compact grid SHALL give, for every cell, the bits of the same solve on the masked cube grid.

Verify: A
Status: planned
Source: A3 section 6.6, inbox #101, briefing item 9
- The runs of `examples/landmass_scale` (the memory and time of the landmass at 3,072, 6,144 and 12,288 cells) are measurements, not gates; the gate is the tier-A test.

#### Scenario: a compact landmass
- **WHEN** a landmass of a sphere of 24 cells a face edge is routed and carved for 2 rounds on the masked cube grid and on the compact grid
- **THEN** the neighbours, areas, distances, global ids, receivers, drainage areas and heights of every cell are the same bits
