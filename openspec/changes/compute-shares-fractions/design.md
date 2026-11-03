## Design notes

### The rule
- `P` is the logical threads on performance cores, SMT siblings counted, at most `available_parallelism`. E-cores are not counted (ADR 0092 point 1).
- `B = max(P − 2, 1)`.
- For each share: `n = clamp(floor(pct × B / 100), min, max)`, in integer arithmetic. Floor, not round, keeps the sum at or under B while no minimum binds.
- **Fit:** with this table every non-terrain share sits at its minimum whenever the sum exceeds B (`floor(pct × B / 100)` stays below the minimum for B below about 14), so only terrain can give up threads. The rule says exactly that, instead of naming a class order that could never act.
- If the minimums still exceed B, the machine runs at the minimums, oversubscribed (a share is a ceiling, not a reservation).
- If a later table lets a non-terrain share exceed its minimum on a machine that needs a fit, a fit order is needed then.

### Table (accepted by the planner 2026-11-02 as provisional except terrain)
| Share | f | min | max |
|---|---|---|---|
| terrain | 0.60 | 2 | 10 |
| physics | 0.15 | 1 | 4 |
| game logic | 0.15 | 1 | 4 |
| sky and weather | 0.05 | 1 | 2 |
| networking | 0.05 | 1 | 1 |

### Worked examples (examples, never rules)
| Machine | P | B | terrain | physics | game | sky | net | sum |
|---|---|---|---|---|---|---|---|---|
| Rasierklinge, 6P(12T)+8E | 12 | 10 | 6 | 1 | 1 | 1 | 1 | 10 |
| 8-core desktop, 16T, no E | 16 | 14 | 8 | 2 | 2 | 1 | 1 | 14 |
| 4-core laptop, 8T | 8 | 6 | 3 → 2 (fit) | 1 | 1 | 1 | 1 | 6 |
| 2-core/4-thread machine | 4 | 2 | 2 | 1 | 1 | 1 | 1 | 6, oversubscribed |

### Derived counts
| Count | Today | New |
|---|---|---|
| Tile pool default (STRM-020) | logical threads − `GEN_RESERVE` (18 on Rasierklinge), capped to `TERRAIN_CORES` = 12 in the rig | the terrain share; the rig's ceiling is the terrain share; `GEN_RESERVE` stays only as `--gen-reserve` |
| Horizon bakes | `workers / 2`, at most 8 | `terrain / 2`, at least 1 |
| Coupled reduction | `available_parallelism`, at most 8 | `B` (CON-14: the result does not depend on it) |
| Synthetic loads (BUDG-011) | fixed cores 1, 2, 2, 1 | each load's share |
| Reserve verdict (BUDG-012) | busy threads ≤ 12 | busy threads ≤ the terrain share |

### Where the code lives
- `planet_streaming::topology`: `CpuTopology { p, e }`, `detect()` (Windows, Linux, fallback) and `from_override(&str)`. The detection is thin; the parsing of the API structs and of sysfs is pure and tested at tier A on recorded bytes.
- `planet_streaming::shares`: `Shares::for_topology(&CpuTopology) -> Shares`, a pure function with the table as constants; tier A over synthetic topologies, every worked example plus the below-minimum machine.
- Tools that size their own pools (bake, prefill, stills, preview, terrain-score) keep `--threads` and `available_parallelism`: they are not background work beside a frame (BUDG-003 governs the frame-time shares). They are listed in the PR, not changed.

### Requirements that change with the code (PR 3), not in PR 1
- BUDG-011: the loads' cores become their shares, and the "Cores" scenario's ceiling of 12 becomes the terrain share.
- BUDG-012: the "An overrun" scenario's 14 becomes the terrain share plus 2.
- STRM-020:
  - the default pool becomes the terrain share, and "18 workers on 20 threads" becomes a worked example;
  - `--gen-workers N` still fixes the pool;
  - `--gen-reserve N` keeps today's meaning (logical threads − N) as an explicit override of the share.
- STRM-064: the code bakes on `terrain / 2`, at least 1. The requirement's Detail line ("at most 8 at once") is not modified here: the open change `shadow-clipmap-march` (crusader) removes STRM-064 (the horizon maps retire under ADR 0085), and a MODIFIED here would conflict with that REMOVED at archive. Until then the main spec's "8" is stale (tracked by the planner).
- TEST-039 and TEST-040: the calibration runs on a fixed reference thread count (`min(18, logical threads)`) independent of the pool. Otherwise every share-sized log (6 threads) loses the parallel limits silently. ADR 0065's limits stay valid; accepted (open point 3).
- Code beside `pipelined.rs` and the reserve: `litperf.rs` (`calibration_threads`), `streamperf.rs` (`calib_threads = gen.workers`), `horizonworld.rs` (`HORIZON_WORKERS_MAX`), `coupledworld.rs`, `app/src/lib.rs` (the `GEN_RESERVE` default and its test), `streaming/tests/dispatch_throughput.rs`.
- BUDG-005 (RAM in GB) and BUDG-008 (generic) do not change.

### Open points for the planner
1. Accepted 2026-11-02: the table, BUDG-013.
2. Accepted 2026-11-02: the terrain-only fit with oversubscription at the minimums; integer percent.
3. Accepted 2026-11-02: the calibration on a fixed `min(18, logical threads)`, independent of the pool (PR 3, TEST-039/040 delta; a measurement method, so ADR 0065's limits stay valid).
