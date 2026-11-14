## Why
- SPIKE-109 (2026-11-12): the base without the band holds 11 to 12 % of the land in large shallow basins; part of them are plateau bowls (the plateau field is a thresholded noise: its interior dips where the noise dips).
- Architect, 2026-11-12: plateau uplift monotone from the rim to the interior (smoothstep of the distance to the edge), built at the source, switched off until previewed (ADR 0118 points 2 and 4).

## What changes
- GEN-183: the plateau's uplift `U x plateau_field` is replaced, when the rule is on, by `U x smoothstep(-W, W, x)` with `x` the signed distance in km inside the 0.5 contour of the plateau field (a lattice of its own with the cap `W`); applied to `e0` after the class weights are decided (no class moves). Off by default: every `e0` and every key is the build before.
- `W` comes from a probe (the smallest width that removes the plateau bowls on R3a, R3b and R1h); the switch-on PR waits for the owner's look approval.

## Not in this change
- The band cap (GEN-179) and the coast tilt (GEN-180), both merged; the flags on generate and test-render (GEN-181).

## Impact
generators (classes.rs, bake/inputs.rs). Default path unchanged.
