## Why
- The numerics review of #1564 left two items; lead #156 orders them folded in.
- In `rasterise`, `total_cmp` ranks -0.0 below +0.0, so a signed zero decided a tie instead of the reach id.
- A residual's `needed_m` and `available_m` were never compared with the stage's numbers.

## What changes
- GEN-189:
  - the rasterisation compares `q_w` with -0.0 and +0.0 equal;
  - a residual must carry its stage's numbers (the wanted lowering and the budget left, or the fill and the budget, within 1e-9 m) and record a clamp or a fill overrun (`ResidualMismatch`).
- TEST-140:
  - a signed-zero tie, either way round;
  - a clamp residual with wrong needed or available numbers, a fill residual with wrong numbers, and a residual where nothing clamped.

## Impact
generators (`bake/river_graph.rs`: `rasterise`, `validate_budget`); tests `river_graph.rs`.
