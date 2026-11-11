# compute-budgets Specification

## Purpose
The share table of the runtime compute budgets: main-thread frame time, GPU time, cores, RAM and VRAM per subsystem, with reserves for sky and weather, physics, game logic and networking; the one job system; the reserve test. Governed by CON-20, CON-21, ADR 0083.

## Requirements

### Requirement: BUDG-001 Main-thread frame time shares
At 1440p on the reference machine (CON-21) the main-thread work of a frame SHALL stay within 16.6 ms, split into shares in milliseconds per frame (median): terrain 10.0, sky and weather 1.5, physics 2.0, game logic 2.0, networking 0.5, headroom 0.6. Editor tools and their interface draw from terrain's share.

Verify: C
Status: planned
Source: ADR 0083 points 1 and 3, SPIKE-44, CON-21

#### Scenario: Terrain alone
- **WHEN** the standard flight paths run with no reserved subsystem
- **THEN** the median main-thread time of the frame is at most terrain's share

### Requirement: BUDG-002 GPU time shares
The GPU time of a frame at 1440p on the reference machine SHALL stay within 16.6 ms, split into shares in milliseconds per frame (median): terrain, water and shadows 5.0, sky and weather 3.0, physics 0.5, game rendering 4.0, post and present 1.0, headroom 3.1. REND-044 is a limit inside terrain's share.

Verify: C
Status: planned
Source: ADR 0083 points 1 and 3, SPIKE-44, REND-044

#### Scenario: Terrain alone
- **WHEN** the standard flight paths run with no reserved subsystem
- **THEN** the median GPU time of the frame is at most terrain's share

### Requirement: BUDG-003 Core shares
Work running beside a frame SHALL run in shares of the background capacity `B = max(P − 2, 1)`, where `P` is the machine's logical threads on performance cores, SMT siblings counted (BUDG-013); the 2 are the frame's main and render threads. Each share SHALL be `clamp(floor(pct × B / 100), min, max)` in integer arithmetic, with the percentage, minimum and maximum of the fraction table. If the shares sum to more than `B`, terrain SHALL give up threads down to its minimum; a machine whose minimums still exceed `B` runs at the minimums, oversubscribed. A share is a ceiling on threads busy at once, summed over the subsystem's pools; offline tools and post-passes size their own pools.

Verify: A, C
Status: active
Source: ADR 0092 (owner, 2026-11-02: a conservative terrain share; flight, not teleport, is the goal), ADR 0083 points 2 and 3, SPIKE-89, SPIKE-44; the fit rule and the provisional rows: opus-planner (2026-11-02)

Fraction table (normative, part of the statement):
| Share | pct | min | max |
|---|---|---|---|
| terrain | 60 | 2 | 10 |
| physics | 15 * | 1 | 4 |
| game logic | 15 * | 1 | 4 |
| sky and weather | 5 * | 1 | 2 |
| networking | 5 * | 1 | 1 |

\* provisional: placeholders until a game-engine integration measures them. Terrain's 60 is the owner's (ADR 0092).

Worked examples (not rules):
| Machine | P threads | B | terrain | physics | game logic | sky | networking | sum |
|---|---|---|---|---|---|---|---|---|
| Rasierklinge, 6 P-cores/12 threads + 8 E-cores | 12 | 10 | 6 | 1 | 1 | 1 | 1 | 10 |
| 8-core/16-thread desktop, no E-cores | 16 | 14 | 8 | 2 | 2 | 1 | 1 | 14 |
| 4-core/8-thread laptop | 8 | 6 | 3, fitted to 2 | 1 | 1 | 1 | 1 | 6 |
| 2-core/4-thread machine (below the minimum) | 4 | 2 | 2 | 1 | 1 | 1 | 1 | 6 > 2 |

#### Scenario: The shares of synthetic topologies
- **WHEN** the shares are computed for the four machines of the worked examples, and for the first one again with its 8 E-cores counted as 0
- **THEN** they are the rows of the worked examples, and the E-cores change nothing

#### Scenario: A flight
- **WHEN** a standard flight runs and the threads busy for terrain are sampled
- **THEN** at most the terrain share is busy at once

### Requirement: BUDG-004 One job system with priority classes
All background work SHALL run on one job system with a core budget per subsystem (BUDG-003) and the priority classes frame-critical, visible streaming and edits, prefetch, and idle (bakes), in that order. No subsystem SHALL create threads outside it, including the scoped workers of a job.

