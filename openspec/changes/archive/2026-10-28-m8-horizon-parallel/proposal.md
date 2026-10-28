## Why
M8 criterion 6 failed in window 26 (2026-10-28, stage off): about 60 % of the rows of the table `verdict (10 s, horizon maps)` are over 10 s; the maps of one edit landed one after another, 0.85 to 1.2 s apart, and an edit that touches 12 to 25 maps took 10 to 39 s. Cause: without a chunk stage the horizon jobs shared the one idle thread of the chunk pool (`pool_with_idle(1)`), so one bake ran at a time, with 4 threads inside it.

## What Changes
1. `enable_viewer_horizon` gives the horizon jobs a pool of their own: `horizon_workers` = half the generation workers, at most 8, all idle class, one thread a bake (`HORIZON_WORKERS_MAX`). The window and the edit-latency scene use it.
2. `ChunkHorizonBaker`: bakes that need the same height tile at once wait for the first to evaluate it (a gate a chunk), so neighbours baked together do not evaluate the shared tiles several times.
3. STRM-064 states the pool; tests: `horizon_world` (shared tile evaluated once under concurrent bakes, same maps as one after the other; the worker count) and an ignored throughput measurement.

## Impact
- Spec: STRM-064 (modified). Code: `app/anetworld.rs`, `app/horizonworld.rs`, `app/worldfield.rs`, `app/window.rs`, `app/editperf.rs`. Tests: tier A/B (`horizon_world`).
- ADR 0083 share: up to 8 threads in the idle class for the seconds after an edit or a move into new chunks (before: 1 pool thread plus 4 inside the bake).
- Measured on Rasierklinge (12 maps, while other sessions built; `horizon_bake_throughput`): one bake at a time 21 to 50 s, 8 at once 9.5 s warm and 16.7 s cold, 12 at once 8.1 s warm and 14.4 s cold. Expected after the fix: 12 touched maps land in 8 to 17 s on a loaded machine (about 2.5 to 3x faster than before); 25 maps take about twice that. The 10 s bound holds only for edits that touch up to about 8 maps; larger edits need fewer wanted maps (see the PR).
