## ADDED Requirements

### Requirement: REND-044 The shadows and the material map cost at most 1.5 ms of the terrain pass
At 1440p at each pose of the pose stills (TEST-048), the horizon shadows (REND-038) and the material map (LOOK-008, LOOK-009) together SHALL cost at most 1.5 ms of terrain-pass GPU time, measured by `cargo xtask perf stream shadow-cost` as the median of 41 waited frames of the full draw minus the same draw without both. A pose within 20 % of the budget SHALL NOT be called met before shader variants without the branch are built.

Verify: A, C
Status: active
Source: owner (2026-10-23, D4), opus-planner (2026-10-27), M8 exit criterion 5

#### Scenario: The verdict
- **WHEN** a pose's full draw takes 5 ms and the draw without shadows and material 3 ms
- **THEN** the pose fails the budget, and a pose at 1.3 ms is flagged as within 20 % of it
