## Why
SPIKE-95 (B9): perf windows ran from Git Bash loops. Window 1's script copied a previous log into a refused run; window 2's runner outlived TaskStop; neither recorded times; and every configuration ran its full rounds even when its verdict was settled after one (window 2: every knob 39-65 % over the reference). ADR 0087 point 7 (stop rules, start, end and active time per scene) was never built. PR 3 of SPIKE-95's plan (opus-planner, inbox #72).

## What Changes
1. `cargo xtask perf window <plan> [--no-preflight]` (BUILD-034): one process runs a declared plan of configurations (environments) and flights (perf commands). It does one release build and the preflight (a refusal stops the window), then rotated rounds with the duty idle (BUILD-035), bounded retries of invalid or refused runs, and the stop rule.
2. `planet_perf::window`: the plan parser, the rotation, the stop rule (settled worse after a valid round beyond 3x the 5 % margin; settled within after two valid rounds within it), the run loop over a `WindowRunner` and the table. All pure, tier A on fake runs.
3. Every attempt is kept with its output, log, start, end and idle time under `target/perf-window/<time>/`, with the table.

## Impact
- Code: `planet_perf` (`window.rs`, `ab.rs`: `Change::word`, `default_log` crate-visible), `xtask` (`perfwindow.rs`, dispatch, usage).
- Specs: BUILD-034 (added).
- Not in this PR: a shorter gate inside a window's children. The runner keeps every child's own 10 s readiness gate (ADR 0020), so no ADR changes. A quick re-check after the window's preflight would need an ADR 0020 amendment.
- The proving measurement: in the planner's quiet slot, minutes per valid result against window 2's 10 and window 1's 3.3.
