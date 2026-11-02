## Why
SPIKE-95 (B9): SPIKE-90 window 2 lost 26 of 110 minutes to GPU cool-downs that hit their 120 s cap, before streaming scenes whose verdicts take no GPU time. PR 2 of SPIKE-95's plan (opus-planner, inbox #72); ADR 0094 amends ADR 0057 point 1.

## What Changes
1. Streaming scenes take no cool start (TEST-037); the GPU temperature at the start stays in the log's clock sample.
2. The views' cool start keeps its 60 C target with a 30 s cap instead of 120 s (TEST-033).
3. `perf --ab` and `perf reserve` keep a duty idle between runs: half the previous run's wall time, at most 45 s (BUILD-033); `planet_perf::clock::duty_idle_s` is the one rule, which the window runner (PR 3) reuses.

## Impact
- Code: `planet_perf` (`clock.rs`, `ab.rs`), `xtask` (`perfab.rs`), `planet` app (`streamperf.rs`). Tier A.
- Specs: TEST-033, TEST-037, BUILD-033 (modified). ADR 0094.
- No perf window in this PR; the proving measurement (10 runs with the duty idle against 10 with the old cool-down) waits for the planner's quiet slot.
