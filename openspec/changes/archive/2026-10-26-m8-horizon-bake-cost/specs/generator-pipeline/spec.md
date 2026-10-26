## ADDED Requirements

### Requirement: GEN-085 The horizon march walks a step table
`generators::horizon::bake_horizon` SHALL make the distance of every step of the march and its curvature drop `d^2 / (2 R)` once per bake and walk that table for every texel and azimuth. The table's values and the march's arithmetic SHALL be the ones of a march that adds its steps as it goes, so the bytes of a map are those of GEN-044 before this requirement.

Verify: A
Status: active
Source: opus-planner (2026-10-26, criterion 6), GEN-044, CON-14

#### Scenario: Equal to the long form
- **WHEN** a rough raster is baked at three ranges, two growths and three body radii
- **THEN** every byte equals that of a march that forms each distance and drop as it goes
