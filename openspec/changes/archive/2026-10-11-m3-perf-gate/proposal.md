## Why

ADR 0020 (accepted by the owner, 2026-10-11): `--machine-ready` was an unchecked assertion, and the largest unmeasured risk to the 8 ms CPU plan budget is other sessions building in parallel. The harness should measure readiness and refuse, so perf runs can be scheduled by the architect session without the owner confirming each one.

## What Changes

- `planet-perf::gate`: pure rule functions over measured facts (`readiness`, `offending_processes`, `is_throttling`, `noise_needs_retry`, `validity`) and thin platform queries (AC power, process table, CPU load and GPU utilisation over 10 s, GPU temperature).
- The terrain and skeleton perf workloads call the gate after the (never overridable) adapter check and refuse with every failed condition and its measured value. Throttling and noise are judged after the run; a noisy view is retried once; an invalid session says so on an `INVALID SESSION` line and in the log.
- `--machine-ready` is now the owner's override of a failed gate; the log records it as `owner_override` (replacing `owner_confirmed_machine_ready`), and an overridden session is not valid for budgets.
- `cargo xtask perf` no longer demands the flag; it passes it through as the override.
- Modified: TEST-012. CLAUDE.md: the stop condition is reworded (owner approved).
