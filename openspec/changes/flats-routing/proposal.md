# flats-routing (GEN-173, TEST-119)

## Why
- SPIKE-108: the generator's channels run in straight 45-degree lines (diagonal chords 1.2 to 4.7 times, straight runs 2.3 to 10 times the DEM's).
- Cause: the canonical flood grades a flat by the chessboard count (one epsilon per 8-neighbour step), so a flat drains along the diagonals; the carve incises along those routes and the valleys stay.
- Architect, 2026-11-12: route the flats towards the pour points, canonical order kept; larger flats keep the chessboard grade, with a criterion that does not depend on the tiling.

## What
- `flood::regrade_flats`: a pure post-pass on the canonical filled surface (planar grids). A flat (connected lifted cells of one pour level) with Chebyshev diameter at most `FLAT_BOUND` = halo - 1 = 127 cells and no cell on the grid border gets the level `base + epsilon d / sqrt 2`, `d` the Euclidean distance in cells to the nearest lower rim cell; each cell stays above the neighbour it drains to.
- Called by `priority_flood_with` (canonical) and by the partitioned route on the assembled surface: one function, so no tiling decides a bit.
- The non-canonical flood and cube grids are unchanged.
- Toward-pour field only; the away-from-higher term is not built.

## Measured (R3b, start surface, 1,020 x 1,020 cells; `examples/flats_share.rs`)
- Lifted cells 225,745 = 25.6 % of land; 79.2 % of them in graded flats.
- 120 flats keep the chessboard grade: 47,044 cells, 20.8 % of the lifted cells, 5.2 % of land. Five are lake beds of 20.9 to 43.5 m (diameters 132 to 318 cells, 43,899 cells); the others touch the window border (the rule treats a flat beside the border as larger than the window shows).
- Flats with a lift under 10 m: 98.9 % of their cells are graded (the rest touch the window border).

## Declared changes
- Planar grids only: cube grids are not regraded (production windows are planar).
- Lake-depth test: the per-round bound of 0.25 m is unchanged and a cumulative bound is added (rounds 2 to 4 at most round 1, round 4 below it). The first build deepened the lake by 0.25219 m in round 3 (a lifted cell graded below its own height, found by the numerics review); fixed, the sequence is the chessboard one: 42.665, 41.745, 41.400, 40.855 m.
- Filled-surface bits on every canonical flood with flats: the sea-outlet digest of `bake_round` (`0xb9e66858_0505bf11` to `0x8686f0d1_ac1d02ec`), `docs/stages/manifest.txt`, the depth sequence of the lake test (0.26 m a round).
- Kernel versions: `BAKE_KERNEL_VERSION` 5 to 6, `PASS2_KERNEL_VERSION` 6 to 7 (the flood moves bits in both); stores of the old versions are refused (owner, 2026-11-05: old stores need not stay compatible).
