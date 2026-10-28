## Why
`cargo xtask accept M8` ran the generic steps and printed "No automated criteria"; the hardware verdicts of criteria 4 to 7 were read by hand from the logs (acceptance runbook, 2026-10-28).

## What Changes
- An `M8` arm: `planet_perf::accept_m8` judges criteria 4 to 7 from `coupled-make.json`, `shadow-cost.json`, `edit-latency.md` and `envelope.json` against the spec's budgets (TEST-059); the manual list names criteria 1, 3 and 6 and CI.

## Impact
- Spec: TEST-059. Code: `crates/perf/src/accept_m8.rs`, `xtask/src/accept.rs`. Nothing is measured by the command.
