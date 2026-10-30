## MODIFIED Requirements

### Requirement: BUDG-002 GPU time shares
The GPU time of a frame at 1440p on the reference machine SHALL stay within 16.6 ms, split into shares in milliseconds per frame (median): terrain, water and shadows 5.0, sky and weather 3.0, physics 0.5, game rendering 4.0, post and present 1.0, headroom 3.1. Inside terrain's share, shadow layer 1 (terrain occluders: clipmap upkeep, march, upsample, temporal) has 1.5 ms (REND-052) and the material map 1.5 ms (REND-044); shadow layer 2 (object occluders, ADR 0085 point 8) is reserved inside game rendering's share until it is built.

Verify: C
Status: planned
Source: ADR 0083 points 1 and 3, SPIKE-44, REND-044, ADR 0085, REND-052

#### Scenario: Terrain alone
- **WHEN** the standard flight paths run with no reserved subsystem
- **THEN** the median GPU time of the frame is at most terrain's share
