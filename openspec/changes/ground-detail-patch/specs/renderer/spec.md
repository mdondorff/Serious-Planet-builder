## ADDED Requirements
### Requirement: REND-055 The patch break-up of the material boundaries
With the ground detail on (REND-034) and the plan's patch above 0 (LOOK-012), the lit terrain shader SHALL shift the slope cosine that feeds the material weights by up to that share of it with two gradient-noise octaves of 32 m and 8 m cells (weights 0.65 and 0.35), anchored to the world and fading at four times the detail octaves' footprint, and SHALL grow the rock's tilt of the 2 m and 25 cm octaves with steepness, up to 3 and 2 times past about 60 degrees. With the patch 0 the picture SHALL be what it was, bit for bit, and with the detail strength 0 it SHALL be the off picture.

Verify: C
Status: planned
Source: SPIKE-100 (D1 patch break-up, D2 steepness), owner 2026-11-04, planner 2026-11-04 (slice 1 of I8a), REND-034

#### Scenario: Off and on
- **WHEN** a ground-level scene with a slope-dependent material table is drawn with the patch 0, with 0.25, twice, and with 0.25 and the detail strength 0
- **THEN** the patch 0 picture equals the plain detail picture, the patch picture differs by over 20 % where the boundary moves, is equal on a second render and keeps its mean brightness within a factor of 1.4, and the strength 0 picture is the off picture
