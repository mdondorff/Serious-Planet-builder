## Why
ADR 0083 point 2: background work runs on one job system with a core budget per subsystem and priority classes; no subsystem creates threads outside it. Today the tile pool (cores − 2 = 18), the chunk pool, the plan worker and the horizon bakes each size their own threads, and about 25 spawn sites create threads outside any pool (scoped workers of height, bake, ocean, horizon, water and atmosphere jobs). Measured: 4.7 to 5.4 busy cores in a flight, up to 17.5 of 20 (SPIKE-44). BUDG-003 and BUDG-004 are planned and unmet.

## What Changes
One pool per PR, in this order (planner, 2026-10-30):
1. Tile pool (`GenWorkers::pool`, `ThreadPool` in `planet-streaming`) becomes the job system's `terrain` share with the classes of BUDG-004.
2. Chunk pool: only if anything remains after B6 (ADR 0086 removes the chunk stage).
3. Plan workers (`pipelined::Rig`'s plan pool and the plan job's scoped height workers).
4. Horizon bakes and the generator-side scoped workers.
The spawn scan of BUDG-004 starts as a tier-A test with an allow-list of the sites not yet moved; each PR shortens the list; the last PR empties it.

## Impact
- New: the job system (placement to be decided in design.md: `planet-streaming`, which already holds `ThreadPool`, `JobClass` and `Priority`).
- Touches `crates/streaming`, `crates/generators`, `crates/frame`, `crates/app`: files the B6 lane (remove-chunk-stage) also edits; each PR names its files to the planner before the edit.
- Specs: BUDG-004 (scan test, shorter allow-list per PR), BUDG-003 (busy-thread ceiling per share) and new requirements for the pool API; IDs from the planner.
- Perf: no `cargo xtask perf` without the planner's go; ADR 0087 verdict rules apply (a loaded PASS at a 5 % margin; CPU scenes count only when pinned).
