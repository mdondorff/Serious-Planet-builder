## ADDED Requirements

### Requirement: TEST-161 The plan command's sprint checks are tested on fixture files
Tier-A tests SHALL check BUILD-074 and BUILD-053's draft state on fixture files (`xtask/src/fixtures/`): a sprint file that passes, and one mutation per failure of BUILD-074 (dates and the 7-day cap, header lines, the exit cap and cells, modes, gate classes, states, a missing record, a failed, empty or numerical-exception verdict, the decision cap, a missing Default or By, an unsafe default taken); the A1 notices; `overdue`; `--pr`'s firewall, change-branch and overdue cases; the selection of one active file, one active and one draft, a draft alone (with and without a queue row), two drafts and two actives; and an active sprint read from a temporary repository with its record. The old milestone form keeps BUILD-053's tests.

Verify: A
Status: active
Source: Lead 2026-11-27, BUILD-074

#### Scenario: One mutation per failure
- **WHEN** the passing fixture is changed in one cell at a time
- **THEN** each change gives the one error that names its cell, and the unchanged fixture gives none
