Opened: 2026-11-01
Question: Which hardware constants of the reference machine are fixed in the specs, how does the job system size itself today, and what relative, topology-aware form of the compute shares would run well on other machines?
Why it matters: owner, 2026-11-01: the software should run well on other systems too; BUDG-003 fixes Rasierklinge's 20 threads (terrain 12, physics 2, ...), and the terrain share (10 against 12) is open with the owner.
Time box: 2 h (Sonnet, analysis only)
Success criteria: an inventory with requirement IDs, values and machine assumptions; what the job system detects; a relative proposal evaluated on three machines; the requirements that would change and the cost in PRs.
Fallback if it fails: the inventory alone.
Code location: none (analysis; a 20-line evaluation script is quoted in the Proposal).
Closed: 2026-11-01

## Headline
- **Every thread share is an absolute count of the reference machine.** BUDG-003 states "at most 18 of the 20 hardware threads ... terrain 12, sky and weather 1, physics 2, game logic 2, networking 1"; the code carries `TERRAIN_CORES = 12.0` (`crates/perf/src/reserve.rs:122`), `GEN_RESERVE = 2` (`crates/streaming/src/lib.rs:741`) and caps of 8 (horizon bakes, coupled run). ADR 0083 point 3 asked for "a stated fraction ... of the cores"; the spec turned it into 12.
- **The job system detects only the logical CPU count** (`available_parallelism`, fallback 4): no P/E classes, no SMT siblings, no VRAM size (the VRAM tier is hard-coded to Mid).
- **Proposal:** shares as fractions of a weighted capacity `C = P + 0.65 E` (logical P-core threads plus E-core threads at the measured 0.63-0.71 of a P thread), less 2 threads for the main and render threads, each with a minimum and maximum, plus a fit rule. Detection needs no new crate: one feature of the existing `windows` dependency on Windows, sysfs on Linux, `available_parallelism` as the fallback.
- **Evaluated:** Rasierklinge terrain 9 (total 15), an 8-core desktop terrain 8 (14), a 4-core/8-thread laptop terrain 2 (6). Terrain 10 on Rasierklinge needs a terrain fraction of 0.66 and over-commit of one thread, or an E weight of 0.75: that is the owner's 10-or-12 question restated as one number.

## 1. Inventory: hardware constants
Kinds: **Abs** fixed absolute; **Rel** derived from a detected count; **Ref** a budget measured on or set for the reference machine (left as budgets, not generalised).

### Thread counts and shares
| ID | Value | Assumes | Kind |
|---|---|---|---|
| BUDG-003 (`compute-budgets/spec.md:31`) | at most 18 of 20 threads; terrain 12, sky 1, physics 2, game 2, net 1 | 20 logical CPUs | Abs |
| BUDG-003 scenario (:39) | at most 12 terrain threads busy | 20 logical CPUs | Abs |
| BUDG-011 (:136) | synthetic loads 1 / 2 / 2 / 1 cores | 20 logical CPUs | Abs |
| BUDG-011 scenario (:147) | terrain ceiling 12 | 20 logical CPUs | Abs |
| BUDG-012 scenario (:166) | 14 busy terrain threads fail | 20 logical CPUs | Abs |
| STRM-020 (`streaming-cache/spec.md:586`) | workers = logical count - 2 (18 on 20) | logical count | Rel |
| STRM-064 (:1152) | horizon bakes at most 8, "half the generation workers" | 20 logical CPUs | Rel with Abs cap |
| TEST-039 / TEST-040 (`testing/spec.md:902`, :942) | parallel calibration limit 112,000 us "for 18 threads" | an 18-thread pool | Abs, reference-only |
| BUILD (`build-tooling/spec.md:106-107`) | gpu tier at most 8, heavy window 2, "16-thread laptop" | a 16-thread machine (an error: 20) | Abs (tooling) |
| ADR 0083 Context / Consequences | tile pool 18 (cores - 2), chunk pool 9; "16 threads of the 20" | 20 logical CPUs | historical (0086 removed the chunks) |
| ADR 0086 | none | - | - |

### Memory
| ID | Value | Assumes | Kind |
|---|---|---|---|
| BUDG-005 (:54) | RAM 8 GB: terrain 4.0, sky 0.5, physics 0.75, game 1.0, net 0.25, runtime 0.5, headroom 1.0 | 16 GB RAM (CON-21) | Abs |
| STRM-033 (:933) | terrain RAM 3 GiB of 8 | 16 GB RAM | Abs; **contradicts BUDG-005's 4.0** |
| BUDG-005 / STRM-005 (:325) | VRAM tiers 8 GiB mid, 14 GiB high; terrain 2 / 3 GiB | the GPU's memory | Abs per tier; the code always picks Mid |
| TEST-019 / TEST-020 | RAM 8 GiB; upload 1 MiB / 2 ms | 16 GB RAM | Abs |

