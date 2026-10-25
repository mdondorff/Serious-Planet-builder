## ADDED Requirements

### Requirement: GEN-068 The coarse run's tributaries are routed like stems
The A-net chunk stage SHALL route every coarse link whose drainage area exceeds `river_area_m2` (default 100 km2; the stage's definition sets 5e7 m2) by the node slide, least-cost segment, bed rule and valley law of the main stems (GEN-059, GEN-066), so topology, crossings and areas come from the global run and chunks agree on them by construction (GEN-067). `river_area_m2` is part of the definition hash and `ANET_VERSION` is 7.

Verify: A
Status: active
Source: ADR 0081 point 3, planner decision 2026-10-25

#### Scenario: A tributary under the threshold
- **WHEN** a chunk of a river with a tributary of 3e8 m2 is built with thresholds of 1e8 and 5e8 m2
- **THEN** the tributary has a corridor with the first and none with the second
