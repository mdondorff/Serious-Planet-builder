## MODIFIED Requirements
### Requirement: STRM-020 Dispatch follows the generation pool
The number of tile jobs the scheduler lets run at once (`StreamConfig::max_in_flight`) SHALL equal the number of threads of the tile generation pool, derived from it (`GenWorkers`) and not stated separately; the default is the terrain share (BUDG-003; 6 on the reference machine, an example), which the rig then holds as its ceiling. `--gen-workers N` and `--gen-reserve N` override the share, uncapped: N workers, or the machine's hardware threads less N, never below one worker and never above the hardware threads (`GenWorkers::workers_for(cores, reserve)`, a pure function; perf window 2), a parameter of the pipelined loop and the streaming perf scenes (`--gen-reserve 0` gives a worker per hardware thread; an override's log records `pool: override N (share S)` and its console marks the frame verdicts as not at the BUDG-003 share), and the plan worker's threads are not part of it. The reserve and the size change when jobs run, not what is requested or in which order, so no result depends on them (CON-14). While requests wait, every frame SHALL refill every free place so that the jobs in flight equal the limit, in the order of the request queue (STRM-001, STRM-005), and a worker that finishes between two frames SHALL NOT idle until the next frame: the scheduler hands the spawner `queue_ahead` jobs beyond the running ones (`--gen-ahead N`, default the worker count in the application, 0 in `StreamConfig::default()`), dispatched in queue order and cancelled like any other job (a queued job stops at its start). The jobs handed over at once are `max_in_flight + queue_ahead`; the order of requests SHALL be a function of the wants, the resident set, the failure times and that limit, not of the spawner or the thread count (CON-14), and the cancellation, teardown and failure semantics of STRM-005 are unchanged. With a held demand larger than the pool can finish, the tiles generated per second SHALL be at least 80% of workers divided by the job duration for 4, 8 and 20 workers, where refilling at frame boundaries alone reaches about half of it for jobs just longer than a frame. The generation workers SHALL run at below-normal OS priority by default (`--gen-priority below-normal|normal`; ADR 0046): each worker thread sets its own priority before its first job (`ThreadPool::with_priority`, carried by `GenWorkers::priority`; Windows `SetThreadPriority(THREAD_PRIORITY_BELOW_NORMAL)`, Linux `setpriority` with nice +5 on the thread; a platform without the call, or a refusal, leaves the worker at normal priority and the pool working), so that the OS gives the frame thread its core back when both want it, which a reserve of cores alone does not give it once other processes or the driver's threads take the reserved cores (the ablation of docs/learnings.md, "Generation workers at below-normal priority"). A priority changes when a thread runs, never what it computes: the size, the job limit, the request order and every result are the same at either priority (CON-14), the pool's workers read back the priority they were built with (`priority::current`), and the calling thread keeps its own. The session log records it in `protocol.gen`. The default limit depends on the machine, so a run that must be reproducible (CI, goldens, a perf comparison) pins `--gen-workers` and `--gen-ahead`.

Verify: A
Status: active
Source: ADR 0092, perf window 2 (2026-10-16), M4 fix, perf window 3 and F10 (ADR 0046), STRM-001, STRM-005, TEST-018, ADR 0031, ADR 0032, CON-04, CON-14

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
- **THEN** it is the threads less the reserve, one when the reserve covers them all, never above the threads (18 workers for 20 threads and a reserve of 2, 2 for 4); the machine's default is its terrain share with as many jobs queued as workers, and `--gen-workers` overrides both, with the override and the share in the log (`pool: override 18 (share 6)`) and the console line that the frame verdicts are not at the BUDG-003 share

#### Scenario: Workers read back their priority
- **WHEN** a pool is built with below-normal priority, and one with the default, and each runs jobs that read the priority of their thread
- **THEN** on Windows and Linux every job of the first reads below normal and every job of the second reads normal, the thread that built the pool is unchanged, and `GenWorkers` of equal size give equal job limits at either priority

#### Scenario: Option
- **WHEN** the options are parsed without `--gen-priority`, with `below-normal`, with `normal` and with another value
- **THEN** the pool's priority is below normal, below normal, normal, and the last is an error naming both values

### Requirement: STRM-064 The horizon maps are baked from the base terrain
The horizon jobs SHALL bake the maps of the chunks of the plan's tiles (chunk-grid level 11 and finer) from the base terrain and the stamps, in the format, march and slots of STRM-048, and SHALL bake a map again when an edit or a change of the world word reaches it.

Verify: A
Status: active
Source: owner fallback 2026-10-28, opus-planner M8-F step 2, STRM-048, STRM-060, finding 16 of the owner test 2026-10-27, ADR 0092

Detail: Every chunk counts as ready with no change. The bakes run on a pool of their own in the idle class, at most half the terrain share at once, at least one (BUDG-003, ADR 0092) and one thread a bake: a map takes about 6 s on one thread and the march scales poorly inside a map (4 threads: 1.5x) and linearly across maps; bakes that need the same height tile at once wait for the first to evaluate it. The result does not depend on how many bakes run at once (CON-14). The jobs keep the maps of the wanted chunks (at most 24) and, of the others, the most recently wanted up to the configured number beyond them. An edit that reaches a chunk or the halo of its march, a neighbour included, changes its edit word; a change of the world word (the coupled lens, the base of the definition, the stage) changes every word; the map is baked again once its word has been unchanged for the settle window. A height tile of the bake is kept only if no edit was published since the field it was evaluated from was built.

#### Scenario: Base-only maps
- **WHEN** the horizon jobs run over the base terrain and two chunks are wanted
- **THEN** both start at once, land, and are baked again after an edit word change and the settle window; maps of chunks no longer wanted go beyond the cap, the oldest first

#### Scenario: Window without a stage
- **WHEN** the window runs lit over a store
- **THEN** `chunk jobs` stays `None` and the horizon jobs bake at least one map (judged by the real-store gate `cargo xtask gate`; the small-world run reports it, because a software adapter is too slow to wait for a bake)
