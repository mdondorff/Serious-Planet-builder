## Why
Window 10 (2026-10-27, `docs/milestones/M8/criterion-6-edit-window.md`): with the horizon jobs on, 3 of 19 edits are drawn within 10 s with 4 bake threads and 9 of 19 with 1 (horizon off: 13). The bake took the cores the chunk re-solves needed; a bake that an edit made stale kept running to the end. Planner (2026-10-27): the bake takes 1 thread while chunk work runs and up to 4 otherwise; a stale bake stops and is dropped.

## What Changes
1. The chunk horizon baker marches on 1 thread while a chunk job is queued or running (`ChunkManager::in_flight`), else on its configured threads (default the cores, at most 4).
2. `HorizonBaker::bake` returns `Option<Vec<u8>>`: `None` when the edit word of the chunk changed while the map was made (checked per block of heights and before the march). The job publishes and stores nothing, counts `HorizonStats::cancelled` and frees its slot; the chunk is baked again against the new word once the neighbourhood has settled.
3. `--horizon-threads N` for the edit-latency scene (the probe of window 10).

## Impact
- Spec: STRM-061 (new). Code: `streaming/chunks.rs`, `app/horizonworld.rs`, `app/worldfield.rs`, `app/lib.rs`, `app/editperf.rs`, `xtask/testcmd.rs`. Test: `streaming/tests/horizon_jobs.rs`.
- Measured in the next window against window 10 (3 and 9 of 19).