Verify: A
Status: active
Source: ADR 0083 point 2; opus-planner (2026-10-31, the order of the moves: tile pool, chunk pool, plan workers, horizon bakes)

#### Scenario: A thread outside
- **WHEN** the source of the crates is scanned for thread creation (BUDG-010)
- **THEN** every start is on the list with its count and reason, and each pool that moves onto the job system leaves the list in the change that moves it
- **AND** the requirement turns `active` when only the drivers and tools that stay are left on the list

### Requirement: BUDG-005 Memory shares
RAM SHALL stay within 8 GB (CON-21), in shares in GB: terrain 4.0, sky and weather 0.5, physics 0.75, game logic 1.0, networking 0.25, runtime and interface 0.5, headroom 1.0. VRAM SHALL stay within the tier cap (8 GiB mid, 14 GiB high), in shares in GiB mid / high: terrain 2.0 / 3.0 (STRM-006), sky and weather 1.0 / 2.0, physics 0.25 / 0.5, game content 2.5 / 5.0, frame targets 1.0 / 1.5, headroom the rest.

Verify: A
Status: planned
Source: ADR 0083 point 4, CON-21, STRM-006

#### Scenario: Terrain's pools
- **WHEN** the pools are configured for a tier
- **THEN** terrain's allocation is within its share of that tier

### Requirement: BUDG-006 Reserves and the reserve test
A share no subsystem has claimed SHALL stay unused. A subsystem SHALL claim a share by a change to this spec before it adds work. The perf harness SHALL run a synthetic load in each reserved share at its full size and SHALL report whether terrain stays within its own shares and the frame within 16.6 ms.

Verify: C
Status: planned
Source: ADR 0083 points 3 and 5

#### Scenario: Reserves at full load
- **WHEN** the synthetic loads of sky and weather, physics, game logic and networking run at their shares during a standard flight
- **THEN** terrain's main-thread, GPU and core use stay within its shares and the frame median stays within 16.6 ms

### Requirement: BUDG-007 An overrun names its share
A frame over its budget SHALL name the share that overran: the phase marks of the stall probe (VIEW-056) carry the name of the share they belong to, and the perf harness reports each share's median and p99 against this spec as a regression.

Verify: A
Status: planned
Source: ADR 0083 consequences, VIEW-056

#### Scenario: A report
- **WHEN** a perf run's shares are compared with the spec
- **THEN** each share is listed with its measured value, its limit and the verdict, and the largest overrun is named

### Requirement: BUDG-008 A share's ceiling and the class order
The job system SHALL run at most a share's ceiling of that share's jobs at once and SHALL start the jobs of the best class first (frame-critical, visible, prefetch, idle), in submission order within a class and share; a running job SHALL never be preempted. A share at its ceiling SHALL not hold back another share's jobs. A share MAY cap its idle-class jobs at once (STRM-060). A system MAY have workers of its own for frame-critical jobs (normal priority, ADR 0046) or for idle jobs (the horizon bakes): the other workers SHALL then never take that class.

Verify: A
Status: active
Source: ADR 0083 point 2, BUDG-003, BUDG-004

#### Scenario: A ceiling with free workers
- **WHEN** five jobs of a share with ceiling 2 are queued on six workers and none ends
- **THEN** two run, the other three wait, and the share's peak is 2 for the whole run

#### Scenario: An idle cap
- **WHEN** a share with ceiling 4 and an idle cap of 1 has three idle jobs and then three visible jobs queued
- **THEN** one idle job runs, the other two wait, and the three visible jobs run beside it

#### Scenario: Frame-critical workers
- **WHEN** both ordinary workers of a system with one frame-critical worker are held by visible jobs, and a frame-critical job and a visible job are queued
- **THEN** the frame-critical job runs at once on the frame worker, and the visible job waits for an ordinary worker

#### Scenario: Idle-only workers
- **WHEN** a system with one ordinary and one idle-only worker has two idle jobs queued, the first holding the idle worker
- **THEN** the second waits for the idle worker though the ordinary one is free, and a visible job runs on the ordinary worker at once

#### Scenario: A better class passes a worse one
- **WHEN** jobs of the classes idle, prefetch, visible, idle, frame-critical and visible are queued behind a running job of one worker
- **THEN** they start in the order frame-critical, visible, visible, prefetch, idle, idle

