## ADDED Requirements

### Requirement: STRM-019 Scheduler work follows changes
The frame thread's scheduler work in a frame (collecting completions, applying the wants, queueing requests, making room, dispatching) SHALL be proportional to what changed since the previous frame (the tiles added to or removed from a request class, the completions, the requests that start, the evictions they need), not to the size of the wants. A frame in which nothing changed SHALL touch no element (the scheduler counts the elements it reads in sequential passes and the elements it writes, `SchedWork`, CON-20); a frame in which the draw set turns (the plan changed, R did not) SHALL write no tile of the residency or prefetch class and read only the draw set; a frame in which c tiles join or leave R SHALL write a number of elements proportional to c (times the levels climbed), independent of how many tiles R holds, plus one sequential pass over the lists that differ; a priority change alone SHALL write no tile of a class and re-order no request that nothing reads. The requests, their order, the cancellations, the evictions and the answers of every frame SHALL be those of a scheduler that recomputes everything from the wants each frame (STRM-005, STRM-007, CON-14): the order does not depend on how the wants were handed over (all lists every frame, or only those that changed), the spawner or the thread count. The residency manager SHALL fit the wants to the pool (STRM-006, STRM-007) only when R with the closure of D could exceed it, and when the pool holds them SHALL report the numbers of the pure fit without running it. The frame thread's scheduler time (`scheduler_ms`, TEST-012) is reported per frame; its target is a p99 of at most 2 ms in every streaming scene, tracked in the perf window (a time budget is measured on the reference machine only, CON-20).

Verify: A
Status: active
Source: perf window 1 (2026-10-16), M4 fix F2, STRM-005, STRM-007, STRM-017, CON-14, CON-20, ADR 0031

#### Scenario: Settled frame
- **WHEN** every tile of R is resident, nothing is queued or running and the wants do not change, for R of N and of 4N tiles
- **THEN** a scheduler frame reads and writes no element, through the streamer and through the managed loop (still camera, 30 frames)

#### Scenario: Turn
- **WHEN** the camera turns at a fixed position with R resident (the plan changes every frame, R does not)
- **THEN** no tile of the residency or prefetch class is written in any frame, the reads are those of the draw set, and nothing is requested or cancelled

#### Scenario: Changes proportional to c
- **WHEN** c = 1, 10 and 100 finest tiles are removed from R, or c finer tiles are added below its leaves, with R of N and of 4N tiles
- **THEN** the elements written are at most 4c (removal) or 8c (addition, including the requests that start) and are the same for N and 4N, and the elements read are those of one pass over R and the c tiles

#### Scenario: Same decisions as the full recomputation
- **WHEN** the scripted runs (streamer alone with pools that hold R, that are smaller than R, with failures and a vertex pool; the managed loop with prefetch, an upload budget and pools smaller than R) are replayed
- **THEN** the full per-frame record (answers, counts, dispatch order, resident changes, written tiles) equals the digests taken from the scheduler that recomputed everything every frame
