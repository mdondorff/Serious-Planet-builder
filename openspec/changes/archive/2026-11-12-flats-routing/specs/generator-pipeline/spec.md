## ADDED Requirements
### Requirement: GEN-173 Flats are graded by distance to the pour points
The canonical flood SHALL grade a flat by the Euclidean distance to its lower rim, not by the chessboard count. A flat is a connected set of cells the fill lifted (filled level above the height) with one pour level; its lower rim is the cells next to it at or below that level. A flat whose Chebyshev diameter is at most `halo - 1` cells (the production profile's halo is 128 cells: 127) and none of whose cells lies on the border of the grid SHALL have the level `base + epsilon d / sqrt 2`, `d` the distance in cells to the nearest rim cell, every cell kept above the neighbour it drains to; a larger flat, or one that touches the border, keeps the chessboard grade (one epsilon a step).
- The grade SHALL be a function of the flat and its rim only: the resident flood and the partitioned route SHALL give the same bits, on any tiling, for a flat of any size, including one of exactly the bound across a tile corner.
- No cell SHALL stand above its chessboard count, and no cell above a lower outlet beside it SHALL stand above one epsilon over it (GEN-159, GEN-162 keep their intent; their exact values follow this grade).
- The non-canonical flood and cube grids are unchanged.

Verify: A
Status: planned
Source: Architect, 2026-11-12 (SPIKE-108)

#### Scenario: A square pit with one pour point
- **WHEN** a square pit drains through one outlet in its wall
- **THEN** the longest straight diagonal run of the receivers is under half that of the chessboard flood, and every cell has a lower neighbour
