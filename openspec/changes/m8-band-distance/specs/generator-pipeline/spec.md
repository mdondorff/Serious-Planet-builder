## MODIFIED Requirements

### Requirement: GEN-060 The channel from discharge, the valley from relief
Round every corridor cell an A-net chunk SHALL hold the cells within `band_cap_m` of the river at a channel and valley profile: a channel of width `w_coef Q^w_exp` and depth `d_coef Q^d_exp` (`Q = q_per_km2 A`, km2) under a valley floor `lerp(floor_low, floor_high, rf)` channel widths wide at the bed, the corridor cells at the channel floor, and sides rising at `lerp(side_low, side_high, rf)` to the ground. `rf` is the base's relief over the river's coarse cell and its eight neighbours over `relief_full_m`; the valley depth follows GEN-057; the cap follows the halo (`(halo_rings - area_window - 2)` cells at most; 0 paints no band). The constants are part of the definition hash. The distance of a cell from the river SHALL be the Euclidean distance to the source its band cell is owned by (the nearest source the relaxation reaches), not a sum of edge lengths over the cell graph, so that a wall off the grid axes rises evenly from cell to cell.

Verify: A
Status: active
Source: SPIKE-40, SPIKE-41, planner decision 2026-10-25 (H2), GEN-057, GEN-058

#### Scenario: A great lowland river
- **WHEN** a chunk of a river of 9.5e6 km2 over smooth ground runs its incision
- **THEN** the channel is within a factor 2 of the law's width at half depth and its depth within 1.5 of the law's, in a valley floor wider than half the channel

#### Scenario: A wall off the grid axes
- **WHEN** a straight corridor at 26.6 degrees to the grid axes is painted over flat ground at the lowland side slope, and the slopes of three adjacent cells in the middle of the wall (between the floor's edge and the rim) are compared along both axes
- **THEN** the larger over the smaller slope of the two pairs is at most 1.2 at the 90th percentile (the graph distance gave 1.4 with a median of 1.2; the Euclidean distance 1.05 with a median of 1.02)
