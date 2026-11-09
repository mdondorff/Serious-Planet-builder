## ADDED Requirements
### Requirement: TEST-098 The preview's tests run on small planets in tier A
The tier-A tests of the level-1 preview (AUTH-007 to AUTH-010, GEN-157) SHALL run on planets of at most 32 cells a face edge, cite their requirement and need no store and no GPU.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #95), BUILD-003

#### Scenario: The size
- **WHEN** the preview's tests run
- **THEN** none builds a grid over 32 cells a face edge

### Requirement: TEST-099 The latency of level 1 is measured at the H0 grid
`cargo xtask perf` SHALL have a scene that measures, at the H0 grid (3,072 cells a face edge, 3.26 km), the seconds from an edit to its trunk rivers and watersheds for a height stamp and for a rainfall region, the process peak and the resident bytes, reported per edit kind. The result SHALL be the number the briefing's exit list needs.

Verify: D
Review: the perf window's report for the scene, `target/perf-window/<time>/` (the seconds and the peak per edit kind), read by the planner against the briefing's exit list
Status: planned
Source: Architect order 2026-11-09 (inbox #95), A3 11.1 (F6: 7 s)

#### Scenario: The scene
- **WHEN** the scene runs in a perf window
- **THEN** it reports the seconds and the peak per edit kind
