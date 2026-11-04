## ADDED Requirements
### Requirement: REND-055 The patch break-up of the material boundaries
With the ground detail on (REND-034) and the plan's patch above 0 (LOOK-012), the lit terrain shader SHALL shift the slope cosine that feeds the material weights by up to that share of it with two gradient-noise octaves of 32 m and 8 m cells (weights 0.65 and 0.35), anchored to the world like the detail octaves and fading out between 0.12 and 0.5 of a cell a pixel (twice the detail octaves' footprint). The roughness and the albedo follow the shifted weights. With the patch 0 the picture SHALL be what it was, bit for bit, and with the detail strength 0 it SHALL be the off picture.

Verify: C
Status: planned
Source: SPIKE-100 (D1 patch break-up), owner 2026-11-04, planner 2026-11-04 (slice 1 of I8a), REND-034

#### Scenario: Off and on
- **WHEN** a ground-level scene with a slope-dependent material table is drawn with the patch 0, with 0.25, twice, and with 0.25 and the detail strength 0; and the goldens of the lit ground at natural strength are drawn
- **THEN** the patch 0 picture equals the plain detail picture and the goldens need no candidate, the patch picture differs by over 20 % on at least 100 pixels, is equal on a second render and keeps its mean brightness within a factor of 1.4, and the strength 0 picture is the off picture
