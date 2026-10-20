## Context
- Owner (2026-10-19): the overlay tint is the coder's call. The regions come from `region-items` (surface table with a tint per entry, polygon points in the footprint item).

## Decisions
1. **Overlay.** `Overlay::Regions` (shader code 3): the frame plan carries up to 8 region polygons nearest the camera (each: surface tint, up to 64 camera-relative f32 points in the plan's tile-local frame); the lit fragment stage tests the fragment's tangent-plane position against them (even-odd), tints with the surface colour at weight 0.45, counted by the plan text (` overlay regions`). The tint is analytic: no mask texture.
2. **Plan text.** One line per polygon after the overlay token; a plan with no overlay has no such text and the hash it had.
3. **Source of the polygons.** The app reads the region items near the camera from the definition (footprint cap query over the shown tiles) and hands the nearest eight to the plan; the renderer decides nothing (CON-03).
4. **Not now.** Feathering, regions over 40 km, texture masks, per-region albedo in the exact render.

## Risks
- Eight polygons of 64 points is the budget of the overlay; more are dropped farthest first and counted.
