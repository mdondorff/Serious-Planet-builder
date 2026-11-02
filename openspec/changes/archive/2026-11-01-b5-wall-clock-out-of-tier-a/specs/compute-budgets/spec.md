## MODIFIED Requirements

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

#### Scenario: The real clock
- **WHEN** `cargo xtask perf wallclock` runs on the reference machine
- **THEN** a 2 ms spin calibrated on the real clock takes its milliseconds within 5 % (the median of 41 spins, the best of three calibrations); this is a wall-clock check (BUILD-003), not tier A, which checks the calibration on fake clocks
