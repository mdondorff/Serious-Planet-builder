## ADDED Requirements
### Requirement: COORD-011 Global cells and the cross-edge fold
`planet_core::GlobalCell` SHALL name a cell `(face, i, j)` of a level with `n` cells per face edge (`0 <= i, j < n`) as one `u64`, and its neighbours SHALL cross the twelve cube edges by the public `planet_core::tile::fold_cell` (exact integers on the cube, the fold `TileId::neighbors8` uses): an axis step that leaves a face lands on the cube-adjacent cell of the neighbour face, and no diagonal crosses an edge or a corner (GEN-139).

Verify: A
Status: active
Source: Lead 2026-11-24 (#193, owner O2 2026-11-23: planet primitives on main during G3, additive and output-neutral; the G3 port order's C1), GEN-139, COORD-007
Detail:
- bits: face in bits 60-62, `j` in bits 30-59, `i` in bits 0-29; 30 bits an axis hold 2^30 cells per face edge (`MAX_CELLS_PER_EDGE`), above the runtime's deepest level; bit 63 stays reserved (0); a cell outside `0..n`, a face over 5 or an `n` over 2^30 is refused;
- order: the `u64` order is (face, j, i); it is the canonical tie key of the seam-pair spill engine (GEN-191);
- `step(n, di, dj)` for `|di|, |dj| <= 1`: inside the face the cell itself; one coordinate one step outside `0..n` with the other inside and an axis step, `fold_cell`; a diagonal that leaves the face, or both coordinates outside, has none;
- `neighbours4(n)` (east, west, north, south) always exist; `neighbours8(n)` adds the diagonals that stay in the face: 8 inside a face, 6 on an edge, 5 at a face's corner cell;
- `centre_point(n)`: the cell centre as a cube point in half-cell units (normal coordinate `n`, in-face coordinates `2 i + 1 - n`, `2 j + 1 - n`); `of_centre_point(p, n)` its inverse;
- additive: no caller changes; hashes, goldens and the determinism manifest stay bit-identical.

#### Scenario: Rotations and corners
- **WHEN** every cell of levels with 4 and 5 cells per face edge is mapped by each of the cube's 24 rotations (its centre point rotated)
- **THEN** its 4- and 8-neighbourhoods map onto the rotated cell's, every neighbour relation is symmetric, a neighbour on another face is an axis neighbour, and at each cube corner the three corner cells are pairwise axis neighbours
