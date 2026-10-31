## 1. Change
- [x] 1.1 Planner confirmed the design and the order; IDs BUDG-008, 009, 010 reserved (2026-10-31)
- [x] 1.2 Spec deltas (BUDG-004 modified; BUDG-008, 009, 010 added)

## 2. PR 1: tile pool
- [x] 2.1 Tier A: the spawn scan with an allow-list of the sites not yet moved (BUDG-004, BUDG-010; xtask/src/threadscan.rs)
- [x] 2.2 Tier A: the share ceiling and the class order on fake work (BUDG-008, BUDG-009; planet-streaming jobs.rs)
- [x] 2.3 PR 2: `GenWorkers::pool` and `pool_with_idle` return an `OwnedShare` (a job system of `workers` threads, share `terrain` with ceiling `workers`; the idle cap of STRM-060 is the share's); behaviour unchanged. The perf check of the overhead is the planner's call (no `cargo xtask perf` without the go)
- [ ] 2.4 Verifier; counts companion (jobs by class, tier B); tell the planner when a perf measurement is due

## 3. PR 2 to 4
- [ ] 3.1 Chunk pool (if B6 leaves it); plan workers; horizon bakes and the scoped workers; the allow-list empties

## 4. PR 3a: plan worker
- [x] 4.1 `JobSystem::with_frame_workers`, `Share::classed`; the pipelined rig runs tile and plan workers on one system and one share `terrain` (ceiling workers + 1), the plan worker as a frame-critical thread at normal priority; `ThreadPool` has no production caller
- [ ] 4.2 PR 3b: the plan job's scoped height workers (generators): needs the injected scoped fan-out in `planet-core` (CON-02, CON-04); planner decides
