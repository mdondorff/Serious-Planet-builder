## MODIFIED Requirements
### Requirement: BUILD-032 The report states loaded or quiet
Every row of a perf report, and every perf log (the one-off logs `global-bake`, `coupled`, `coupled-make`, `envelope` and `edit-latency` included), SHALL state `loaded` or `quiet`; a loaded one SHALL carry the load readings (CPU load, competing load, the top processes, GPU utilisation, CPU performance %, calibration). A loaded row with a time reading and a budget (the streaming and terrain rows, the lit passes, the bake and coupled times, the gated edit rows) SHALL carry its verdict of BUILD-031.

Verify: A
Status: active
Source: ADR 0087 points 1 and 2, planner (2026-11-01, perf-one-off-mode)

#### Scenario: Rows
- **WHEN** a loaded and a quiet session are reported
- **THEN** each row names its mode and readings, and the loaded rows their verdicts

#### Scenario: One-off logs
- **WHEN** a one-off log is written in a quiet and in a loaded session
- **THEN** both open with their mode, the loaded one also with its load record, and a log with a time budget closes with its verdicts in the loaded case only

#### Scenario: Lit rows
- **WHEN** a loaded session reports a `lit/` row
- **THEN** its GPU passes and its frame p99 carry the verdicts of BUILD-031 against their budgets
