## Why

Perf window 3 (2026-10-16, RTX, 16384 store pre-filled, 18 workers + 18 queued ahead) showed 36 tile jobs in flight and only about 6 dispatched a frame (about 370 tiles/s) against the 1,000-1,190 tiles/s of the standalone bench. Candidates: per-tile time inflated by contention between workers; the 31-block cache thrashing at 36 concurrent tiles; the pack read the pre-fill added. The F10 ablation could not reproduce the slowdown (baseline 10-11 dispatched a frame) and had no counters to say where a tile's time goes.

## What Changes

- streaming: the streamer logs every tile job that ran its load (`JobLog`, `Streamer::job_log`): the wait from dispatch to start, the load time, the number of loads running when it started, the level and whether it produced a tile. Recorded only; no decision reads it.
- cache: the block source counts its shared-cache hits, evictions, reads that waited for another thread's decode, and the time spent on the cache lock, the fetch and the decode (`SourceStats`); per thread, the time spent getting blocks and the blocks decoded (`thread_block_wait_ns`, `thread_block_decodes`); the tile store counts its lookups, those that found a record, and their time (`TileStore::read_stats`).
- app/perf: the world terrain counts the pre-filled tile lookups, hits, skips and their time, the fields built, and the jobs' own block time and decodes (`WorldTerrain::counters`); each streaming case records the `load_*` counts of its measured frames and a CPU calibration (a fixed ALU kernel alone and on the pool's thread count at once, before and after the case) in the log; the `load_` counts are never compared with an earlier log.
- docs: the measurement in docs/learnings.md (workers 1 to 18, 8 / 18 / 36 in flight, cache budgets, planner threads, pack lookups, page cache) naming the cause.
- spec: STRM-022 added.

Not in this change: any change of what is generated, requested or in which order; the block-cache size (2 GiB would give +2-3 % and halve the job p99, at 1.5 GB more RAM: not taken); a cap on the planner's parallel block reads (worth about a tenth of the generators' time, costs plan latency: left to the coarse summary of ADR 0045). No golden, pinned count, mesh version or budget changes.
