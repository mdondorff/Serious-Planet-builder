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
Background work SHALL use at most 18 of the 20 hardware threads of the reference machine, in shares: terrain 12, sky and weather 1, physics 2, game logic 2, networking 1. The frame's main and render threads are outside the shares. A share is a ceiling on threads busy at once, summed over the subsystem's pools.

Verify: C
Status: planned
Source: ADR 0083 points 2 and 3, SPIKE-44

#### Scenario: A flight
- **WHEN** a standard flight runs and the threads busy for terrain are sampled
- **THEN** at most 12 are busy at once

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
A synthetic load SHALL stand for one reserved share at its full size (BUDG-001 to BUDG-003): sky and weather 1.5 ms main thread, 3.0 ms GPU, 1 core; physics 2.0 ms, 0.5 ms, 2 cores; game logic 2.0 ms, no GPU, 2 cores; networking 0.5 ms, no GPU, 1 core. The CPU spin SHALL be fixed integer work calibrated to its milliseconds against the clock within 5 %, never a sleep. The core part SHALL keep exactly its core count of idle-class spin jobs running on a share with that ceiling, never more. The GPU burn pass SHALL be a plan node of the plans of test scenes only; a production plan SHALL never carry it (CON-03).

Verify: A
Status: active
Source: ADR 0083 point 3, BUDG-006, BUDG-008, planner inbox #6 (2026-10-31)

#### Scenario: Calibration
- **WHEN** a spin is calibrated on a fake clock that runs at half speed, and again at double speed
- **THEN** it runs half and double the iterations for the same milliseconds

#### Scenario: Cores
- **WHEN** the physics load runs beside a terrain share of ceiling 12 on fake work
- **THEN** exactly two of its jobs run at once, and terrain still reaches 12

### Requirement: BUDG-012 The reserve scene and its verdict
The perf harness SHALL run the standard flights (dive, look-1m, look-2km, teleport-1m, teleport-2km) with all synthetic loads on and without, in alternating runs, and SHALL report per flight terrain's main-thread, GPU and busy-thread medians and p99, the frame median and the share table of BUDG-007. The reserve verdict SHALL pass only if terrain stays inside its shares (BUDG-001 to BUDG-003) and the frame median inside 16.6 ms with the loads on. Under load the frame time SHALL need a quiet window (BUILD-031).

Verify: C
Status: active
Source: ADR 0083 point 3, BUDG-006, BUDG-007, ADR 0087, planner inbox #6 (2026-10-31)

#### Scenario: A pass
- **WHEN** terrain stays within its shares and the frame median is 12 ms with all loads on
- **THEN** the report lists each share with measured value, limit and verdict, and the reserve verdict is a pass

#### Scenario: An overrun
- **WHEN** terrain's busy threads reach 14 with the loads on
- **THEN** the verdict fails and names the terrain core share with its overrun
