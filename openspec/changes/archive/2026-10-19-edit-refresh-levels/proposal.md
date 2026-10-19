## Why
Window 14 (2026-10-19) attributed the exact time (380 to 1361 ms) to refresh throughput: 58 to 542 tiles are refreshed for a stamp of 50 m to 500 m. Before any fix, the scene must say which tiles those are: drawn by the plan or only resident, at which level, and whether the stamp's change is below the level's sample spacing (opus-planner, 2026-10-19).

## What Changes
- **TEST-041 modified**: the `edit-latency` scene prints and writes a third table, the refreshed set of the first run of each case by level: the level, its worst-case sample spacing, the tiles the edit named, how many of them the drawn plan contained (the rest are resident only) and how many lie at a level whose spacing is above the stamp's largest change.

## Impact
- Spec: TEST-041 (modified, testing). Code: `crates/app/src/editperf.rs`, the smoke test `gpu_edit_perf.rs`. No golden, budget or format changes.
