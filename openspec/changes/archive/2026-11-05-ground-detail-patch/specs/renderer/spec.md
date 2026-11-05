## ADDED Requirements
### Requirement: REND-055 The patch break-up of the material boundaries
With the ground detail on (REND-034) and the plan's patch above 0 (LOOK-012), the lit terrain shader SHALL shift the slope cosine that feeds the material weights by up to that share of it with two gradient-noise octaves of 32 m and 8 m cells (weights 0.65 and 0.35), anchored to the world like the detail octaves and fading out between 0.12 and 0.5 of a cell a pixel (twice the detail octaves' footprint). The roughness and the albedo follow the shifted weights. With the patch 0 the picture SHALL be what it was, bit for bit, and with the detail strength 0 it SHALL be the off picture. `shadow-cost` and the stills SHALL take a look file (`--look`) whose `ground` table drives the lit plan, and the cost log (REND-044) SHALL carry the share of the patch and of the 64 m and 8 m octaves (REND-057) at each pose: the full draw minus the draw with it at 0.

Verify: C
Status: planned
Source: SPIKE-100 (D1 patch break-up), owner 2026-11-04, planner 2026-11-04 (slice 1 of I8a), REND-034

#### Scenario: Off and on
- **WHEN** a ground-level scene with a slope-dependent material table is drawn with the patch 0, with 0.25, twice, and with 0.25 and the detail strength 0; and the goldens of the lit ground at natural strength are drawn
- **THEN** the patch 0 picture equals the plain detail picture and the goldens need no candidate, the patch picture differs by over 20 % on at least 100 pixels, is equal on a second render and keeps its mean brightness within a factor of 1.4, and the strength 0 picture is the off picture

#### Scenario: A look file in the stills and the cost path
- **WHEN** a pose is drawn without and with a look file whose `ground` table gives the detail strength, a crack depth and an 8 m octave, a look file that does not load is named, and the cost row of a plan with the patch and the two octaves given is read
- **THEN** the pictures differ, the unreadable file is refused naming its path, a run without a look file draws what it drew, and the row has the patch's and the octaves' share as the full draw minus the draw with it at 0 (absent when the plan has it at 0)
