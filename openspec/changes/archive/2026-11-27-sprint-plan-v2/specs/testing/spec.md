## ADDED Requirements

### Requirement: TEST-161 The plan command's sprint checks are tested on fixture files
Tier-A tests SHALL check BUILD-074 and BUILD-053's draft state on fixture files in `xtask/src/fixtures/`: a sprint file that passes and the mutations of the scenarios below, `--pr`, the selection of milestones, and an active sprint read from a temporary repository with its record. The old milestone form keeps BUILD-053's tests.

Verify: A
Status: active
Source: Lead 2026-11-27, BUILD-074

#### Scenario: A mutation per failure
- **WHEN** the passing fixture is changed in one cell at a time: dates and the 7-day cap, each header line, the exit cap, cells, modes, gate classes (and `-`), states, a missing record for a promoted and a parked row, a failed, empty or numerical-exception verdict, the decision cap, a missing Default or By, an unsafe default taken
- **THEN** each change gives an error that names its cell, and the unchanged fixture gives none

#### Scenario: Notices, paths and the PR check
- **WHEN** the tests run the A1 notices, whole-word safe defaults, `overdue`, the docs-only path set, `--pr`'s firewall, change-branch and overdue cases, its arguments (`--branch`, `--body-file`'s `Exit:` lines, `--files`) and its no-op without an active sprint file
- **THEN** each gives the result BUILD-074 states

#### Scenario: The selection
- **WHEN** the tests select one active file, one active and one draft (also an old-form draft), a draft alone (with and without a queue row), two drafts and two actives
- **THEN** each passes or fails as BUILD-053 states
