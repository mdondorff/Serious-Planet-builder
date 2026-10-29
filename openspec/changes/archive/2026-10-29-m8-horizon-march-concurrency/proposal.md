## Why
Window 27 showed marches of 21 to 26 s with 7 at once against 2.06 s alone (planner, 2026-10-29). Whether thread priority or concurrency explains it needed a measurement.

## What Changes
1. Ignored measurement `horizon_march_concurrency`: one warm map at 1, 2, 4 and 7 bakes at once, with the threads at normal and at below-normal priority (`app/tests/horizon_world.rs`).

## Impact
- No requirement and no behaviour change; no timing claim is made by this change.
- The numbers of 2026-10-29 06:50 to 08:30 (marches of 14 to 26 s) were taken while the CPU was throttled to 17 to 18 % of its performance level (`cargo xtask perf preflight`: single thread 373 to 410 ms against 52 ms) and are inflated by that factor; they are not a result. The test is for a later quiet, calibrated window.
