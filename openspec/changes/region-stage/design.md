## Context
- Owner (2026-10-19): a region mask clips a chosen layer's edits, inside or outside; the outline is a closed points list; surface types are a data table; the overlay tint is the coder's call.
- Depends on `line-stage` (schema 2 points, `FootprintItem.points`, the stamp stage reading line items).

## Decisions
1. **Surface table.** `SurfaceTable` in `world-def`: nine entries in a fixed order (`rock`, `sand`, `grass`, `snow`, `forest`, `ice`, `desert`, `wetland`, `urban`), each with a name and an sRGB debug tint; the item's `surface_type` is an index. The table is part of the world definition file's data (like the liquid kinds), so a later entry is data, not code; an index beyond the table is refused at put.
2. **Item.** `region`: `[params]` `surface_type` and `mask_mode` (0, 1, 2; whole numbers written as floats, exact), no height; the layer to clip is the optional top-level string key `mask_layer` of the item file (schema 2, after `part`; ADR 0069 point 2), required for a `mask_mode` other than 0 and refused otherwise. `reach_m` covers every point; at most one cell edge (40 km): a larger region is refused with the limit.
3. **Inside test** (CPU, f64, `libm`): a point is inside the closed polygon by the even-odd rule on the gnomonic projection of the points and the point onto the tangent plane at the first point (the polygon fits one reach, so the projection is monotone and has no wrap), with the boundary counted inside.
4. **Mask.** In the stage of `mask_layer`, for each item delta at a point, the factor is 0 when a mask region of that layer clips the point's side (mode 2: inside; mode 1: outside), else 1; several regions multiply. A hard edge: a delta is wholly kept or wholly removed per sample. Mask regions enter the footprint of the layer's tiles (`FootprintItem` of a region carries its points); the tile key chain covers them, so editing a region re-keys the layer's tiles it touches. A mode-1 region (clip outside) removes the layer's edits everywhere outside the polygon, so its footprint is every tile of the layer within the body: it is limited to one region per layer for that mode and costs a full refresh (documented in the spec).
5. **Bounds.** A mask only scales deltas by 0 or 1, so the delta range of the layer over a cap is the range with and without the mask joined, which is the range without it (the existing bounds stay valid).
6. **Overlay.** `Overlay::Regions` (shader code 3): the frame plan carries up to 8 region polygons nearest the camera (each: surface tint, up to 64 camera-relative f32 points in the tile-local frame of the plan); the lit fragment stage tests the fragment's tangent-plane position against them (even-odd), tints with the surface colour at 0.45 weight, counted by the plan text (` overlay regions`). The tint is analytic: no mask texture.
7. **Not now.** Feathering, regions over 40 km, texture masks, per-region albedo in the exact render.

## Risks
- The mode-1 footprint (everywhere outside) is costly; the editor should prefer mode 2.
- Eight polygons of 64 points in the plan is the budget of the overlay; more are dropped farthest first and counted.
- `mask_layer` names a layer: renaming a layer leaves a region clipping nothing; the command layer rewrites the name when a layer is renamed.
