## Open questions for the planner
- IDs: requirement IDs for the material table growth, the per-material coefficients in the look, the six octaves; LOOK schema 7 is a bump of LOOK-002.
- Sand: SPIKE-100 lists five materials without sand; today's sand is the desert ground. Proposed: six slots (rock, sand, grass, soil, scree, wet), sand kept.
- Bit-for-bit: the four-octave Earth picture must stay exact (goldens `gpu_ground_look`). Proposed: the new coefficient table defaults to today's constants, the two new octaves are zero-weighted by default and skipped by a uniform branch.

## Decisions (proposed)
- Weights: `vec4` becomes `vec4` + `vec2` (slots 0-3, 4-5); `material_weight` indexed by first word, 24 vec4s of curves.
- Coefficient table: per material and octave `(albedo, tilt)` as look data (6 octaves x 6 materials x 2 floats = 72 floats, 18 vec4s); the shader sums `dot(w, coef)` per octave as now.
- Roughness table: six entries; gloss modifiers unchanged.
- Scree and wet curves in `EARTH` are off; the owner's look turns them on (slope cosine 0.62-0.78 for scree below rock; wet from the channels in 2b, slope-only stand-in none).
