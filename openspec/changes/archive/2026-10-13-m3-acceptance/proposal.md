## Why

M3 item 7: `cargo xtask accept M3` and the M3 report. ADR 0016 rejects terrain too steep for a crack-free morph interval, so M3 must show that realistic terrain plans at production parameters (cells 32, viewport / tau 240) on a production-resolution bake; the tier A tests plan on a 256-cell bake only.

## What Changes

- `planet generate --plan-check DIR --global-res N [--world F | --seed N]` (`crates/app/src/plancheck.rs`): plans the ten standard views over the stored bake with the terrain field and the eroded field, prints nodes, deepest level and time per view, and fails if a view is rejected or the bake cannot be read (a missing block is an error naming it, never a default). A tier A test runs it on a 256-cell bake and checks the refusals.
- `cargo xtask accept M3` lists the M3 criteria that no local run settles (CI determinism, the architect's perf windows, the owner's flight review, the gate).
- The M3 report `docs/milestones/M3-report.md` (this change) records the production-resolution result.
