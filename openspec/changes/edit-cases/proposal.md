## Why
Inbox #100 step 3 (A3 sections 6.5 and 12): the edit cases E1-E6 of the prototype region (spikes/prototype-region/results/results.md): a re-solve of an edit is the solve of the edited definition (G-EQ) with the work of what changed (G-INC), and the structural gates hold after every edit.

## What Changes
- `bake::round`: `Drivers` (class carve factor, uplift per cell, runoff weights), `solve_rounds_with`, and `solve_rounds_memo` with a `RoundCache`: the partitioned route with memoised tile floods per round and a memo of catchment rounds by their key inputs; `RoundWork` reports the work per round.
- `bake::partition`: `partitioned_route_weighted` (runoff weights), `PartCache::use_slot` (change counts read against the same round of the previous solve).
- `bake::hydro` (inbox #102): `Q_w`, the routed discharge by the cascade in descending filled level (a basin resolves when its spill cell is reached; flow stops at its exit cells and `q_out` enters at the spill cell), `E_open = PET - (P - r)` as the evaporation input (the caller's `max(PET - P, 0)` goes), the 256-step re-entry window dropped (the cascade makes it unnecessary), `Verify` gets the closure of `Q_w`; `hydrology_with` (intentional basins), `Basin.overflows`, `Hydro.label`, `basin_continuity` (the continuity map of A3 section 5.4), `BasinClass::Intentional`.
- GEN-146 added (generator-pipeline).
- `examples/edit_cases`: E1-E7 (E7: a land bridge joins two landmasses, then is taken away), the complete result in G-EQ, on the prototype region at 12,288 cells with G-EQ, G-INC and the structural gates after each edit.

## Impact
- Code: `generators::bake::{round, partition, hydro}`, `examples/edit_cases`. No call site in any pass of the bake; nothing in today's output moves.
- Not in this change: the canonical forms of A3 section 9 (f32 rounds, 0.25 m levels) against an f64 run (the prototype compares f64 with f64); storing the memo; the partitioned rounds at landmass scale (next).
