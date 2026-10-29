## Why
Window 27 (criterion 6, stage off): 500 m edits take 40 to 60 s to the last horizon map, while 12 maps at about 2 s over 8 threads would take about 5 s. The planner asked where the time goes (2026-10-29).

## What Changes
1. `ChunkHorizonBaker` times its steps (edit words, field build, height tiles, march): `bake_times()` sums them over threads; a diagnosis log of bakes (`bake_log_start`, `bake_log_take`, `BakeRecord`: start, end, step times, made or dropped) is off unless started.
2. The edit-latency report gets a section per case: the edit to the exact frame, the first bake's start, the bakes dropped by a later edit, and per bake its start, end and step times (`horizon_diag`).
3. The edit-latency scene takes `--altitudes-m` and `--radii-m` to run named hill cases only (a diagnosis of one case, not a criterion run).
4. Ignored measurement `horizon_bake_split` (12 maps, 8 at once, cold, warm, after hill edits of 150, 500 and 2000 m reach).

## Impact
- No requirement changes. Code: `app/horizonworld.rs`, `app/editperf.rs`, `app/lib.rs`, `app/tests/horizon_world.rs`.
