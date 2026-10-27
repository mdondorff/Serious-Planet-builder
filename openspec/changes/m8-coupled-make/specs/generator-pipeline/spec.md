## MODIFIED Requirements

### Requirement: GEN-042 Global coupled run time
The global coupled run (GEN-040) at 2048 cells per face edge and 100 steps SHALL take no more than ten minutes on the reference machine, measured by the performance harness through the architect and never in a test (CON-20). The `coupled` perf scene (`perf_coupled`) cites this requirement for spec-lint (TEST-001). The harness entry is `cargo xtask perf coupled` (ADR 0020 readiness gate, log `perf/logs/coupled.json` with wall time, steps, seconds per step and threads); the run is single-threaded, so `threads` is 1. `cargo xtask perf coupled --store DIR --global-res N --remake-into OUT` SHALL time the make of the run from the stored bake in `DIR` (the viewer's make: the reduction of the bake, then the steps; any stored channel is ignored), against the same ten minutes, log `perf/logs/coupled-make.json`; `OUT` SHALL be created if missing, SHALL NOT be the store and SHALL lie under a `target` directory (the store is only read).

Verify: C
Status: active
Source: opus-planner (2026-10-24), M8 exit criterion 4

#### Scenario: Coupled run timing
- **WHEN** the coupled run of 2048 cells per face edge and 100 steps runs on the reference machine in a perf window
- **THEN** it finishes within ten minutes
