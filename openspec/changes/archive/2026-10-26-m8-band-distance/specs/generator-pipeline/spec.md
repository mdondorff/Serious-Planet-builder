## ADDED Requirements

### Requirement: GEN-083 The valley band's distance is Euclidean
The band painting of an A-net chunk (GEN-060) SHALL take the distance of a cell from the river as the Euclidean distance to the source that owns the cell (the nearest source the relaxation reaches), not as a sum of edge lengths over the cell graph, so that a wall off the grid axes rises evenly from cell to cell and the band of a core cell does not depend on the halo (GEN-049).

Verify: A
Status: active
Source: look review 2b (river-r1 sunset ripple), GEN-060, GEN-049

#### Scenario: A wall off the grid axes
- **WHEN** a straight corridor at 26.6 degrees to the grid axes is painted over flat ground at the lowland side slope, and the slopes of three adjacent cells in the middle of the wall (between the floor's edge and the rim) are compared along both axes
- **THEN** the larger over the smaller slope of the two pairs is at most 1.2 at the 90th percentile (the graph distance gave 1.4 with a median of 1.2; the Euclidean distance 1.05 with a median of 1.02)

#### Scenario: Two corridors and two halos
- **WHEN** two corridors off the grid axes that run towards each other are painted over a chunk with a halo of 30 rings and of 45 rings
- **THEN** every core cell has the same painted height, bit for bit, in both
