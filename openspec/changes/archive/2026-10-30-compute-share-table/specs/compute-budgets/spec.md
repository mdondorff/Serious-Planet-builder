## ADDED Requirements

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
Status: planned
Source: ADR 0083 point 2

#### Scenario: A thread outside
- **WHEN** the source of the crates is scanned for thread creation
- **THEN** every spawn goes through the job system or is listed with a reason

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
