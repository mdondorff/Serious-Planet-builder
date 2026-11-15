## MODIFIED Requirements
### Requirement: GEN-173 Flats are graded by distance to the pour points
The canonical flood SHALL grade a flat by the Euclidean distance to its lower rim, not by the chessboard count: the level is `base + epsilon d / sqrt 2`, `d` the distance in cells to the nearest rim cell, every cell kept above the neighbour it drains to. A graded level SHALL be at most the cell's chessboard level; where that bound binds, the cell keeps its chessboard parent as a lower neighbour. A flat whose Chebyshev diameter exceeds `halo - 1` cells, or that touches the border of the grid, keeps the chessboard grade. The grade SHALL be a function of the flat and its rim only, so the resident flood and the partitioned route give the same bits on any tiling. No cell SHALL stand above its chessboard count.

Verify: A
Status: active
Source: Architect, 2026-11-12 (SPIKE-108); the cap and its drain: lead, 2026-11-14 (K7 refusal on the L-eroded surface, spikes/erosion-at-l/results/k7-flat-refusal.md)

#### Scenario: A square pit with one pour point
- **WHEN** a square pit drains through one outlet in its wall
- **THEN** the longest straight diagonal run of the receivers is under half that of the chessboard flood, and every cell has a lower neighbour

#### Scenario: A flat with a hump
- **WHEN** a flat holds a cell the fill lifted by less than its chessboard count, beside cells nearer the pour by count but farther from it by Euclidean distance, and a cell outside the flat drains into them
- **THEN** no graded cell stands above its chessboard level, every cell that is not an outlet has a lower neighbour, and the routing takes a receiver for every cell

#### Scenario: Details of the grade
- **WHEN** a flat is graded
- **THEN** the production profile's halo is 128 cells, so the bound is 127; a flat of exactly the bound across a tile corner gives the same bits resident and partitioned
- **AND** a flat is a connected set of cells the fill lifted with one pour level, and its lower rim is the cells next to it at or below that level
- **AND** the non-canonical flood and cube grids are unchanged
- **AND** no cell above a lower outlet beside it stands above one epsilon over it (GEN-159 and GEN-162 keep their intent; their exact values follow this grade)
