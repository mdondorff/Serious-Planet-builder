# Change: ground-gloss

## Why
- M7 G2b (opus-planner, 2026-10-21; owner: roughness and shine are in M7): the ground has no specular term at all (SPIKE-27). Wet or icy ground is glossy, rock and soil rough, from the same weights as the detail.
- The owner must see the detail in the editor and viewer without code changes (opus-planner, 2026-10-21): F6.

## What changes
- Shader: `ground_specular` (GGX, Smith-Schlick visibility, Schlick Fresnel) and the roughness from the weights and two modifiers, added to the lit view of the detailed ground in the terrain and the atmosphere path (the atmosphere's transmitted, shadowed sun scales it). Globals +48 B (the gloss table; `f0` in the detail vec4).
- `GlossParams` (look data): roughness per material, the wet and icy modifiers, `f0`; `LitPlan::gloss`, written in the plan's text only when not the default (LOOK-010).
- F6 cycles `GroundDetailMode` (natural 0.6, full 1, off 0) in the viewer and the editor; the title and the help name it; the window starts at natural (VIEW-046). The window's lit plan takes the mode's strength; every other plan keeps 0.
- The review stills tool puts the sun ahead of the camera so the highlight shows.

## Design notes
- Same function, same inputs: the roughness reads the weights of LOOK-008 and the input vector; the wet modifier reads the reserved moisture input, so a driver map (M8) makes wet ground glossy with a look change only.
- The seam for the photo layer (REND-034) extends to a roughness map: a texture source returns `(mul, tilt, roughness)` for a material.
- Natural strength 0.6 from the tuning pass on the review stills (1 is full, the stills' look at 1.8 m and 0.5 m); the owner's eye decides the final number (one constant).

## Tests
- Tier A: the gloss in the plan text, its limits, the roughness of a point (LOOK-010); F6 (VIEW-046) and the key tables (VIEW-003, VIEW-022: bound, named by the help).
- Tier C: the highlight is additive, shows, follows the table, and is absent when the detail is off or in the albedo view (REND-035). The tier-C suite passes with the goldens as they are.

Expected golden changes: none (the strength is 0 in every plan the goldens use).
