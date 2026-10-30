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
