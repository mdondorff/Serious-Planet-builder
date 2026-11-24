## ADDED Requirements
### Requirement: TEST-152 The cube cell's solid angle against independent references
Tier-A tests SHALL check COORD-013's solid angle against references independent of the form under test, its sums, its symmetry and its thread independence, each test naming its reference.

Verify: A
Status: active
Source: Lead 2026-11-24 (#197, the plan's references), COORD-013
Detail:
| Check | Reference | Levels |
|---|---|---|
| a cell within 1e-9 | the exact four-term `atan` solid angle of the gnomonic patch (its rounding at most about 5e-11 here) | every canonical cell of n 1 to 64, 128, 256, 512 |
| a cell within 1e-9 | 8 x 8 Gauss-Legendre in the angles on the whole cell (not exact at n of 1 to 4, so only above 512) | n 513, 1,000, 1,024, 4,096, 98,304, 2^20, 2^30; corner, edge, centre and 20 scattered cells |
| a face sums to 4 pi / 6, the cube to 4 pi, within 1e-9 | the fixed reduction (pairwise per row, rows, faces) | n 1, 2, 3, 4, 5, 7, 8, 16, 64, 512, 1,024 (the cube at 1, 7, 1,024) |
| the same bits under the 24 rotations | the rotated cell's own value | every cell of n 5 and 6; 200 cells of n 1,024 |
| the same bits on 1 and 7 threads | the fixed reduction | face 0 of n 1,024 |
- GEN-138's `Metric::area` against COORD-013 at 98,304 is measured and printed, not judged (report only).

#### Scenario: A form that drops the fold or the subcells
- **WHEN** the octant fold or the subcells (`k = 1` at every `n`) are removed
- **THEN** the rotation test, or the small-level and closure tests, fail
