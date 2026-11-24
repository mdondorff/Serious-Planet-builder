## ADDED Requirements
### Requirement: COORD-012 A tile plus its halo as one grid on the cube
`planet_core::CubePatch` SHALL be a square core of `size x size` cells of one face of a level with `n` cells per face edge plus a halo of `halo` cells on every side, as one grid of `(size + 2 halo)^2` cells `(u, v)`, `-halo <= u, v < size + halo`, each a `GlobalCell` (COORD-011), so that ties between its cells break canonically (CON-30). It is the only tile-plus-halo grid the L stages use (reconciliation row 7, from the port on).

Verify: A
Status: active
Source: Lead 2026-11-24 (#195, owner O2 2026-11-23: planet primitives on main during G3, additive and output-neutral; the G3 port order's C2, reconciliation rows 1, 2 and 7), COORD-011, GEN-139, CON-30
Detail:
| Patch cell `(i, j) = (i0 + u, j0 + v)` | Its `GlobalCell` |
|---|---|
| inside the face | the face's cell `(i, j)` |
| one coordinate outside `0..n` | the neighbour face's cell folded over that edge (`fold_cell`'s rule, any depth up to `n`; axis-only, GEN-139) |
| both outside (past a cube corner) | two folds, the deeper coordinate first; at equal depths, first across the edge towards the face `F` with `det(n, n_F, n_other) > 0` |
- refused (None): a core outside the face, `size < 1`, `halo < 0` or `halo > n`, `n` outside `1..=2^30`, a cell count that does not fit a `usize`; `halo = n` reaches the neighbour face's far edge;
- the stitched quadrant past a cube corner continues each strip across the corner's third edge, so no patch has a hole; for a core touching the corner its cells are cells of the patch's strips (a cube corner has 270 degrees) and of the corner's three faces only;
- `extended_point(u, v)`: the cell's point on the core face's extended plane in half-cell units; a rotation of the cube maps it onto the rotated patch's;
- additive: no caller changes; `fold_cell` keeps its results (its body is `fold_cell_axis`, crate-private); hashes, goldens and the determinism manifest stay bit-identical.

#### Scenario: Interior, halo and corner
- **WHEN** every core position of cores of 1 to 4 cells with halos of 2 to 5 cells, and up to `n`, is built on every face of levels of 4 to 12 cells
- **THEN** inside the face each cell is the face's own; each cell across an edge, to a depth of `n`, is the cell reached by straight axis steps from the face's border; each stitched cell past a corner is the straight continuation of the strip it adjoins, on the diagonal the one the orientation rule names; a patch whose core touches the corner spans three faces and every stitched cell repeats a strip cell

#### Scenario: Rotations
- **WHEN** every patch of levels of 6 and 7 cells is mapped by each of the cube's 24 rotations
- **THEN** the rotated patch holds, at the rotated extended point of each cell, the rotated cell
