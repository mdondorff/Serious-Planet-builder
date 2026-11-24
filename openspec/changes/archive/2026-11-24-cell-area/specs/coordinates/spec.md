## ADDED Requirements
### Requirement: COORD-013 The solid angle and area of a cube cell
`GlobalCell::solid_angle(n)` SHALL be the solid angle in steradians of a cell of a level with `n` cells per face edge, within a relative 1e-9 of the exact spherical value at every `n` from 1 to 2^30. `GlobalCell::area(n, radius_m)` SHALL be `(radius_m * radius_m) * solid_angle(n)`, in that order, so the radius stays out of the metric.

Verify: A
Status: active
Source: Lead 2026-11-24 (#197, owner O2 2026-11-23: planet primitives on main during G3, additive and output-neutral; G3-5's quadrature metric; the plan with the lead's four changes, 2026-11-24), contract §3 (`a_geo` conserved to 1e-9), GEN-138, COORD-011, CON-14
Detail:
- projection: the cube is equiangular (`TangentWarp`): face coordinate `s` in `[-1, 1]` is the angle `alpha = pi/4 s` and the tangent coordinate `x = tan(alpha)`; a level's cells are equal steps of `alpha`, `pi / (2 n)`;
- integrand: `d Omega = sec^2(alpha) sec^2(beta) / (1 + tan^2(alpha) + tan^2(beta))^(3/2) d alpha d beta`, even in each angle and symmetric under their swap (the face square's eight symmetries);
- form: a 2 x 2 Gauss-Legendre rule in `(alpha, beta)` on each of `k x k` equal subcells, `k = ceil(512 / n)` (`SUBCELL_LEVEL` 512), one form at every `n`; no difference of nearly equal values, so no cancellation;
- canonical: computed from the cell folded into one octant, `(min(i, n-1-i), min(j, n-1-j))` sorted; every cell a symmetry of the cube maps onto it has the same bits, the 24 rotations included;
- sums: a sum of cell areas uses one fixed reduction, `pairwise_sum` (halves down to at most 8 values, then in order), over each whole face row, then over the rows, then over the faces; a caller that holds the cells in tiles or regions SHALL assemble whole face rows before reducing (a sum of tile sums is other bits);
- `libm` only, no thread or platform input (CON-14); panics for a cell outside the level `n` or a level outside `1..=2^30`;
- additive: no caller switched; GEN-138's `Metric::area` keeps its values (9.1e-7 worst relative error at 98,304, measured in TEST-152, report only); hashes, goldens and the determinism manifest stay bit-identical.

#### Scenario: Exact within 1e-9, closed sums, the same bits
- **WHEN** cells of levels from 1 to 2^30 are measured against references independent of the form, faces and the cube are summed, and cells are rotated
- **THEN** every cell is within 1e-9, a face sums to 4 pi / 6 and the cube to 4 pi within 1e-9, and a rotated cell and a sum on 1 or 7 threads have the same bits
