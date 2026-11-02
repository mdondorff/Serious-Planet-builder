## ADDED Requirements
### Requirement: BUILD-035 The duty idle between perf runs
`cargo xtask perf --ab` (BUILD-033), `cargo xtask perf reserve` (BUDG-012) and `cargo xtask perf stream all` SHALL idle between two runs for half the previous run's wall time, at most 45 s, and print it (ADR 0094): never before the first run, and not between a refused try and its retry, which waits for the gate. `planet_perf::clock::duty_idle_s` SHALL be the one rule.

Verify: A
Status: active
Source: ADR 0094 (SPIKE-95: 45 s idle before each run held SPIKE-90 window 2 at 52-58 C with no cool-down wait), BUILD-033

#### Scenario: The duty idle
- **WHEN** the previous run took 40 s, 77 s or 160 s, or no run came before
- **THEN** the idle is 20 s, 38.5 s, 45 s and none

#### Scenario: Between runs only
- **WHEN** the A/B helper runs 5 rounds with fake runs of two builds and 3 refused tries
- **THEN** the runner rests 9 times: between the 10 runs, not before the first and not for the refused tries
