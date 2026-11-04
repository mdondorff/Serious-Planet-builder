## Decisions
- The shader keeps its arithmetic order. The weights of the new slots are a second vector `w2` (scree, wet); every sum that took `w` takes `w2` as a further term, which is +0 where the slots are off, so the Earth picture is bit for bit the old one (checked on the goldens, 8-bit output).
- Octave coefficient layout: a fourth `vec4` an octave, (scree albedo, wet albedo, scree tilt, wet tilt); the crack word moves from index 12 to 16. The roughness table is two `vec4`s of material roughness (the first four; scree, wet, 0, 0) then the two modifiers.
- Fixed tints of the new slots are shader constants like the old ones (scree 1.0, 0.97, 0.94; wet 0.92, 0.92, 0.95); their mean tone is (0.33, 0.31, 0.29) and (0.07, 0.06, 0.05).
- Earth values of the new coefficients (rock-like scree, soil-like wet ground) are placeholders for the look sheet of step (c); roughness 0.90 for scree, 0.35 for wet.
- Cost: one more `vec2` of weights, two curve words of 4 and eight more coefficient words; measured with step (c) via `shadow-cost` (indicative until the machine restart).
