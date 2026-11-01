## ADDED Requirements
### Requirement: BUILD-030 The gate records the load in loaded mode
`cargo xtask perf` with `--loaded` SHALL run the readiness gate (ADR 0020) with its hard conditions (AC power, power scheme, reference adapter, GPU temperature, GPU throttling) and SHALL record the load conditions (build and test processes, CPU load, GPU utilisation, the CPU calibration of ADR 0065) as readings instead of refusing on them. `--machine-ready` SHALL stay the owner's override, and `--loaded` with `--machine-ready` SHALL be refused. Quiet mode SHALL behave as before.

Verify: A
Status: planned
Source: ADR 0087 consequences, ADR 0020, owner 2026-10-31

#### Scenario: A loaded machine
- **WHEN** the gate runs in loaded mode with a build process, 40 % CPU load and a calibration over its limit
- **THEN** it passes, and the readings are in the load record

#### Scenario: A hard condition
- **WHEN** the gate runs in loaded mode on battery, or with the GPU over its temperature limit
- **THEN** it refuses naming the reading and the limit

### Requirement: BUILD-031 The loaded verdict rules
A pure function SHALL give a time verdict measured under load per ADR 0087: a reading under budget by at least 5 % is a loaded pass; over budget is inconclusive, never a fail; within the margin goes to a window. The frame time with streaming, a cross-session regression comparison and a milestone acceptance run SHALL need a quiet window. A CPU-timed scene SHALL count only when pinned, or with a 30 % margin. A GPU timing SHALL carry the +18 % note.

Verify: A
Status: planned
Source: ADR 0087 points 1 to 5

#### Scenario: Margins
- **WHEN** readings of 94 %, 97 % and 110 % of a GPU budget are judged
- **THEN** they are a loaded pass with the note, a window case and inconclusive

#### Scenario: CPU and exclusive kinds
- **WHEN** an unpinned CPU reading at 80 % of its budget and a frame p99 with streaming are judged
- **THEN** the first needs 70 % or less to be a loaded pass, and the second needs a quiet window

### Requirement: BUILD-032 The report states loaded or quiet
Every row of a perf report SHALL state `loaded` or `quiet` and carry the load readings (CPU load, competing load, the top processes, GPU utilisation, CPU performance %, calibration), and a loaded row SHALL carry its verdict of BUILD-031.

Verify: A
Status: planned
Source: ADR 0087 points 1 and 2

#### Scenario: Rows
- **WHEN** a loaded and a quiet session are reported
- **THEN** each row names its mode and readings, and the loaded rows their verdicts

### Requirement: BUILD-033 The A/B helper
`cargo xtask perf --ab <ref> <perf command>` SHALL run the command in the working tree and in a worktree of `<ref>` in alternating order for N rounds (default 5), SHALL retry a gate refusal up to a cap and report how often and why, SHALL keep every run's log, and SHALL print one table: per metric both medians, the change in %, the verdict against the 5 % margin and the rule that judges the metric under load (the metric table of the design: tile-ready and frame p99 by default). It SHALL not write the store or the main tree's `perf/logs`.

Verify: A
Status: planned
Source: ADR 0087, the job-system A/B of 2026-10-31

#### Scenario: Rounds
- **WHEN** the helper runs 5 rounds with fake runs of two builds
- **THEN** the order alternates, each log is kept, and the table gives both medians and the change
