# 0020 Autonomous performance runs behind a measured readiness gate
Status: accepted (owner, 2026-10-11; decided in the opus-planner session)
Date: 2026-10-11
Source: owner review of the "owner only" perf policy (2026-10-11); constitution CON-20, CON-21; M2 report addendum (first run on Balanced, run-to-run noise 0.749)
Note: 0018 and 0019 are reserved for the cache-key and tile-storage ADRs of PR #45.

## Context
`cargo xtask perf --machine-ready` only asserts readiness; nothing checks it. The owner typed the flag, and CLAUDE.md made a Rasierklinge measurement a stop condition. In practice the owner's laptop is on the charger during development, the power scheme is detected (but only warned about: the first M2 run was on Balanced and still printed "ok" verdicts), and the largest unchecked risk is other Claude sessions building or testing in parallel, which distorts exactly the CPU plan time the 8 ms budget measures. The owner wants perf runs to be autonomous by default, scheduled by the architect.

## Options (with trade-offs)
1. Owner confirms every run (status quo). Safe, but every perf-dependent change waits for the owner, and the flag is still unchecked.
2. **The harness measures readiness and refuses when it is not met; the architect schedules runs in a quiet window** (chosen). Autonomous, and stricter than today because the conditions are measured, not asserted.
3. Run perf in CI on a self-hosted Rasierklinge runner. Fully automatic, but the machine is a laptop in daily use; out of scope for now.

## Decision
Option 2.
- **Readiness gate (before any measured run; refusal, not warning):** AC power connected; a performance power scheme; the reference adapter (existing check); a quiet machine: no `cargo`, `rustc`, `nextest`, `planet` or `clippy-driver` process outside the harness's own process tree, CPU load below 15 % averaged over 10 s, GPU utilisation below 10 % (`nvidia-smi`); GPU temperature below 70 degrees C. Each failed condition is reported with its measured value.
- **Validity (after the run):** no thermal or power throttling reported during measured frames; run-to-run noise (relative spread of run medians) at most 10 %, otherwise one retry, then the session is invalid. Only a valid session's verdicts count, and an invalid session says so on its first output line.
- **`--machine-ready`** remains as an explicit owner override of a failed gate; the override and the failed conditions are recorded in the log (`owner_override`).
- **Scheduling:** only the architect starts perf runs. It opens a perf window (coder sessions idle in `ListAgents`, or asked to pause builds), runs `cargo xtask perf`, records the result in the run log and `docs/status.md`, and closes the window. Coder sessions never run perf.
- **Owner keeps:** changing budgets or the measurement protocol, accepting an invalid session, and the final word at milestone gates.

## Consequences
- CON-21 (charger, performance profile, discrete GPU confirmed by the harness) is implemented, not changed; no constitution amendment.
- CLAUDE.md: the perf command line and the stop condition "a performance measurement is needed on Rasierklinge" change once the gate is merged; until then the owner confirms as before.
- Tests (tier A): every gate and validity rule is a pure function over measured facts (process list, loads, temperatures, throttle flags, run medians) with tests for each refusal; the platform queries are thin and logged.
- Easier: perf-dependent changes (plan time, draw counts, M3+ field generation) no longer stall on the owner. Harder: the harness must read Windows power and load state reliably.

## Licences of adopted code or techniques
None.
