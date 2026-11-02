## MODIFIED Requirements
### Requirement: BUDG-003 Core shares
Background work SHALL run in shares of the background capacity `B = max(P − 2, 1)`, where `P` is the machine's logical threads on performance cores, SMT siblings counted (BUDG-013); the 2 are the frame's main and render threads. Each share SHALL be `clamp(floor(pct × B / 100), min, max)` in integer arithmetic, with the percentage, minimum and maximum of the fraction table. If the shares sum to more than `B`, terrain SHALL give up threads down to its minimum; a machine whose minimums still exceed `B` runs at the minimums, oversubscribed. A share is a ceiling on threads busy at once, summed over the subsystem's pools.

Verify: A, C
Status: planned
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

## ADDED Requirements
### Requirement: BUDG-013 CPU topology detection
The job system SHALL know the machine's logical threads on performance cores `P` and on efficiency cores `E`, with no new dependency: on Windows from `GetLogicalProcessorInformationEx` core records, summed over processor groups, each core's threads counted from its affinity mask, the highest efficiency class being performance (one class: all performance); on Linux from `/sys/devices/cpu_core/cpus` and `/sys/devices/cpu_atom/cpus`, else `cpu_capacity` (the highest value being performance); otherwise, or when a query fails, `available_parallelism`, all performance. `P` SHALL be at most `available_parallelism`. `PLANET_CPU_TOPOLOGY=P,E` (P at least 1) SHALL override detection. `E` is reported only. Parsing SHALL be pure.

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
