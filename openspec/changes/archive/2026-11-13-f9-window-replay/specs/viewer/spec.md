## ADDED Requirements
### Requirement: VIEW-071 An F9 bundle records the commit and the site of the run
An F9 repro bundle written by a window started with `--commit DIR` or `--site LAT,LON` SHALL record them in the bundle's head as `open_commit PATH` (absolute, escaped as a world line) and `open_site LAT,LON` (degrees), so that a replay opens the same world at the same place. A run without them records neither line. The bundle format stays version 2; a bundle without the lines reads as before, and the lines survive a write and a read. The lines are read by `cargo xtask repro --window` (BUILD-054) and decide nothing in the plan replay of VIEW-026.

Verify: A
Status: active
Source: Lead 2026-11-13 (P2 E3), crusader's #124 diagnosis (a bundle without --commit cannot be replayed faithfully), VIEW-024, VIEW-069

#### Scenario: A bundle of a commit flight
- **WHEN** a bundle records `open_commit` and `open_site` and is written and read back
- **THEN** both values are equal after the read, a path with a space and a backslash included, and a bundle of a run without them has neither line
