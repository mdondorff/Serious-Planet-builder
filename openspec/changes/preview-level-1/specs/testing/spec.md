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
`cargo xtask perf preview` SHALL run the `preview_latency` example in release: at the H0 grid (3,072 cells a face edge, 3.26 km) it SHALL time, after a warm run and over three runs, the level-1 preview of a height stamp, of a rainfall region and of both, and report per edit kind the median and maximum seconds, the phases (stamps, route, discharge, overlay), the process peak and the resident bytes, and once the base's build time. `--smoke` SHALL run at 96 cells a face edge and label the table as no measurement. The height-stamp median is the number the briefing's exit list needs; a timing read needs a quiet machine and an authorised perf window.

Verify: D
Review: the perf window's report for the scene, `target/perf-window/<time>/` (the seconds and the peak per edit kind), read by the planner against the briefing's exit list
Status: planned
Source: Architect order 2026-11-09 (inbox #95), A3 11.1 (F6: 7 s)

#### Scenario: The scene
- **WHEN** the scene runs in a perf window
- **THEN** it reports the seconds and the peak per edit kind
