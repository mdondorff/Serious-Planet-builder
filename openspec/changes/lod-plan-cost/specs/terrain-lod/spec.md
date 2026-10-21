## ADDED Requirements

### Requirement: LOD-012 Selection over nested bounds
A height field MAY say that its bounds nest (`HeightField::bounds_monotone`, default no): for a cap inside another (the angle between the centres plus the inner radius at most the outer radius) the bounds of the inner lie inside those of the outer widened by `MONOTONE_SLACK_M` (0.01 m) on each side, and are numbers where the outer's are. A wrapper says yes only if the field below it does and its own addition keeps it so; every field that says yes SHALL have a contract test over random nested caps.
Over such a field the node selection MAY skip the ceiling work of LOD-008 where it has proved that no tile of the position-only selection can be too steep to morph. The proof SHALL cover, for every level that reads its neighbours' bounds, every tile the selection can hold (a tile split by distance, or one balancing made, which lies within the split distance of the camera plus the radius of the volume of its parent), with the bounds of caps that hold the volumes of those tiles and of the same-level neighbours their morph intervals read, and SHALL test each level's interval on those bounds as `morph_interval_for` tests it. The proof SHALL fail, and the selection take the full path, whenever a bound is not a number, a cap is too wide for the interval, or the work exceeds its budget. The result SHALL be the selection of the full path, node for node and ceiling for ceiling, on every field and camera, and an edit of the items SHALL start every cached bound and certificate over.

Verify: A
Status: active
Source: ADR 0074, LOD-008, perf window 22

#### Scenario: Nested bounds
- **WHEN** random caps and random caps inside them are asked of every field that says its bounds nest
- **THEN** the bounds of the inner cap lie inside those of the outer within the slack, and a field that does not say so, or a stage over one, does not say so

#### Scenario: The same selection
- **WHEN** random steep and gentle spike fields are selected from random cameras over a field that says its bounds nest and over the same field without saying so
- **THEN** the nodes and the ceiling are equal, and over gentle ground the first visits fewer tiles