### Requirement: BUDG-009 Pool API and fan-out on the caller's share
A subsystem SHALL spawn background work on its `Share` and fan a computation out with `fan_out`, which returns the results in index order whatever the number of workers (CON-14). The calling thread SHALL work on the indices too; helpers SHALL stay inside the share's ceiling; a panic of the work SHALL be raised in the caller after every index has finished and SHALL not end a worker. Code below the job system SHALL fan out through an injected `planet_core::par::Par` (CON-04); the share's `Par` takes the free slots as permits, counted as running, runs the chunks on that many scoped threads beside the calling job, and gives them back.

Verify: A
Status: active
Source: ADR 0083 point 2, CON-14, opus-planner (2026-10-31: the plan job's fan-out counts against terrain's share at the highest priority within it)

#### Scenario: Order and progress
- **WHEN** 200 indices are fanned out over shares of 1 to 8 workers and ceilings of 1 to 8, and again with the only worker held by another job
- **THEN** the results are identical and in index order each time, and the held case completes on the calling thread

### Requirement: BUDG-010 The thread-creation scan
A tier-A test SHALL scan the production code of `crates/*/src` and `xtask/src` for calls that start a thread (`thread::spawn`, `thread::Builder::new`, `thread::scope`, `spawn_named`, `spawn_scoped`) and SHALL fail on a call that is not on the allow-list, on a count that differs from the list, and on a listed file that starts no thread any more. Each entry SHALL carry a reason: the job system itself, a driver or tool that stays, or the PR of the change `job-system` that moves it.

Verify: A
Status: active
Source: ADR 0083 point 2, BUDG-004

#### Scenario: A new thread
- **WHEN** a file starts a thread that the list does not name, or a moved pool leaves its entry in place
- **THEN** the test fails and names the file, the count and what to do

### Requirement: BUDG-011 Synthetic loads for the reserved shares
A synthetic load SHALL stand for one reserved share at its full size (BUDG-001 to BUDG-003): sky and weather 1.5 ms main thread, 3.0 ms GPU; physics 2.0 ms, 0.5 ms; game logic 2.0 ms, no GPU; networking 0.5 ms, no GPU; each with its share's cores on the machine (BUDG-003). The CPU spin SHALL be fixed integer work calibrated to its milliseconds against the clock within 5 %, never a sleep. The core part SHALL keep exactly its core count of idle-class spin jobs running on a share with that ceiling, never more. The GPU burn pass SHALL be a plan node of the plans of test scenes only; a production plan SHALL never carry it (CON-03).

Verify: A
Status: active
Source: ADR 0083 point 3, ADR 0092, BUDG-006, BUDG-008, planner inbox #6 (2026-10-31)

#### Scenario: Calibration
- **WHEN** a spin is calibrated on a fake clock that runs at half speed, and again at double speed
- **THEN** it runs half and double the iterations for the same milliseconds

