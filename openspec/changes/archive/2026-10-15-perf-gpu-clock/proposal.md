## Why

M4 item 6b. GPU milliseconds depend on the GPU clock state (ground-1m 2.9-3.0 ms at P0 about 1.4 GHz on 2026-10-11, 4.1-4.2 ms at P2/P3 about 1.15 GHz on 2026-10-13; SPIKE-06 read 2.85-6.0 ms between process runs), because the CPU-bound frame leaves the GPU half idle. The harness recorded the state but acted on none of it, so the regression report compared GPU medians across clock states. Proposed ADR 0034 amends ADR 0020 in part.

## What Changes

- perf crate: `clock` module (view clock state, P0 validity, equal-state rule with 5 % tolerance, `warm_up` loop over closures); the log gains per-run `gpu_valid` / `gpu_clock_state` and `warmups`; the reader derives GPU validity per view from the recorded samples; `comparison_lines_gpu` compares `gpu_ms` and `frame_ms` only at equal clock state against a separate baseline `terrain.last-gpu-valid.json` and reports `not comparable: clock state X vs Y`.
- app: the terrain perf scene warms up each view to P0 (30 s cap) before its runs, prints `NOT VALID for GPU budgets (state)` instead of the GPU and frame verdicts of a view outside P0, and keeps both baselines.
- testing spec: TEST-012 text and four scenarios.
- Not run on the reference GPU (the architect's perf window does that); no golden or pinned count changes. Existing compare fixtures gained P0 clock samples (a log without samples is not valid for GPU figures).
