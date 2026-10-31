## 1. Change
- [x] 1.1 Planner confirmed the design and the order; IDs BUDG-008, 009, 010 reserved (2026-10-31)
- [x] 1.2 Spec deltas (BUDG-004 modified; BUDG-008, 009, 010 added)

## 2. PR 1: tile pool
- [x] 2.1 Tier A: the spawn scan with an allow-list of the sites not yet moved (BUDG-004, BUDG-010; xtask/src/threadscan.rs)
- [x] 2.2 Tier A: the share ceiling and the class order on fake work (BUDG-008, BUDG-009; planet-streaming jobs.rs)
- [x] 2.3 PR 2: `GenWorkers::pool` and `pool_with_idle` return an `OwnedShare` (a job system of `workers` threads, share `terrain` with ceiling `workers`; the idle cap of STRM-060 is the share's); behaviour unchanged. The perf check of the overhead is the planner's call (no `cargo xtask perf` without the go)
- [ ] 2.4 Verifier; counts companion (jobs by class, tier B); tell the planner when a perf measurement is due

## 3. PR 2 to 4
- [x] 3.1 Chunk pool (gone with B6); plan workers (4.1); horizon bakes and the scoped workers (5.1); the allow-list holds only the job system, `Par` and the drivers and tools that stay

## 4. PR 3a: plan worker
- [x] 4.1 `JobSystem::with_frame_workers`, `Share::classed`; the pipelined rig runs tile and plan workers on one system and one share `terrain` (ceiling workers + 1), the plan worker as a frame-critical thread at normal priority; `ThreadPool` has no production caller
- [x] 4.2 PR 3b: `planet_core::par::Par` (`impl Par for usize` = plain scoped threads), `HeightField::heights_threaded_until` takes `&dyn Par`, `eval_chunked` and the lattice gather run on it; `Share::try_acquire`/`par()` (permits of the terrain share, no unsafe); `PlanWorker::with_par`, the rig passes the terrain share's `Par`. Tests: bit-exact heights for 1, 2, 7 threads and a fake `Par` (GEN-020, CON-14); the permit rule on the share. Spawn cost: `frame/tests/par_cost.rs`

## 5. PR 4: horizon bakes and the scoped workers
- [x] 5.1 `planet_core::par::map` (ordered results on a `Par`); the scoped sites of `generators` (`global`, `horizon`, `ocean`, `coarsehorizon`, `coupledinput`, `height`), `frame` (`water`, `look/atmosphere`, `scenes`), `cache/meshpack` and `app` (`skeleton`, `stills`) run on a `Par` (the callers' `threads: usize`, plain scoped threads, or the share's `Par`); the rig sets the terrain share's `Par` on the world (`WorldTerrain::set_job_par`) for the water reads of a tile job
- [x] 5.2 The horizon bakes are the idle class of the rig's `terrain` share (`share_with_idle`, ceiling workers + 1 + horizon, idle cap horizon; the system has `horizon` more threads), not a pool of their own; the tools without a rig (`editperf`, `envelopeperf`, `stills`) keep `enable_viewer_horizon` with a pool of their own
- [x] 5.3 The scan: the allow-list shrinks to the job system, `Par` and the drivers and tools; BUDG-004 turns `active`
