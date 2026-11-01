## ADDED Requirements

### Requirement: TEST-086 The world preview runs and reports the class shares
A tier-B check SHALL run the preview for seed 1 (at a reduced size) and report the class shares of land under 60 degrees latitude, the shares of land above 2 and 4 km, the highest land and the largest step of the untapered `k` between lattice points, each beside TEST-082's band. The report is not judged here (TEST-082 judges with the bake); the check SHALL fail if the shares do not sum to 1 or the report omits a measure.

Verify: B
Status: planned
Source: opus-planner (2026-11-01, inbox #37), GEN-103, TEST-082

#### Scenario: Seed 1
- **WHEN** the preview of seed 1 runs
- **THEN** the class shares of land sum to 1, badlands and the sand sea are 0, and the report names every measure with its band
