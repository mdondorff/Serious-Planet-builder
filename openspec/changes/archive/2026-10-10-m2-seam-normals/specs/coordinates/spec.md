## MODIFIED Requirements

### Requirement: COORD-007 Cross-face neighbours
Neighbour lookup SHALL work across all twelve face edges and the eight face corners, SHALL be symmetric (A's east neighbour has A as its west neighbour, after the corresponding orientation change), and positions on a shared border SHALL evaluate to the same 3D direction, bit for bit, from both sides (guaranteed by evaluating face lattice samples with exact edge values and an odd warp, see design); corner neighbours (the other tiles sharing a tile corner) SHALL be three at an ordinary corner and two at each of the eight cube corners, and symmetric

Verify: A
Status: active
Source: CON-08, ADR 0003

#### Scenario: Corner neighbours
- **WHEN** the corner neighbours of every tile corner of levels 1 to 3 are listed
- **THEN** each list equals the set of other tiles with a bit-identical corner direction, has 3 entries (2 at the 24 cube-corner cases per level) and is symmetric

#### Scenario: Shared border
- **WHEN** a border sample point is evaluated from the tile on each side of a face edge
- **THEN** both directions are bit-identical

