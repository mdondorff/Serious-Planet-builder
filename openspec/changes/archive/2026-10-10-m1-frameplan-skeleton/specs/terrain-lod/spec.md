## ADDED Requirements

### Requirement: LOD-006 Plan builder
The GPU-free plan builder SHALL lay tiles out in pixel rectangles, compute each camera-relative origin as the float64 difference rounded once to float32, place tile origins on the reference sphere, give distinct stable colours to distinct tile IDs, and reject unusable requests (no tiles, unknown face mapping, frame too small) with a message.

Verify: A
Status: active
Source: CON-03, CON-11, ADR 0001

#### Scenario: Two tiles
- **WHEN** two tiles are planned into a 256 by 128 frame
- **THEN** their rectangles are [0,0,128,128] and [128,0,128,128] and each camera-relative origin equals the f64 difference rounded once
