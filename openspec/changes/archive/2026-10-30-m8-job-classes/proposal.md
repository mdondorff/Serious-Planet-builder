## Why
Window 8 (2026-10-27, `docs/milestones/M8/criterion-6-edit-window.md`): with the horizon jobs on, the eroded result of an edit is drawn within 10 s in 2 of 19 cases; with `--no-horizon-jobs` in 13 of 19. The horizon job shares the chunk pool and is in flight for the whole 10 to 17 s of the re-solve. Owner via planner (2026-10-27): horizon jobs yield to chunk jobs, as a priority inside the one shared pool (ADR 0083 point 2: visible streaming and edits above idle).

## What Changes
1. `JobClass { Visible, Prefetch, Idle }` and `Spawner::spawn_in(class, job)` (default: `spawn`, which is Visible).
2. `ThreadPool` keeps a queue per class: a free worker takes the best class with a job waiting, FIFO within a class, no preemption. `ThreadPool::with_classes(threads, priority, idle_cap)` lets idle jobs run on at most `idle_cap` workers.
3. The viewer's chunk pool is `VIEW_CHUNK_WORKERS` threads plus one for the idle class (`GenWorkers::pool_with_idle(1)`): a bake never holds a worker a chunk job needs; the horizon jobs (`HorizonConfig::workers` 1) run in `Idle`.
4. `ManualSpawner` runs the best class first.
Explorer's chunk ordering by time-to-need (#780) is the order of submissions inside `Visible`; prefetch chunk jobs may use `Prefetch`.

## Impact
- Spec: STRM-060 (new). Code: `streaming/lib.rs`, `streaming/chunks.rs` (the horizon spawn), `app/anetworld.rs`, `app/chunkstage.rs`. Test: `streaming/tests/job_classes.rs`.
- Measured after the change in one window (edit-latency, horizon on, against window 8).
