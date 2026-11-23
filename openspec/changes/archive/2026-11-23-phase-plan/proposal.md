## Why
Inbox #109 and #116 (architect, 2026-11-09): the complete resident build at 12,288 cells read 11,329 MiB of peak commit against the 11,444 MiB gate with the spill and the release in the example only. G-SCALE's complete-build row closes when the spill and the release are in the build's phase plan and the worst of two runs on that path is under the gate.

## What Changes
- `bake::build::build_resident` and `PlanParams`: the rounds, H1 and the sediment pass after the landmass, with the pre-carve surface spilled, H1's drivers from a climate closure, one flood for the weighted route and H1, G-CON before the release of `Q` and `Q_w`, a mark at every allocation boundary, and the stage probe at the landmass and at H1.
- `route::route_rules_filled`: `route_rules` that also returns its filled surface (the same bits as a separate flood).
- `hydro::{weighted_cell_area, verify_with_area}`: `verify` with the weighted cell area given, so that the weights are not regenerated for the check.
- `examples/landmass_scale plan`: the plan at any k with the memory at every mark, the per-phase seconds, the hashes and the check.
- GEN-158 added (generator-pipeline).

## Impact
- Code: `generators::bake::{build (new), route, hydro, mod}`, `examples/landmass_scale`, `tests/bake_compact`, `tests/bake_round`. `route_rules` and `verify` are the old calls (no bit moves); nothing in the default bake calls the plan.
- Not in this change: a tile-wise route, the production call site of the plan (the world build), G-SED's partition test (GEN-155).
