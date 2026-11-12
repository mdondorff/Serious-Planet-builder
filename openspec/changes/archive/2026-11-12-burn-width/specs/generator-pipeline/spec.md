## ADDED Requirements
### Requirement: GEN-168 The burnt channel matches the width law
`burn_channels_wide` SHALL lower every cell of a channel's 8-connected line to the channel's level there, SHALL raise no cell, and SHALL burn a trench whose mean width across the line (burnt cells per cell of arc length, the line's ends left out) is within 0.5 cell of max(1, w) on a line off the cell rows and columns, within 0.71 cell on a diagonal or curved line, and within 1.05 cells on a line on a cell row or column; at every column across an axis-parallel line at least floor(max(1, w)) cells SHALL be burnt.

Verify: A
Status: planned
Source: architect, 2026-11-11

#### Scenario: Lattice limit
- **WHEN** a symmetric burn is centred on a cell row
- **THEN** its cell count is odd, so an integer w can differ by one cell from w; the 1.05 adds 0.05 for the line's ends
- **AND** the carve that follows widens the valley around the burnt channel; that is not a burn defect

### Requirement: GEN-169 The channel profile is monotone
The burn SHALL target, at each cell of a channel's line, the lowest of the level and of `min(height found, level)` over the cells within L line steps upstream along the channel tree (a tributary is a channel whose last cell is this channel's first cell; L is the tile's halo in cells), lowering only; the heights are those found before any burn, and the disc of a wide river burns to the same target. A final pass at the end of K7 (after incision, thermal, deposition and pit spreading) SHALL lower every land cell of each line to the lowest height over the cells within L steps upstream along the tree (read from the surface as the pass finds it), lowering path cells only; the removed volume SHALL be booked as eroded and exported (G-SED closes) and reported with the largest lowering, the cells lowered by more than 1 m and the cells where the cutoff binds. Channels SHALL be listed upstream first (a tributary before the channel it joins), ties by segment id, by `channels_from_hydro` and by the feed. A trench SHALL be at most 32 cells wide (`DISC_RADIUS_MAX` 16), and with a width law the burn carries L - 16 steps, so that the disc of a line cell in the halo does not break partition independence; the halo SHALL hold both: K7 SHALL refuse a tile with channels whose halo is under 16 + 8 cells (the smallest carry), naming both numbers (architect, 2026-11-11). A channel inside one cell SHALL be nobody's tributary (what it joins is read from the channels that end in that cell). The stored channel graph SHALL carry its margin (H1 cells beyond a block) in its spec, its key and each block, equal to the H1 cells that hold the tile halo plus one; a bake SHALL refuse a graph whose margin is smaller than its tile halo needs, and a reader SHALL refuse a block whose margin is smaller than its commit's halo needs, each naming both numbers. The pass-2 kernel version SHALL be bumped (the tile keys carry it).

Verify: A
Status: planned
Source: architect, 2026-11-11

#### Scenario: A hole under the profile
- **WHEN** a cell of the line already stands under its level
- **THEN** the cells downstream are lowered to at least that height, and none stands above the cell before it
