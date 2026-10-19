## ADDED Requirements

### Requirement: TEST-040 Perf preflight
A preflight SHALL run before a perf window (`cargo xtask perf preflight [--out DIR] [--smoke] [--machine-ready]`, the scene `preflight` of the binary). It SHALL sample the machine's load for about 70 s, run the readiness gate (ADR 0020, TEST-012) at the start, and measure the CPU calibration (STRM-022) three times about 20 s apart, on the generation pool's thread count. It SHALL report the AC state, the active power plan, the top 5 competing processes by mean CPU share over the sampled period, and each calibration reading with its limit. It SHALL refuse, with a non-zero exit and one line per cause naming the reading and the limit, when any gate condition fails, when a calibration cannot be measured, or when the worst of the three readings lies over a limit of ADR 0065 for a reading taken before a case (TEST-039: single over 52,000 us, or parallel median over 112,000 us at the reference thread count); it SHALL otherwise print `PREFLIGHT OK`. The decision SHALL be a pure function over the measured facts, used by the scene and tested at tier A. With `--out DIR` the report SHALL be written to `DIR/preflight.md`. `--smoke` SHALL run one round on any adapter, print the report, and never refuse; `--machine-ready` SHALL print every failure as an owner override and not refuse. A preflight SHALL NOT be a measurement and SHALL NOT change any perf log.

Verify: A, C
Status: active
Source: M5 process review improvement 3, ADR 0020, ADR 0065

#### Scenario: Ready machine
- **WHEN** the gate has no failure and all three calibrations are within the limits
- **THEN** the decision passes and the report lists the AC state, the power plan, the top 5 processes and the readings

#### Scenario: Refusals
- **WHEN** the gate fails (battery, a build process, high load), or a calibration is missing, or the worst single reading is over 52,000 us, or the worst parallel median at the reference thread count is over 112,000 us
- **THEN** the decision refuses and names each cause with its measured value and limit

#### Scenario: Worst of three
- **WHEN** two calibrations are within the limits and the third is over
- **THEN** the decision refuses, naming the third reading

#### Scenario: Smoke
- **WHEN** the scene runs with `--smoke` on the software adapter
- **THEN** it prints the report and exits successfully whatever the readings
