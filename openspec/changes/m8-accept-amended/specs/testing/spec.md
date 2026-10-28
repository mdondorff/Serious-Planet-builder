## MODIFIED Requirements

### Requirement: TEST-060 Milestone M8 acceptance
`cargo xtask accept M8` SHALL run the automated steps of `accept M5` plus the discrete-GPU tier C run and `gate --small` (TEST-063), and SHALL judge M8's criteria 4 to 7 only from the logs of a valid window in `perf/logs/` or `--logs DIR`, never from a measurement of its own: `coupled-make.json` (at most 600 s from the 16384 bake), `shadow-cost.json` (every pose at most 1.5 ms by the compiled-out draws), `edit-latency.md` (criterion 6: no row over in the M6 table headed `verdict` and none in the horizon table headed `verdict (10 s, horizon maps)`; the chunk table is not judged; a missing table is MISSING) and `envelope.json` (the steady speed at 300 m at least 200 m/s). A missing, smoke, other-scene or overridden log SHALL be MISSING, not a pass; any failed row fails the command, otherwise any MISSING row makes it incomplete. Criterion 3 (`cargo xtask gate` on the real store with the chunk stage off) SHALL be listed as a manual item.

Verify: A
Status: active
Source: opus-planner (2026-10-28), TEST-026, GEN-042, REND-044, TEST-042, TEST-047, TEST-061 to TEST-063, owner fallback 2026-10-28

#### Scenario: Logs against the budgets
- **WHEN** the four logs are within their budgets, then one is over, then they are missing, smoke or of another scene
- **THEN** the rows pass, the over row fails and names its number, and the others are MISSING; the outcome is pass, fail and incomplete, and a failure wins over a missing log

#### Scenario: The edit log's tables
- **WHEN** the chunk table has an over row, the horizon table is absent, then present with an over row
- **THEN** the chunk table is not judged, criterion 6 is MISSING without the horizon table and FAILS naming the horizon row with it
