## Why
`b5-wall-clock-out-of-tier-a` (#1029) left `cargo xtask perf wallclock` without the readiness gate and with no automated caller. The planner's answers (inbox #44):
- every perf command runs through the gate (ADR 0020), so a busy machine gives no verdict rather than a false red;
- the command is a step of `cargo xtask accept` and runs at the start of every perf window the planner authorizes.

## What Changes
- `xtask/src/testcmd.rs`, `perf wallclock [--machine-ready]`:
  - builds the checks first (a build beside the gate is a busy machine), then measures the machine (`planet_perf::gate::measure_machine`) and judges it with ADR 0020's gate minus the reference-adapter condition (CPU-only checks; xtask has no GPU adapter to name);
  - a refusal is `INCOMPLETE: perf wallclock NOT RUN: the readiness gate refused (ADR 0020): …` (exit code 3); `--machine-ready` runs the checks anyway; other options are refused.
- `xtask/src/accept.rs`:
  - a new last step, `perf wallclock (BUILD-003)`, for every milestone;
  - a step that ends `INCOMPLETE:` without a locked-desktop marker gets its text as its NOT RUN line, so a refused gate makes the step and the run INCOMPLETE, never a pass; the row and summary wording no longer assume a locked desktop.
- `xtask/src/main.rs`: the usage line of `perf wallclock`.
- Tier-A tests (`// spec: BUILD-003`): a ready machine without a GPU adapter passes the wall-clock gate and the full gate still refuses it; a build process and a CPU load are refused with their values; battery is refused; an unknown option is refused before anything runs; an `INCOMPLETE:` step is NOT RUN with its reason, and markers win over it.
- Not run: `perf wallclock` itself (its first run is in the next perf window the planner authorizes).

## Capabilities
### Modified Capabilities
- `build-tooling`: BUILD-003, the wall-clock scenario no longer says the command runs without a gate or outside `accept`; a new scenario for the gate and the accept step; ADR 0020 and inbox #44 in its sources.
- `testing`: TEST-073, a scenario for a step that gave no verdict without a marker.

## Impact
- `xtask/src/{testcmd,accept,main}.rs`. `cargo xtask accept` runs one more step (a build in release of three test targets, a 10 s gate, about 1 s of checks) and is INCOMPLETE on a busy machine, including a laptop where another session builds or tests.

## Departures (for the planner)
- The gate is ADR 0020's without its reference-adapter condition: the checks use only the CPU and xtask names no GPU adapter. GPU utilisation and temperature still apply (conservative). The tier-A test pins that the full gate refuses and the subset passes.
- ADR 0020 has only the architect start perf runs; `accept`, which coders run, now runs `perf wallclock` (inbox #44 asked for it; a CPU tolerance check behind the gate).
- `--machine-ready` prints the refused conditions; there is no perf log to record the override in.
- Running it at the start of each perf window is the planner's process; the code enforces only the `accept` step.
