## Why

Perf window 2 (2026-10-16, RTX, 16384 store): in the dive `in_flight` was exactly 8 in every frame and 4 tiles were dispatched per frame (1,200 per 300 frames, 240 tiles/s) against the 1,000-1,190 tiles/s the store sustains on 20 threads. The request queue grew to 8,900 and the detail lag stayed at 3.5 to 14 levels below 2,000 km. Below 2,000 km the cap, not cold coarse tiles, is the cause.

The measurement (docs/learnings.md, 2026-10-16): the 8 is two magic numbers, `ThreadPool::new(threads.clamp(2, 8))` and `max_in_flight: 8`. The 4 per frame is the frame-boundary refill: the scheduler learns of a completion in `collect` at the start of `step` and `dispatch` refills there only, so a job of 18 ms on one of 8 workers occupies it for two frames of 16.7 ms: 8 jobs per 2 frames, 240 tiles/s, whatever waits in the queue (N / (ceil(D/T) x T) in general). Tier A did not see it because its rigs ran the tile jobs with `max_in_flight` 64 and a spawner that finished 24 or 10,000 jobs a frame: no worker cap, no job duration.

## What Changes

- streaming: `GenWorkers` (workers, ahead): one place that builds the tile generation pool and sets `StreamConfig::max_in_flight` (jobs running at once = worker threads) from it; default = the machine's hardware threads (not clamped to 8). `StreamConfig::queue_ahead` (default 0, so every existing configuration and digest is unchanged): jobs handed to the spawner beyond the running ones, waiting in the pool's queue, so a worker that finishes between two frames takes the next request at once; the app's default is as many as workers. Dispatch order is the request queue's order and the limit is `max_in_flight + queue_ahead`, so the request order is a function of the limit only (CON-14).
- streaming: `TimedSpawner`, a deterministic spawner with a generation-rate model (N workers, a duration per job on a model clock) for tier A.
- app: the pipelined loop and the streaming perf scenes take `--gen-workers N` and `--gen-ahead N` (`cargo xtask perf stream` passes them through); the generate path's `streamed_meshes` states its 4 + 4 through `GenWorkers`.
- tests (tier A): in-flight equals the pool size and stays while the queue is non-empty; dispatch per frame equals the completions; throughput over a held demand and over the dive script is at least 80% of N / duration for N = 4, 8, 20 (dive: 8, 20), and the previous refill (no queue ahead) is shown at about 54%; request order is a function of the job limit; teardown with a full pool runs no job.
- spec: STRM-020 (new), TEST-018 (modified: the dive case on the generation-rate model).

Not in this change: reserving cores for the frame thread (F8), the cold start of the dive (owner decision), the pre-filled dive (F9). No golden, pinned count or mesh version changes.