### Budgets on the reference machine (kept as budgets)
BUDG-001 (main-thread 16.6 ms and its split), BUDG-002 (GPU 16.6 ms split), TEST-018 (GPU median 8 ms, p99 20 ms), TEST-039 single-thread limits 52,000 / 56,000 us, TEST-012 gate limits (load 15 %, 70 / 60 C), the global bake and coupled run under 10 min, `CARGO_BUILD_JOBS=10`, CON-21 (constitution).

## 2. How the job system sizes itself today
| Where | What | Kind |
|---|---|---|
| `crates/streaming/src/lib.rs:741`, :770 | `GEN_RESERVE = 2`; workers = `available_parallelism()` (fallback 4) - 2 | Rel, fallback Abs |
| `crates/app/src/pipelined.rs` `terrain_sizes(workers, horizon, cap)` | tile pool `min(workers, cap - 1)`, share ceiling `min(cap, tile + 1 + horizon)`, cap = `TERRAIN_CORES` = 12 | cap Abs, rest Rel |
| `crates/app/src/horizonworld.rs:41-47` | horizon workers `(workers / 2).clamp(1, 8)` | Rel with Abs cap |
| `crates/app/src/coupledworld.rs:65` | `available_parallelism().min(8)` | Rel with Abs cap |
| `crates/streaming/src/jobs.rs` `share(name, ceiling)` | ceilings passed in; no table of shares in code | - |
| `crates/perf/src/calibration.rs:22-30` | `REFERENCE_THREADS = 18` | Abs |
| `crates/streaming/src/budget.rs` | VRAM Mid 8 GiB / High 14 GiB; `Tier::Mid` hard-coded at the pool (`render/src/terrain.rs:1316`, `app/src/skeleton.rs:1047`) | Abs |
- Detected: the logical count only. No physical cores, P/E classes, SMT siblings, NUMA, affinity or VRAM size. Platform calls exist only for thread priority (`streaming/src/priority.rs`: `SetThreadPriority`; `libc` on Linux).
- Dependencies at hand: `windows = 0.62` (features `Win32_System_Threading` only) and `libc = 0.2`, both in `crates/streaming`. No `num_cpus`, `sysinfo`, `core_affinity`.
- Rasierklinge: i7-12800H, 14 cores / 20 logical (`Win32_Processor`): 6 P-cores with SMT (logical 0-11) and 8 E-cores (12-19, SPIKE-58).

## 3. Proposal: relative, topology-aware shares
### Capacity
- `P` = logical threads on performance cores (SMT siblings counted), `E` = logical threads on efficiency cores.
- `C = P + w_E x E`, `w_E = 0.65`: the explorer's scaling sweep (2026-11-01, via the planner) measured terrain 40-60 % slower per worker on E-cores than on P-core threads, a throughput ratio of 0.63-0.71.
- Background capacity `B = C - R`, `R = 2` (the main and render threads, outside the shares as today).

### Shares (ceilings on threads busy at once, as BUDG-003 defines a share)
`share = clamp(round(f x B), min, max)`; if the shares sum to more than `B`, the share with the most room above its minimum gives up one thread at a time until the sum fits or every share is at its minimum. A machine whose minimums exceed `B` runs at the minimums (a share is a ceiling, not a reservation) and is below the supported minimum.
| Share | f | min | max |
|---|---|---|---|
| terrain | 0.60 | 2 | 10 |
| physics | 0.12 | 1 | 4 |
| game logic | 0.12 | 1 | 4 |
| sky and weather | 0.06 | 1 | 2 |
| networking | 0.06 | 1 | 1 |
- The terrain maximum 10 comes from the knee of the scaling sweep (tile-ready saturates at 8-10 P-pinned workers; status.md, 2026-11-01).
- Derived caps follow the terrain share: horizon bakes `terrain / 2` (today `workers / 2` capped at 8), the coupled run `B` (today 8).
- RAM and VRAM: unchanged fractions of CON-21's 8 GB and of the detected VRAM tier (tier detection is a separate item: DXGI `DedicatedVideoMemory` on Windows, the Vulkan memory heaps on Linux).

