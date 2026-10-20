## Why
The debug overlay should show the surface type of the regions (owner, 2026-10-19: the tint is the coder's call). The region item, its table and its mask are the change `region-items`; this change is the overlay that tints the terrain by the surface type of the region over each fragment (analytic, no mask texture).

## What Changes
- **Overlay** (VIEW-030): `Overlay::Regions` (shader code 3) tints each fragment by the surface type of the region over it, analytic in tile-local arithmetic from a short polygon list in the frame plan; F3 cycles tiles, mesh, regions, off.
- **Not in this change**: a feathered edge, regions larger than one reach, the editor's region tools (crusader).

## Impact
- Spec: VIEW-030 (added, viewer). Code: `frame` (`look::Overlay::Regions`, plan polygon list, plan text), `render` (the lit fragment tint), `app` (the polygons from the definition near the camera, the F3 cycle).
- Goldens: none moves with the overlay off.
- numerics-reviewer (shader), spec-guardian before archive.
