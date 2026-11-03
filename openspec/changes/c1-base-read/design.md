## Context
- The bake is a lattice of 16,384 cells a face edge (611 m). `read_direction` blends the four corner samples of a cell bilinearly (ADR 0027): continuous, with a slope jump across every cell edge and saddles inside a cell.
- From 300 m the jumps read as pyramid-like facets where the base is steep; on flat ground the crease detail (amplitude about 100 m, always carving) hides them (E2b probe, 2026-11-03).

## Decisions
1. Interpolant: Catmull-Rom, tension 0.5, separable (rows, then columns).
   - It interpolates: a lattice point is its own sample (weights `(0, 1, 0, 0)` exactly), so coasts, continent shapes and every stored sample stay where they are.
   - C1 across every cell edge; small overshoot beside steps and spikes (at most 0.119 of the local sample range in the tests; bound 0.28125, see 4).
   - Rejected: smoothstep weights (zero slope at every node gives terraces); a quadratic or cubic B-spline (no overshoot, but it does not interpolate: every peak and coast of the bake lowers or moves, and the calibrations made on the bake shift).
2. Face edges and corners.
   - The stencil of a point within two cells of a face edge reaches nodes beyond it. Such a node is the bilinear read (`read_direction`) of the direction of the extended face coordinates, that is the neighbour face's own samples (at a cube corner the third face's, on the diagonal beyond both edges).
   - Across a face edge the two faces' lattices are warped differently, so the ghost does not sit on a vertex of the neighbour: the slope break across a face edge falls about threefold (tests: mean 161 m per cell against 459 for bilinear), it is not removed. A second pass reading the ghost by C1 gave no gain and is not used.
   - Same bits from either face: the read is a pure function of the direction; edge points and corners given in either adjacent face's coordinates read the same bits (tested on 371 directions on all 12 edges, near the corners, and on the 8 corners).
3. A field beside the old one, not a replacement. `cpu-erosion-v2` stands beside `cpu-erosion-v1` with a new key family (inputs hash with `EROSION_C1_VERSION` and `READER_C1_VERSION`), selected by `world.height_field`. The default switch, and with it every pin and golden of the default field, is a follow-up after the preview.
4. Bounds. A C1 value is a combination of 16 samples whose weights sum to 1 with negative weights summing to at most 0.28125 (1.25 per axis, so 1.5625 in two dimensions). The scan therefore:
   - reads the window 3 cells wider (the stencil's two cells and one for the neighbour ghosts);
   - widens its sample range by 0.28125 of itself on each side;
   - multiplies the largest lattice step (the slope bound) by 4: 2.8125 within a face (the Catmull-Rom derivative is at most 2.25 times the largest of the three differences it spans, times 1.25 for the four rows), rounded up for the ghosts.
   - The erosion bound's Lipschitz term takes the factor through `max_du` and `max_dv`.
   - Tests hold every height of random caps, of caps on edges and corners, and of caps over a lattice with spikes and steps. The factor 4 and the pad 3 are validated by those tests, not proved for the ghosts.
5. Cost. The read touches 16 nodes instead of 4 (the gradient stencil adds six reads): about 1.9x on `height(dir)` and per tile, 1.45x on `height_bounds` (PR body table).

## Open
- Which goldens move when the default switches: none in this change; the list is made in the follow-up.
- Ghosts at a corner are the third face's bilinear values: C0 only on the diagonal beyond both edges, within one cell of the corner.
