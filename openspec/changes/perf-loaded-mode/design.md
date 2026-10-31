## Gate split
`readiness(&MachineFacts)` returns `Vec<GateFailure>` today. It becomes `readiness(&MachineFacts, Mode) -> Gate { failures, load }`:

| Condition | Quiet | Loaded |
|---|---|---|
| AC power, power scheme, reference adapter | refuse | refuse |
| GPU temperature limit, GPU throttling | refuse / invalid | refuse / invalid |
| build and test processes, CPU load, GPU utilisation | refuse | recorded in `load` |
| CPU calibration (ADR 0065 limits) | invalid | recorded in `load` |

`Mode::Loaded` is chosen by `--loaded` only. `--machine-ready` keeps printing every failure as an override and not refusing; combining both flags is refused.

## Load record
`LoadRecord { cpu_pct, competing_pct, gpu_util_pct, top processes, calibration_single_us, calibration_parallel_us, cpu_perf_pct }`: the values the gate and the preflight already read, plus the CPU performance % (the localised counter name failed on the German-locale machine: look the counter up by index, or record `unavailable`).

## Verdict function
`loaded_verdict(kind, reading, budget, pinned) -> Verdict` with `Kind { Gpu, Cpu, FrameStreaming, Regression, Acceptance }`:
1. `FrameStreaming`, `Regression`, `Acceptance` (the exclusive verdicts of ADR 0087 point 3): `NeedsQuiet`.
2. `Cpu`: pinned (competing builds on the E-cores at BelowNormal, the scene on the P-cores at High) is judged at the 5 % margin; unpinned only at a 30 % margin; otherwise `NeedsQuiet`.
3. `Gpu`: 5 % margin; the report carries the +18 % note.
4. Under budget by the margin: `LoadedPass`; over budget: `Inconclusive` (re-run quiet), never a fail; within the margin: `NeedsWindow`.
The pinning mode of point 2 is not built here; until it exists a `Cpu` scene needs the 30 % margin.

## A/B helper
`cargo xtask perf --ab <ref> <perf command...>`: a worktree of `<ref>` under `target/ab/`, N rounds (default 5), each round runs the command in both trees (order alternating per round), retries a gate refusal up to a cap (counting and naming the reasons) and copies each run's log out before the next. The verdict compares the medians of the chosen metrics in one table (both values, the change in %, the margin). It never writes the store or the main tree's `perf/logs`.

## Decisions (opus-planner, 2026-10-31, under the owner's delegation for process ADRs)
1. In loaded mode the ADR 0065 calibration limits are recorded and do not invalidate the session; the verdict follows ADR 0087's rules. Quiet sessions keep ADR 0065 unchanged.
2. `perf --ab` takes its metrics from the command's JSON counts and metrics; the default pair is tile-ready (`load_run_us_median`) and frame p99.
