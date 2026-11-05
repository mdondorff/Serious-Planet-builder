## Why
- The tier-B `oracle_stream_smoke` failed three times on the lavapipe runner (#1078, #1072, #1180) with "the scheduler_ms median is 1.1 to 1.5 ms, at most 0.5 ms expected" (docs/status.md, Tracked). The kept log of one failure (run 41908981919) shows the cause: the dive's frame (`record_ms`) took 132 ms against 2.5 ms on a development machine, the generation jobs ran at their usual speed (`load_run_us_median` 1,303 against 1,363 us), and the plan step was unchanged (0.34 ms); only the main-thread scheduler step was stalled, by a runner whose cores the software renderer's threads fill.

## What Changes
- The scheduler's median limit is the larger of 0.5 ms (the old bound, the floor) and 0.1 of the same run's `record_ms` median (TEST-090). The upload bound (1.6 ms) is unchanged: it has not failed.
- The test is not deleted, skipped or plainly loosened: a quiet machine keeps 0.5 ms, and a scheduler ten times dearer than its frame is found on a loaded one.
- The Tracked flake is removed from `docs/status.md`.

## Impact
- Code: `planet-perf` (`smoke.rs`). Requirements: TEST-090 added (it supersedes the scheduler bound of TEST-035).