### Evaluation (the script: the formula above in 20 lines of Python, run 2026-11-01)
| Machine | P | E | C | B | terrain | physics | game | sky | net | sum |
|---|---|---|---|---|---|---|---|---|---|---|
| Rasierklinge, i7-12800H 6P(12T)+8E | 12 | 8 | 17.2 | 15.2 | 9 | 2 | 2 | 1 | 1 | 15 |
| desktop, 8 cores / 16 threads, no E | 16 | 0 | 16.0 | 14.0 | 8 | 2 | 2 | 1 | 1 | 14 |
| laptop, 4 cores / 8 threads | 8 | 0 | 8.0 | 6.0 | 2 (fit from 4) | 1 | 1 | 1 | 1 | 6 |
| (below minimum) 4 cores / 4 threads | 4 | 0 | 4.0 | 2.0 | 2 | 1 | 1 | 1 | 1 | 6 > 2 |
| today, any machine | - | - | - | - | 12 | 2 | 2 | 1 | 1 | 18 |
- Rasierklinge with terrain f = 0.66 rounds to 10 but the fit takes it back to 9 (sum 16 > 15.2). Terrain 10 needs either an over-commit allowance of one thread or `w_E >= 0.75`. That is the owner's 10-or-12 question as one parameter.
- On the 4-core laptop terrain falls to its minimum because the four other shares sit at theirs; if terrain should win there, the fit order becomes "the share with the lowest priority first" (BUDG-004's class order) instead of "most room first".

### Topology detection (no new crate)
| Platform | Source | Needs |
|---|---|---|
| Windows 10 1903+ | `GetLogicalProcessorInformationEx(RelationProcessorCore)`: per core `EfficiencyClass` (higher = faster), `Flags & LTP_PC_SMT`, group affinity masks | the feature `Win32_System_SystemInformation` on the existing `windows = 0.62` |
| Linux (Intel hybrid, kernel 5.13+) | `/sys/devices/cpu_core/cpus` and `/sys/devices/cpu_atom/cpus` | `std::fs` |
| Linux (ARM big.LITTLE) | `/sys/devices/system/cpu/cpu*/cpu_capacity` (1024 = fastest) | `std::fs` |
| any other | `available_parallelism`, all counted as P | none |
- An override for tests and odd machines: `PLANET_CPU_TOPOLOGY=P,E` (or a flag), so the tier-A tests evaluate fixed topologies and CI runners are reproducible.
- Results do not depend on the shares: CON-14 already makes every generated value independent of the thread count; the shares change only scheduling.
- Placement stays the OS scheduler's (Thread Director on Windows): no affinity. An optional later item: EcoQoS (`SetThreadInformation`, `ThreadPowerThrottling`) for background-class threads, to steer them to E-cores; that needs its own spike.

## 4. Requirements that would change, and cost
| Requirement | Change |
|---|---|
| BUDG-003 + scenario | the formula, the f/min/max table, the fit rule; a scenario per evaluated machine (synthetic topologies) |
| BUDG-011 + scenario | synthetic loads sized from the shares; "beside a terrain share of the formula's value" |
| BUDG-012 scenario | "busy threads above the terrain share" instead of 14 |
| STRM-020 | workers = `B` rounded down, `R` named; the 1 to 64 thread scenario becomes topologies |
| STRM-064 | horizon bakes `terrain / 2`, no absolute 8 |
| TEST-039 / TEST-040 | stated as reference-machine-only calibration (other machines: no verdict), `REFERENCE_THREADS` tied to Rasierklinge by name |
| new ADR (amends ADR 0083 point 3) | the relative form, `w_E`, `R`, detection without new crates |
| errata, not this decision | BUDG-005 terrain RAM 4.0 GB against STRM-033 3 GiB; `build-tooling` "16-thread laptop" (20) |

| PR | Content | Size |
|---|---|---|
| 1 | ADR (proposed) and the BUDG / STRM / TEST deltas (docs) | small |
| 2 | `core` or `streaming`: `cpu_topology()` (Windows API, Linux sysfs, fallback, override) with tier-A tests on synthetic topologies | small-medium |
| 3 | `shares_for(topology)`, a pure function in a GPU-free crate, replacing `TERRAIN_CORES`, `GEN_RESERVE`, the horizon and coupled caps; tier-A tests of the evaluated table | medium |
| 4 | perf: the reserve test and calibration read the formula; reference-only labels | small |
| later | VRAM tier detection (1 PR, Windows and Linux paths); EcoQoS for background threads (spike first); the two errata (docs) | - |
- Estimate: 4 PRs, about 1.5 to 2 lane-days, plus one perf window on Rasierklinge to confirm that terrain 9 (or 10) is no worse than 12 (the explorer's unpinned 8/10 runs are the first evidence).

## Limits
- `w_E` comes from one sweep on one machine (terrain jobs, via the planner); other workloads may weigh E-cores differently.
- The desktop and the 4-core laptop are evaluated on paper; no run on such a machine.
- SMT threads are counted as full P threads (as today); a P-core with both siblings busy delivers less than two single-thread P units, which `w_E` (measured against SMT-loaded P threads) already absorbs on Rasierklinge only.

## Visuals
Nothing to look at: tables only.