#### Scenario: Cores
- **WHEN** the physics load with 2 cores (the 8-core/16-thread desktop's share) runs beside a terrain share of ceiling 4 on fake work
- **THEN** exactly two of its jobs run at once, and terrain still reaches 4

#### Scenario: The real clock
- **WHEN** `cargo xtask perf wallclock` runs on the reference machine
- **THEN** a 2 ms spin calibrated on the real clock takes its milliseconds within 5 % (the median of 41 spins, the best of three calibrations); this is a wall-clock check (BUILD-003), not tier A, which checks the calibration on fake clocks

### Requirement: BUDG-012 The reserve scene and its verdict
The perf harness SHALL run the standard flights (dive, look-1m, look-2km, teleport-1m, teleport-2km) with all synthetic loads on and without, in alternating runs, and SHALL report per flight terrain's main-thread, GPU and busy-thread medians and p99, the frame median and the share table of BUDG-007. The reserve verdict SHALL pass only if terrain stays inside its shares (BUDG-001 to BUDG-003) and the frame median inside 16.6 ms with the loads on. Under load the frame time SHALL need a quiet window (BUILD-031).

Verify: C
Status: active
Source: ADR 0083 point 3, ADR 0092, BUDG-006, BUDG-007, ADR 0087, planner inbox #6 (2026-10-31)

#### Scenario: A pass
- **WHEN** terrain stays within its shares and the frame median is 12 ms with all loads on
- **THEN** the report lists each share with measured value, limit and verdict, and the reserve verdict is a pass

#### Scenario: An overrun
- **WHEN** terrain's busy threads reach its share plus 2 (8 against 6) with the loads on
- **THEN** the verdict fails and names the terrain core share with its overrun

### Requirement: BUDG-013 CPU topology detection
The job system SHALL know the machine's logical threads on performance cores `P` and on efficiency cores `E`, with no new dependency: on Windows from `GetLogicalProcessorInformationEx` core records, summed over processor groups, each core's threads counted from its affinity mask, the highest efficiency class being performance (one class: all performance); on Linux from `/sys/devices/cpu_core/cpus` and `/sys/devices/cpu_atom/cpus`, else `cpu_capacity` (the highest value being performance); otherwise, or when a query fails, `available_parallelism`, all performance. Detected `P` SHALL be at most `available_parallelism`; `PLANET_CPU_TOPOLOGY=P,E` (P at least 1) SHALL replace detection, uncapped. `E` is reported only. Parsing SHALL be pure.

Verify: A
Status: active
Source: ADR 0092 point 4, SPIKE-89 (topology detection)

#### Scenario: Windows records
- **WHEN** recorded core records of 6 P-cores with 2 threads each and 8 E-cores with 1, in one and in two processor groups, and of 8 cores in a single class, are parsed
- **THEN** they give 12 and 8, 12 and 8, and 16 and 0

#### Scenario: Linux sysfs
- **WHEN** `cpu_core` lists `0-11` and `cpu_atom` lists `12-19`, and on another machine capacities 1024 on 4 threads and 512 on 4
- **THEN** they give 12 and 8, and 4 and 4

#### Scenario: The cap and the override
- **WHEN** 12 performance threads are detected in a process allowed 8, and the overrides `12,8`, `0,4` and `x` are read
- **THEN** P is 8; the first override gives 12 and 8, the others are refused with their text

### Requirement: BUDG-014 World build time shares
The world build (the whole offline run from a WorldDefinition to a store, ADR 0097) SHALL finish within 24 h on the reference machine, split into shares of that day: terrain 40 % (field bake, erosion bake passes 1 and 2, wet floor, coupled run, pre-fill), future content 40 % (fauna, instancing, cities), margin 20 % (values proposed; the owner confirms them). The world build SHALL report each stage's wall time and each share's sum against the share's hours; a stage has no limit of its own. A full world build SHALL start only after the owner approves its preview (a coarse-planet or region mode of every stage).

Verify: C
Status: planned
Source: ADR 0097

#### Scenario: A slow stage inside its share
- **WHEN** the field bake takes 17 min and the terrain stages together take 6 h
- **THEN** the report lists every stage, terrain's 6 h against its 9.6 h, and the world build passes

#### Scenario: A share over its hours
- **WHEN** the terrain stages together take 11 h
- **THEN** the report names terrain's share with 11 h against 9.6 h, and the world build fails

### Requirement: BUDG-015 World build threads
Every world-build stage (field bake, erosion bake, pre-fill) SHALL run by default on the background capacity B of BUDG-013, never on every logical thread, and SHALL log its thread count; an explicit thread count overrides it and is logged as an override. Results SHALL not depend on the thread count (CON-14).

Verify: A
Status: active
Source: ADR 0097 point 7 (owner, 2026-11-03); #1132

#### Scenario: Default and override
- **WHEN** a bake command runs without a thread count on a machine with P = 12
- **THEN** it runs on B = 10 workers and its log says "B = 10 (P = 12)"; with `--threads 4` the log says "override 4 (B = 10)"

### Requirement: BUDG-016 World build memory
The complete world build (rounds, H1, the lakes phase and the sediment pass, ADR 0115 point 7) SHALL stay below a hard limit of 12e9 bytes (11,444 MiB) of peak commit charge on the reference machine, read as the worse of two runs on one commit (gate G-SCALE). It SHALL also meet a working target of 11e9 bytes (10,490 MiB) by the A/B gate, which keeps headroom for later features (owner, 2026-11-11). Until the target holds, a change that adds memory inside the hydrology call SHALL bring an equal saving. A reading SHALL report the peak, the phase that reaches it, and both margins.

Verify: D
Status: planned
Source: ADR 0115 point 7 (owner, 2026-11-08, 2026-11-11); A3 §6.6

#### Scenario: Above the target, below the limit
- **WHEN** two runs peak at 11,376 and 11,387 MiB of commit charge in the hydrology phase
- **THEN** the reading reports 11,387 MiB, the hydrology phase, 57 MiB under the limit and 897 MiB over the target: G-SCALE passes the limit and the target is open

#### Scenario: Over the limit
- **WHEN** one of two runs peaks at 11,500 MiB
- **THEN** the reading names the phase and 56 MiB over the limit, and G-SCALE fails
