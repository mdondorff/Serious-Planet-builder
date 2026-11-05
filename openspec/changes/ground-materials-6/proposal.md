## Why
- I8a slice 2a (planner, 2026-11-04), step (b) of three, golden-neutral: the ground detail gets two more material slots, scree and wet ground (SPIKE-100 D1), off in the Earth default. Step (a) (#1176) made the octave coefficients look data; step (c) adds two octaves.

## What Changes
- Slots six: rock, sand, grass, soil, scree, wet. The curve table, the roughness table and the octave coefficient table grow by the two; the weights are normalised over all six.
- The look file takes `[body.<id>.ground.scree]` and `[body.<id>.ground.wet]` curve tables and a `scree` and a `wet` entry (albedo coefficient, tilt) in each `ground.octave.<name>` table; look schema version 8 with an identity step.
- The Earth default has scree and wet off (no curve pair), so the weights, the plans' picture and every golden stay as they were.

## Impact
- Code: `frame` (materials, look file), `render` (terrain shader and globals). Requirements: REND-056 added; LOOK-002 wording (schema 8, the version 8 fixture) edited in the unarchived delta of `ground-detail-patch`, which owns that text.
- The inputs that turn the new slots on (deposition depth, drainage area; SPIKE-100 thresholds) come with slice 2b; until then the slots are driven by whatever curves a look gives them.
