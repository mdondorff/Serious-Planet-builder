## MODIFIED Requirements

### Requirement: GEN-014 Global bake time
A global-coarse bake at a cell size of 1 km or finer (16384 cells per face edge at Earth's radius) SHALL take no more than ten minutes on the reference machine, measured by the performance harness through the architect and never in a test (CON-20), and SHALL stay within the resident memory budget (RAM no more than 8 GB, CON-21), which the tiled bake supports by never holding more than a batch of blocks (tested at tier A under GEN-013). The `global-bake` perf scene (`perf_global_bake`) cites this requirement for spec-lint (TEST-001). The harness entry is `cargo xtask perf global-bake` (ADR 0020 readiness gate, default grid 16384 cells per face edge, log `perf/logs/global-bake.json` with wall time, the most blocks alive and the bytes they take, and the store size); the architect session runs it.

Verify: C
Status: active
Source: architecture report §14 (M3 exit criteria), CON-20, CON-21

#### Scenario: Bake timing
- **WHEN** the global bake of a 1 km grid runs on the reference machine in a perf window
- **THEN** it finishes within ten minutes and its resident memory stays within the budget
