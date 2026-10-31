## Inventory (2026-10-30, main 215db6ad)
| Pool or site | Where | Threads | Class today | Moves in |
|---|---|---|---|---|
| Tile pool | `streaming/lib.rs` `ThreadPool`, built by `GenWorkers::pool` | cores − `GEN_RESERVE` (18 on Rasierklinge) | visible and idle (`JobClass`, `idle_cap`) | PR 1 |
| Chunk pool | `streaming/chunks.rs` (`ChunkConfig.workers`) | 4 (M8-7) | one | PR 2 (if B6 leaves it) |
| Plan worker | `app/pipelined.rs` `ThreadPool::new(1)` | 1 | one | PR 3 |
| Plan job's height workers | `generators/height.rs` `heights-worker`, `tile-worker` (scoped) | up to 20 per call | none | PR 3 |
| Horizon bake pool | `app/horizonworld.rs` `GenWorkers::new(1).pool_with_idle` | 1 + idle | idle | PR 4 |
| Scoped bakes | `generators/global.rs`, `horizon.rs`, `ocean.rs`, `coarsehorizon.rs`, `coupledinput.rs`, `meshpack.rs` | per call | none | PR 4 |
| Scoped renders | `frame/water.rs`, `look/atmosphere.rs`, `scenes.rs` | per call | none | PR 4 |
| Drivers and tools | `app/{coupledworld,horizonworld,hoverpick,stall,streamperf,worldfield}.rs`, `perf/*` | 1 each | none | listed with a reason |

## Design (to confirm with the planner before PR 1)
1. The job system lives in `planet-streaming` beside `ThreadPool`: one `JobSystem` owns the workers (at most 18 on the reference machine, BUDG-003); a `Share` handle per subsystem carries a ceiling on jobs running at once and spawns with a `JobClass` (frame-critical, visible, prefetch, idle).
2. A scoped fan-out (`par_for`, `scope`) runs on the caller's share: it asks the share for workers up to the ceiling and runs the rest on the calling thread; it never creates a thread of its own.
3. `GenWorkers` keeps its role (pool size, `ahead`, priority) as the terrain share's configuration; its tests keep passing unchanged.
4. Results do not depend on the number of workers (CON-14); the tests of the moved callers stay as they are.

## Open
- Requirement IDs (planner): the pool API, the class order, the share ceiling test.
- Whether the plan job's scoped workers count against the terrain share or the frame-critical class (the plan is frame-critical: the draw waits for it).

## Decisions (opus-planner, 2026-10-31)
1. The plan job's fan-out counts against terrain's share, at the highest priority within it (`Class::FrameCritical`); it is not a share of its own.
2. The first PR adds the job system as a module (`planet-streaming::jobs`: `JobSystem`, `Share`, `Class`) with its tests on fake work and the spawn scan; no pool moves in it. The tile pool moves in the next PR, after B6 PR 4 (streaming); generators and frame pools after B6 PR 5.
3. `fan_out` takes a `'static` closure (an `Arc<dyn Fn(usize) -> R>`): safe code, no lifetime erasure. A scoped site that borrows large arrays wraps them in `Arc` or, if that costs too much, PR 3 adds a scoped variant and says so.
4. The calling thread of a fan-out works on the indices and is not counted in the share's ceiling; the helpers are.

## Decision: the plan job's scoped height workers (opus-planner, 2026-10-31; PR 3b)
1. The workers cannot call the job system (CON-02: generators sit below streaming; CON-04: spawning is injected). `planet-core` gets a `Par` trait; `HeightField::heights_until_threaded` takes `&dyn Par` instead of `threads: usize`.
2. The first version has no lifetime erasure. The Share's `Par` takes k permits from the terrain share (class frame-critical), then runs the borrowed chunks with `std::thread::scope` on k threads, the caller working too: safe Rust, and the share's ceiling holds because the permits are counted. The thread-creation scan lists that one site with its permit rule.
3. The spawn cost is measured against the plan job's time. Only above 1 % is a pooled version with erasure considered, in its own PR with a numerics-reviewer pass.
4. CON-14 test: the heights are bit-exact for 1, 2 and k chunks and for the fake `Par`.
5. Built after B6 PR 6 merges (it edits `generators`).

## PR 3b: what was measured (opus-planner's 1 % rule)
`frame/tests/par_cost.rs` (ignored diagnostic): one fan-out of 8 trivial chunks costs 252 us on the share's `Par` and 231 us on plain scoped threads (what the plan job paid before), medians of 201; a cold plan job of the 200 m view on the reference field (cheap to read) takes 14.3 ms. The fan-out is 1.8 % of that plan job, as it was before this change; what the share adds is 21 us (0.15 %). A plan over the production store is longer, so both ratios are upper bounds there. The pooled version with lifetime erasure is not needed for this change.

## Open point (opus-planner, 2026-10-31)
A pooled fan-out (helper threads taken from the job system's workers instead of scoped threads started per call) could save about 1.5 % of a cold plan job (the 1.8 % thread-start cost less the 0.15 % the share adds). No work now; it needs the lifetime erasure the first version avoids, in its own PR with a numerics-reviewer pass.

## PR 4: what moved and what stays
1. A scoped site takes a `Par`: tools and bakes keep their `threads: usize` argument (`impl Par for usize` is the scoped threads, results in index order, CON-14); a job of the rig reaches the terrain share's `Par` through the world (`set_job_par`: the water reads of a tile job).
2. The horizon bakes share `terrain` with the tile and plan jobs: one ceiling (workers + 1 + horizon), the idle class capped at `horizon` (STRM-060). The system grows by `horizon` threads, the sum of the old two pools.
3. Stay as drivers: `app/coupledworld` (the coupled run), `hoverpick`, `streamperf`, `worldfield` (the height warm-up thread waits on a queue: a job would hold a worker), `horizonworld` (the one-shot read of the stored coarse horizon). `ThreadPool` stays for tests and examples.
4. Risk for the perf window: the water reads of a tile job (8 helpers before) now take free permits of the terrain share; with the tile jobs at their limit the free ones are the idle slots (`horizon`) and the plan worker's. A cold lit tile may wait longer on its store blocks; the planner decides whether a window measures it (ADR 0087).
