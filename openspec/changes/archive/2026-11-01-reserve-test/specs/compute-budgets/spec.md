## ADDED Requirements
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
