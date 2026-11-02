## ADDED Requirements
### Requirement: BUILD-034 The perf window runner
`cargo xtask perf window <plan> [--no-preflight]` SHALL run a plan of configurations and flights in one process: one release build, the preflight (TEST-040; a refusal stops the window), then the plan's rounds, configurations rotated each round, and the duty idle between runs (BUILD-035). Invalid or refused runs SHALL be retried up to the plan's count; a failed run stops the window. A configuration SHALL stop once settled against the reference: after a valid round more than 15 % worse on a metric, or after two valid rounds within 5 % on every metric (ADR 0087 point 7). Every attempt SHALL be kept with its start, end and idle time, and a table with medians, changes and minutes per valid run.

Verify: A
Status: active
Source: ADR 0087 point 7, ADR 0094, SPIKE-95 (PR 3 of its plan; SPIKE-90 windows 1 and 2: shell-loop runners, a copied log, a runner that outlived TaskStop), opus-planner (inbox #72)

#### Scenario: A plan
- **WHEN** a plan names 3 rounds, 1 retry, the reference `c`, three configurations with their environment and a `stream dive` flight, and plans with a zero round count, an unknown statement or metric, a missing log name or no flight are parsed
- **THEN** the first gives those values and the log `dive.json`, and each other is refused naming the fault; round k runs the configurations rotated by k

#### Scenario: The stop rule
- **WHEN** a configuration reads p99 +62 % against the reference after one valid round, another +3 % after one and after two, another +10 % after two
- **THEN** the first settles worse after round 1, the second stays open after one and settles within after two, and the third runs every round

#### Scenario: A window
- **WHEN** a fake window of 3 configurations and one 40 s flight runs, one configuration far worse and one within the margin
- **THEN** the runs are c, d, e, then e, c; the runner rests 20 s between runs, never before the first; every attempt has its start, end and idle; the table names who settled after which round and the minutes per valid run

#### Scenario: Retries and failures
- **WHEN** the first run is refused and its retry comes out invalid with retries 1, and another window's child fails
- **THEN** the refusal waits once, both attempts are kept with their reasons, nothing settles without a valid reference round, and the failure stops the window naming the run
