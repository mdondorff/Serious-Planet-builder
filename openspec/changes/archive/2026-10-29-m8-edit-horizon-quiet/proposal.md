## Why
Window 27, criterion 6: the per-bake log showed 7 bakes in flight at the edit (dropped, STRM-060), so each row paid for a second wave. The planner decided on 2026-10-29 to measure the edit on a quiet horizon (log: `target/perf-w27c`, not committed).

## What Changes
1. The edit-latency scene waits for quiet horizon jobs (no bake in flight, no start for 1.5 s) before each edit and after each revert.
2. The first case of each altitude is also edited once during the wave; its table has no verdict column.
3. The bake diagnosis block names the bakes already running at the edit and the seconds waited.

## Impact
- Spec: `testing` gains TEST-066. TEST-065 and the `verdict (10 s, horizon maps)` table are unchanged.
- Code: `app/editperf.rs`. Criterion 6 rows of earlier windows are not comparable with later ones.
