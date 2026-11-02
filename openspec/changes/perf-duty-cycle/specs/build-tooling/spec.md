## MODIFIED Requirements
### Requirement: BUILD-033 The A/B helper
`cargo xtask perf --ab <ref> <perf command>` SHALL run the command in the working tree and in a worktree of `<ref>` in alternating order for N rounds (default 5), SHALL retry a gate refusal up to a cap and report how often and why, SHALL keep every run's log, and SHALL print one table: per metric both medians, the change in %, the verdict against the 5 % margin and the rule that judges the metric under load (the metric table of the design: tile-ready and frame p99 by default). Between two runs (never before the first, and not between a refused try and its retry, which waits for the gate) it, `cargo xtask perf reserve` (BUDG-012) and `cargo xtask perf stream all` (between its scenes) SHALL keep a duty idle of half the previous run's wall time, at most 45 s, and print it (ADR 0094). It SHALL not write the store or the main tree's `perf/logs`.

Verify: A
Status: active
Source: ADR 0087, ADR 0094 (SPIKE-95: 45 s idle before each run held SPIKE-90 window 2 at 52-58 C with no cool-down wait), the job-system A/B of 2026-10-31

#### Scenario: Rounds
- **WHEN** the helper runs 5 rounds with fake runs of two builds
- **THEN** the order alternates, each log is kept, the runner rests 9 times (between the 10 runs, not for the refused tries), and the table gives both medians and the change

#### Scenario: The duty idle
- **WHEN** the previous run took 40 s, 77 s or 160 s, or no run came before
- **THEN** the idle is 20 s, 38.5 s, 45 s and none
