## MODIFIED Requirements

### Requirement: TEST-064 The gate runs without the chunk stage by default
`cargo xtask gate` SHALL run the sites of TEST-062 without the chunk stage and report the step between levels of the base field alone; `cargo xtask gate --chunks` SHALL run them with the stage and the chunk change's step. `gate --small` SHALL run the small world both ways.

Verify: C
Status: active
Source: opus-planner (2026-10-28), owner fallback 2026-10-28, TEST-061, TEST-062

#### Scenario: Stage off
- **WHEN** the window runs over a store with a coupled run without `--chunks`
- **THEN** no chunk job is computed and the gate line reports `chunk jobs: None`
