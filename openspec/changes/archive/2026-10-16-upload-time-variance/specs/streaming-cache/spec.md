## MODIFIED Requirements

### Requirement: STRM-020 Dispatch follows the generation pool
The number of tile jobs the scheduler lets run at once (`StreamConfig::max_in_flight`) SHALL equal the number of threads of the tile generation pool, derived from it (`GenWorkers`) and not stated separately; the default is the machine's hardware threads less a reserve of `GEN_RESERVE` = 2 cores kept for the frame thread, the plan worker and the driver's threads (perf window 2: with a worker on every core a frame's upload and scheduler steps wait for the OS to schedule them), never below one worker and never above the hardware threads (`GenWorkers::workers_for(cores, reserve)`, a pure function; a named default, not a measured optimum; 18 workers on the 20 threads of the reference machine), a parameter of the pipelined loop and the streaming perf scenes (`--gen-workers N` fixes the size outright and the reserve then decides nothing, `--gen-reserve N` changes the reserve, 0 giving a worker per hardware thread), and the plan worker's threads are not part of it. The reserve and the size change when jobs run, not what is requested or in which order, so no result depends on them (CON-14). While requests wait, every frame SHALL refill every free place so that the jobs in flight equal the limit, in the order of the request queue (STRM-001, STRM-005), and a worker that finishes between two frames SHALL NOT idle until the next frame: the scheduler hands the spawner `queue_ahead` jobs beyond the running ones (`--gen-ahead N`, default the worker count in the application, 0 in `StreamConfig::default()`), dispatched in queue order and cancelled like any other job (a queued job stops at its start). The jobs handed over at once are `max_in_flight + queue_ahead`; the order of requests SHALL be a function of the wants, the resident set, the failure times and that limit, not of the spawner or the thread count (CON-14), and the cancellation, teardown and failure semantics of STRM-005 are unchanged. With a held demand larger than the pool can finish, the tiles generated per second SHALL be at least 80% of workers divided by the job duration for 4, 8 and 20 workers, where refilling at frame boundaries alone reaches about half of it for jobs just longer than a frame. The default limit depends on the machine, so a run that must be reproducible (CI, goldens, a perf comparison) pins `--gen-workers` and `--gen-ahead`.

Verify: A
Status: active
Source: perf window 2 (2026-10-16), M4 fix, STRM-001, STRM-005, TEST-018, ADR 0031, ADR 0032, CON-04, CON-14

#### Scenario: In flight equals the job limit, every worker busy
- **WHEN** a demand larger than the pool is held on N = 4, 8 and 20 model workers
- **THEN** from the first frame the jobs in flight equal N plus the queue ahead and N workers are busy in every frame in which requests wait, and from the second frame each frame dispatches exactly the jobs that completed since the last

#### Scenario: Throughput follows the generation model
- **WHEN** the same demand, and the dive script, run on workers of 18 ms a job with frames of 1/60 s
- **THEN** the tiles per second are at least 80% of N / 18 ms (N = 4, 8, 20 for the held demand; 8 and 20 for the dive), and with no queue ahead (the previous refill) fewer than 70% of it

#### Scenario: Same order for the same limit
- **WHEN** wants that move are run with limits 8 + 0, 4 + 4 and 2 + 6
- **THEN** the dispatch order is identical

#### Scenario: Teardown with a full pool
- **WHEN** the streamer is dropped with N running and as many queued jobs
- **THEN** no job loads a tile afterwards and none is left waiting

#### Scenario: Default pool size
- **WHEN** the pool size is derived for 1 to 64 hardware threads and reserves of 0 to 70
- **THEN** it is the threads less the reserve, one when the reserve covers them all, never above the threads; the default reserve is 2 (18 workers for 20 threads, 2 for 4), the machine's default equals that rule on its threads with as many jobs queued as workers, and `--gen-workers` overrides it
