## Why
- SPIKE-109 (2026-11-12): with the class band off, the base still holds 11 to 12 % of the land in large shallow basins on the coast plains (R3a 11.9 %, R3b 17.3 %, R1h 19.8 % with the coast term alone: depth p50 1.7 to 3.5 m); round 1 cuts their outlet paths (5.001 m cells, G-PROFILE's rises).
- Architect, 2026-11-12 (D114): a regional rise of the coast plains away from the sea, built at the source and switched off until previewed (ADR 0118 points 2 and 4); the +3 km tilt is refused.

## What changes
- GEN-180: `added(d) = weight x t x D x s(d / D)` metres at `d` km inland of the coast, `s(x) = x (2 - x)` saturating at 1, so the added height never exceeds `t x D`; weight = the plain and rolling-plain class weights (mountains, plateaus and hills get none); land only; added to `e0` after the class weights are decided (it moves no class). Off by default: every `e0` and every key is the build before.
- The values of `t` and `D` come from a sweep (t in {2, 4, 8} m/km, D in {100, 200, 400} km on R3a and R3b; the smallest `t x D` with added height over land p50 at most 100 m, max at most 400 m and interior basins over 30 cells at most 7 % with the band cap on), measured in a separate window; the switch-on PR waits for the owner's look approval.

## Not in this change
- The band cap (GEN-179, merged), the plateau (PR 3); R1h is left to the cap and the plateau.

## Impact
generators (classes.rs, bake/inputs.rs). Default path unchanged.
