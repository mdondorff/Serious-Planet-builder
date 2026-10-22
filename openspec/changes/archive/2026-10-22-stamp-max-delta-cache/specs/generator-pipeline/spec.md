## ADDED Requirements

### Requirement: GEN-032 The stage's field-wide bound is read once per item revision
A stamp stage's `max_abs_height_m` (the base's bound plus the sum of the layer's `max_delta_m`) SHALL be computed from the items once for each item revision (`ItemReader::revision`) and read from the stage afterwards. A plan asks for it per tile and per level; SPIKE-29 found the walk over every item was 99.8 % of a plan job at 1,000 items. The value SHALL equal the base's bound plus `ItemReader::max_delta_sum` at the call after items are added, removed, changed or moved between layers. The cache is per stage instance and safe for the plan job's threads. No output changes.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 1), GEN-026, STRM-027

#### Scenario: The cache equals the walk
- **WHEN** items of the layer are added, one of another layer is added, one's `max_delta_m` is changed, items are moved into and out of the layer and one is removed, the stage's bound being asked after each
- **THEN** every answer, the first and a repeat from the cache, equals the base's bound plus the uncached sum over the items at that moment
