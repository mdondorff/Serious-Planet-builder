# Change: m8-perf-coupled

## Why
- M8 exit criterion 4 (`docs/briefings/m8-plan.md`): the global coupled run (GEN-040) takes at most 10 min on Rasierklinge per world version; the perf harness has no entry for it.
- Owner-approved perf window (opus-planner, 2026-10-24).

## What changes
1. `cargo xtask perf coupled [--global-res N] [--machine-ready]`: the `coupled` scene of `test-render --perf` runs the coupled run once from a fixed seed after the ADR 0020 readiness gate (default 2048 cells per face edge, 100 steps) and reports wall time, steps, seconds per step and threads.

## Tests
- Tier A (`app`): a 16-cell smoke run of the scene on the software adapter, the budget check, the option forwarding of xtask.

Expected golden changes: none.
