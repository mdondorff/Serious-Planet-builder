## Why
- SPIKE-132 found two contract gaps, ruled as G0 contracts reading 9 (DEC crusader #157, 2026-11-17):
  - a fill held at the budget left a residual that the validator read as recording nothing (19-21 cells on the fixture);
  - a reach that is already `exception(rise_over_budget)` could not also carry `budget_residual` (458-477 residuals).
- Reading 10 (lead, 2026-11-17): near a confluence the catchment boundary meets the river and is valley floor, not a divide.
- Lead #158 orders both into GEN-189 and the validators before G1d.

## What changes
- GEN-189:
  - Reach row: `exceptions`, a list of `(reason, value)` with at most one per reason, replaces the single `status`;
  - Budget use row: every stage states a wanted fill; a fill held under it is a fill clamp, recorded by a residual with the wanted fill and the budget;
  - a new Divide buffer (KV) row for reading 10 (KV is spike code; no validator yet);
  - Canonical form and Validators rows follow.
- `bake::river_graph`:
  - `Reach::exceptions: Vec<Exception>` and `Reach::has_exception` replace `Status`;
  - the canonical form writes `exceptions=` sorted by the reason's name, then the value's bits;
  - `Stage::wanted_fill`;
  - `validate_budget` assigns fill clamps one to one with the other events (`FillClampWithoutResidual`);
  - `validate` reports `DuplicateException`.
- TEST-140: the fill clamp's cases, a reach with two exceptions as a residual's feature, a duplicate reason, and the list's canonical order.

## Impact
generators (`bake/river_graph.rs`); tests `river_graph.rs`, `g0_graph.rs`. No stored graph exists yet; the canonical form's field is renamed (`status=` to `exceptions=`).
