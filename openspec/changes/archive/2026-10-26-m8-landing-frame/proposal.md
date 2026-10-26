## Why
SPIKE-44 step 2: a chunk landing frame of up to 729 ms on the main thread (the "chunk frame" phase), breaking CON-22. Measured with timers around each step of the viewer's chunk step (run through the envelope Driver, which does the same: a 40 s flight at 300 m, 150 m/s, 8,100 to 8,500 resident tiles): `chunk_landed` took 438 to 672 ms, 99 % of it in `PlanWorker::invalidate_chunk`. The scan of the plan cache for the landed chunk costs 34 ms at a full cache of 371,000 tiles (a micro-benchmark); the rest was the main thread waiting for the plan cache's mutex, which the plan job holds for its whole plan (hundreds of ms to seconds when chunk and horizon jobs take the cores). `edit()`, which `chunk_landed` calls, takes the same lock, so deferring the scan alone left the stall (28 to 2,151 ms measured).

## What Changes
- `PlanWorker` queues every change the main thread asks of the plan cache (landed chunks, `invalidate_cache`, `note_item_change`) in one ordered list; the plan job applies it under the cache's lock before it plans (landed chunks that follow one another in one pass). The main thread takes no lock of the cache.
- `ChunkManager::reads` uses a reach computed once at construction (it was recomputed with `libm` log2 for every tile the scan asked about, 15 times the cost of the lookup).
- STRM-040 says the drop happens before the next plan reads the cache; STRM-058 says the main thread never takes the cache's lock (the queue).

## Impact
- Spec: streaming-cache STRM-040 (the wording) and STRM-058 (new).
- Code: `frame/src/pipeline.rs` (the queue), `frame/src/lod.rs` (`invalidate_chunks`, one pass), `frame/src/managed.rs`, `streaming/src/chunks.rs`.
- Test: `a_landing_only_queues_the_drop_and_no_plan_that_starts_after_it_reads_a_tile_that_reads_the_landed_chunk` (frame/tests/managed.rs).
- Re-measured with the same probe: `chunk_landed` on the main thread 0.9 to 2.1 ms (8 landings), before 438 to 672 ms (and up to 2,151 ms with only the scan deferred); the 88 logged slow frames of the first run are 28.
- Not covered (a second finding): `chunk_frame` and `horizon_frame` calls of 3 to 129 ms without a landing (later 3 to 63 ms), the main thread blocked on the chunk or horizon state lock or starved by chunk and horizon jobs; to be separated with the lock-wait counters.
- An edit's invalidation (`invalidate_cache`, `note_item_change`) is applied at the start of the next plan job, in the order asked, instead of on the main thread under the cache's lock: the drawn-exact path of an edit is unchanged or faster (the main thread no longer waits for a running plan); the editor and edit-latency tests pass; crusader's next edit-latency scene run confirms it in numbers.
- The queue is bounded (STRM-058): a clear replaces what was queued before it, tile drops merge without duplicates, a chunk is queued once, a queue past the cache's capacity becomes a clear.
