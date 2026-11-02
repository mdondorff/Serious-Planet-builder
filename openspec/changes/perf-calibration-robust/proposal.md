## Why
SPIKE-95 (B9): in the last three perf windows 53-75 % of attempts gave no verdict, all from the ADR 0065 calibration. Each reading is one ~50 ms timing on an unpinned thread, the streaming scenes read it before the cool-down on a hot CPU, a failed calibration voids GPU verdicts it says nothing about, and the reserve test's own loads void its sessions. PR 1 of SPIKE-95's plan (opus-planner, inbox #72); ADR 0093.

## What Changes
1. `calibrate_cpu`: the single reading is the smallest of 5 timings, the parallel reading the repetition with the smallest median of 3 (a pure reduction, tested at tier A).
2. Streaming scenes take the "before" reading after the settle and the cool-down, right before the first measured frame (lit views already do).
3. A calibration outside the limits voids the frame verdicts only; `gpu_session_valid`, the console lines and `accept`'s re-judgement follow.
4. A scene that starts its own load before its calibration logs `load_calib_declared_load` = 1 (the reserve test); its readings are recorded, not a cause.

## Impact
- Code: `planet_perf` (`streaming.rs`, `validity.rs`, `calibration.rs`), `planet` app (`streamperf.rs`). GPU-free logic, tier A.
- Specs: TEST-039, STRM-022 (modified). ADR 0093 amends ADR 0065.
- No perf window in this PR; the proving measurement (20 preflights, old against new, plus the slow terminal) waits for the planner's quiet slot.
