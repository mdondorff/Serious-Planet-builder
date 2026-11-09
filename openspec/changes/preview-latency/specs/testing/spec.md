## MODIFIED Requirements
### Requirement: TEST-099 The latency of level 1 is measured at the H0 grid
`cargo xtask perf preview` SHALL run `preview_latency` under the readiness gate: at the H0 grid (3,072 cells a face edge, 3.26 km) it SHALL time, after a warm run and over three runs, the level-1 preview of a height stamp, of a rainfall region and of both, and report per edit kind the median and maximum seconds, the phases (stamps, route, discharge, overlay), the process peak and the resident bytes, and once the base's build time. `--smoke` SHALL run at 96 cells a face edge and label the table as no measurement. The height-stamp median is the number the briefing's exit list needs.

Verify: D
Review: the perf window's report for the scene, `target/perf-window/<time>/` (the seconds and the peak per edit kind), read by the planner against the briefing's exit list
Status: planned
Source: Architect order 2026-11-09 (inbox #95), A3 11.1 (F6: 7 s)

#### Scenario: The scene
- **WHEN** the scene runs in a perf window, and in smoke mode
- **THEN** the first reports the seconds, the phases, the peak and the resident bytes per edit kind, and the second prints the same table labelled as no measurement
