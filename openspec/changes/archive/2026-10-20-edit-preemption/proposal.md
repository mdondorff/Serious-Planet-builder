## Why
The owner's decision of 2026-10-20 (ADR 0072, accepted): "exact" for the M6 criterion "edit visible" means the tiles the plan draws; drawn tiles refresh first; refresh work made stale by a newer edit is not computed. Window 16 measured drawn-exact at 93 to 956 ms; a drag can start one refresh job per edit, each stopped by the next.

## What Changes
- **Pre-emption** (STRM-031): refresh requests that wait stay one entry (counted), running jobs of re-keyed tiles are stopped (counted), their results dropped unuploaded (counted); `Streamer::set_refresh_settle` holds a re-keyed tile's next request until the window has passed since the latest edit that named it (default 0: nothing held); `ManagedLoop::set_refresh_settle`; the editor window sets 40 ms.
- **Drawn first**: stated and tested (the draw class already goes first).
- **Edit-latency scene** (TEST-041 modified): the budget verdict reads the first frame whose draw plan is exact; `--settle-ms N` (default 40); the stages table prints the settle and the three counts; the all-exact time stays a recorded column.
- **Not in this change**: window 18 (the re-measure, on opus-planner's go); the 200 m / 150 m case's speed.

## Impact
- Spec: STRM-031 (added, streaming-cache), TEST-041 (modified, testing).
- Code: `streaming` (`lib.rs`: held map, counters, setter), `frame` (`managed.rs`: setter), `app` (`editwin.rs`/`window.rs`: the editor sets 40 ms; `editperf.rs`: verdict, columns, flag).
- Goldens: none. Default behaviour (settle 0) unchanged for every other user of the streamer.
- spec-guardian and verifier before the PR.
