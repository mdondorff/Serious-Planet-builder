## ADDED Requirements

### Requirement: TEST-064 The gate compares with the chunk stage off
`planet editor --no-chunks` SHALL run the window without the chunk stage, and `cargo xtask gate --no-chunks` SHALL run the sites of TEST-062 that way and report the step between levels of the base field alone in place of the chunk change's.

Verify: C
Status: active
Source: opus-planner (2026-10-28), TEST-061, TEST-062

#### Scenario: Stage off
- **WHEN** the window runs with `--no-chunks` over a store with a coupled run
- **THEN** no chunk job is computed and the chunk stage's counters stay 0
