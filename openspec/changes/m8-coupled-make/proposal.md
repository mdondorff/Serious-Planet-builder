## Why
M8 criterion 4 (the global coupled run at most 10 min per world version, GEN-042) has no measurement from the real bake. The `coupled` scene times a synthetic input at 2048; the only make from the 16384 bake (9,916 s) predates the fill-only run (GEN-065) and the progress line (STRM-051). Nothing can re-make a stored run without touching the shared store (opus-planner, 2026-10-27).

## What Changes
- `perf coupled --store DIR --global-res N --remake-into OUT` makes the coupled run of the stored bake afresh (the viewer's make: reduction, then the steps), ignoring any stored channel, after the readiness gate; log `perf/logs/coupled-make.json`.
- `OUT` is created if missing, is never the store, and lies under a `target` directory; the store is only read.

## Impact
- Spec: GEN-042 (the make from the stored bake); tier A test of the output-directory rule.
- Code: `app/coupledworld.rs`, `app/globalperf.rs`, `app/lib.rs`, `xtask/src/testcmd.rs`.
