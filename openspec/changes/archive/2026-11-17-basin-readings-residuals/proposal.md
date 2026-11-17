## Why
- SPIKE-130's two questions were ruled as G0 contracts readings 5 and 6 (lead, 2026-11-17):
  - the L kept rule reads KS1, and `S0`'s basins are a diagnostic;
  - a spill reaching the sea receives an `outlet_sea`.
- Lead #157 folds in the numerics review's remaining residual items from #1567:
  - a residual naming no stage or an out-of-range cell was never visited;
  - a cell clamping and over-filling in one stage checked every residual against the clamp;
  - duplicate residuals passed.

## What changes
- GEN-189: the Basin row takes readings 5 and 6. Each residual names a cell and a stage that ran and records one clamp or fill overrun, with at most one residual for each.
- `validate_budget`:
  - a residual matches either of its cell's two events;
  - more residuals than events, or a residual naming no cell or stage, is a `ResidualMismatch`.
- TEST-140: a stray stage and cell, a duplicate, and a cell that both clamps and over-fills.

## Impact
generators (`bake/river_graph.rs`: `validate_budget`); tests `river_graph.rs`. No producer reads the kept rule yet (G1e's producer is spike code).
