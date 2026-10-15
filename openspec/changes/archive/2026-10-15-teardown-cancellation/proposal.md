## Why

M4 item 7 report: a smoke run of a perf scene over the 16384 store took minutes per case and the stop report blamed teardown "waiting on queued tile jobs". Measured (2026-10-15): the case spends 243 s in `Rig::new` (the six root tiles read from a cold store) and 9-14 s in teardown. The teardown wait is real but small: dropping a `Streamer` cancelled nothing, so the eight tile jobs in flight ran to the end, each a single uninterruptible stage (a cold tile reads store blocks for seconds); a plan job in flight chained its next pending plan, its residency set and its prefetch set after the teardown began. STRM-005 says an in-flight job stops at its next stage boundary and a queued one never runs; teardown (ADR 0031 point 3) was not covered.

## What Changes

- streaming: `Streamer::cancel_all` (queue emptied, every dispatched job's token set) and `impl Drop for Streamer` calling it; `CancelToken::cancelled()` for tests.
- frame: `PlanWorker::close`; `PlanPipeline` closes its worker on drop: a plan job that has not started does not run, a running one ends after its plan and starts no R, prefetch set or follow-up job.
- app: `MeshSource::load_staged`: every height sample of a tile polls the token (`CancelGuarded` answers 0 once cancelled, delegating every other method), so a cancelled tile job runs out in microseconds and its tile is dropped.
- perf scene: the case's teardown time is printed.
- STRM-005 gains the teardown and teleport wording and scenarios; tier A tests on fakes (queued jobs never start, in-flight ones stop at a stage boundary, nothing becomes resident, a teleport cancels the requests that left R before they start).

Not in this change: the 243 s root load of a cold 16384 store (open point in `docs/status.md`).
