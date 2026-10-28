## ADDED Requirements

### Requirement: TEST-060 Milestone M8 acceptance
`cargo xtask accept M8` SHALL run the automated steps of `accept M5` plus the discrete-GPU tier C run, and SHALL judge M8's criteria 4 to 7 only from the logs of a valid window in `perf/logs/` or `--logs DIR`, never from a measurement of its own: `coupled-make.json` (at most 600 s from the 16384 bake), `shadow-cost.json` (every pose at most 1.5 ms by the compiled-out draws), `edit-latency.md` (no row over) and `envelope.json` (the steady speed at 300 m at least 200 m/s). A missing, smoke, other-scene or overridden log SHALL be MISSING, not a pass; any failed row fails the command, otherwise any MISSING row makes it incomplete.

Verify: A
Status: active
Source: opus-planner (2026-10-28), TEST-026, GEN-042, REND-044, TEST-042, TEST-047

#### Scenario: Logs against the budgets
- **WHEN** the four logs are within their budgets, then one is over, then they are missing, smoke or of another scene
- **THEN** the rows pass, the over row fails and names its number, and the others are MISSING; the outcome is pass, fail and incomplete, and a failure wins over a missing log
