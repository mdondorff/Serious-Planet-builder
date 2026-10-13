## Why

M3 item 6c (ADR 0030 point 1): the local erosion filter is SPIKE-04 variant B, slope-incised ridged creases, as a pure function of the direction under a new implementation id, with bounds as sound as the terrain field's.

## What Changes

- `crates/generators/src/erosion.rs`: `ErosionGenerator::field` and `ErosionField` (`cpu-erosion-v1`). Height = the bake's elevation plus `a(E) c(slope) S(dir)`; the creases are the spike's variant B (12 octaves from a 25 km spacing, `0.25 - (1 - |n|)^3`). Only the variant B formula is taken from `spike/erosion-variants`, which stays unmerged; nothing of the spike harness or its public noise functions.
- The slope is a basis-free gradient (sum over the three axes of the central difference along the tangent part of the axis times that part), not the spike's tangent basis, which switched at `|x| = 0.9`. The radius is an input (the slope is rise over run) and is read through the context.
- `height_bounds` is interval arithmetic: the terrain field's bake scan (extracted unchanged as `terrain::scan_base`, which now also reports the largest lattice steps), a slope interval from the gradient at the centre and a derived bound on its variation over the cap, amplitude and slope factor monotone in elevation and slope, per-octave noise bounds, and the product. The spike's measured Lipschitz constant x3 is not used.
- It replaces the terrain field's fBm detail rather than adding to it (as the spike did: it too read the bake directly); `cpu-terrain-v1` is unchanged and nothing consumes either field in the app yet.
- Tier A (GEN-017): bounds over random, edge and corner caps; a steep ramp lattice where the slope factor changes within the cap (a mutation of the slope spread to zero fails it); hugging and f32 rounding; keys (seed, radius, bake, implementation id); read failure; continuity across face edges; threads; known answers; steep ground is cut deeper. Frame: planning at cells 32 and viewport / tau 240 along the standard flight on a 256-cell bake.
- Not done here: the same planning check on the production-resolution bake (M3 acceptance).
