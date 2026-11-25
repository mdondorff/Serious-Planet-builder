## Why
- Owner O2 (2026-11-23): the planet primitives the port needs land on main during G3, additive and output-neutral (lead #197, PR 4 of the primitives).
- G3-5's quadrature metric: contract §3 conserves `a_geo` to 1e-9. Today's area (GEN-138, `Metric::area`) is the exact solid angle by a four-term `atan` difference, but it loses digits to cancellation: a worst relative error of 9.1e-7 measured at 98,304 cells a face edge. It is computed per window, not per canonical cell.

## What changes
- COORD-013: `GlobalCell::solid_angle(n)` in steradians and `GlobalCell::area(n, radius_m) = (radius_m * radius_m) * solid_angle(n)`, from `planet_core::cell_area`. The form is a 2 x 2 Gauss-Legendre rule in the angle coordinates on `k x k` subcells, `k = ceil(512 / n)`, computed from the cell folded into one octant of its face. `pairwise_sum` is the one fixed reduction for sums of cell areas.
- TEST-152: references independent of the form (exact four-term for n <= 512, 8 x 8 Gauss-Legendre above), face and cube closure, the 24 rotations, 1 and 7 threads; today's `Metric::area` error at 98,304, reported only.
- Not here: any caller switch (a later change, with its own declared output change).

## Expected golden changes:
- none: additive code with no caller; hashes, goldens and the determinism manifest stay bit-identical.

## Impact
- Spec deltas: COORD-013 (coordinates), TEST-152 (testing).
- Crates: core (`cell_area.rs`, `global_cell.rs`); generators tests (`bake_metric.rs`, the report only).
- A new planet contract: numerics-reviewer, and the lead's review before merge.
