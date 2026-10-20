## Why
The debug overlay should show the surface type of the regions (owner, 2026-10-19: the tint is the coder's call). The region item, its table and its mask are the change `region-items`; this change is the overlay that tints the terrain by the surface type of the region over each fragment (analytic, no mask texture).

## What Changes
- **Overlay** (VIEW-030): `Overlay::Regions` (shader code 3, plan token ` overlay regions`) tints each fragment by the surface type of the region over it, analytic from a short polygon list; F3 cycles tiles, mesh, regions, off. VIEW-014 is modified for the fourth value.
- **Departure from the first draft**: the polygons are not part of the lit plan or its text. `LitPlan` is `Copy` and one text line, so the list is a `planet_frame::regions::RegionList` the app decides per frame and hands to the renderer (`TerrainRenderer::set_regions`), like the edit preview (REND-033). It has its own text form (round trip, nearest eight, the rest counted); the plan carries only the overlay token.
- **Not in this change**: a feathered edge, regions larger than one reach, the editor's region tools (crusader), the overlay without the editor.

## Impact
- Spec: VIEW-030 (added), VIEW-014 (modified: four values, the token, the label), viewer. Code: `frame` (`look::Overlay::Regions`, `regions`), `render` (the globals table, the lit fragment tint), `app` (the polygons from the definition near the camera, the F3 cycle).
- Goldens: none moves with the overlay off.
- numerics-reviewer (shader) and spec-guardian ran before archive.
