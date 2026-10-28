## Why
The next perf window should show when the near horizon maps land, not only the last (planner, 2026-10-28; window 26 criterion 6).

## What Changes
1. The edit-latency horizon table `verdict (10 s, horizon maps)` has a `distance m` column (the map's distance from the camera; rows are nearest first); the verdict stays the last column (TEST-065, `accept M8`).
2. Ignored measurement `horizon_march_time` (one map's march, with a byte hash); a line on the march in the learning of 2026-10-26.

## Impact
- No requirement changes. Code: `app/editperf.rs` (table and test), `app/tests/horizon_world.rs`. Docs: `docs/learnings.md`.
