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
