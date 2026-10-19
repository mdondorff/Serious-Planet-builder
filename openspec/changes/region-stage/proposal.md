## Why
M6 plan row E5 asks for regions: a mask over other edits, a stored surface type and a name. Owner answers (2026-10-19): a region is a closed points list; its mask clips a chosen layer's edits, inside or outside, like a layer mask; surface types are a data table (rock, sand, grass, snow, forest, ice, desert, wetland, urban); the overlay tint is the coder's choice. ADR 0069 (proposed) decides the representation. This change follows `line-stage` (item file schema 2, points in the footprint item).

## What Changes
- **Surface type table** (WDEF-012): a data table in the world definition (not a code enum), nine entries with a debug tint each.
- **Kind `region`** (WDEF-012): a closed polygon of 3 to 64 points (`[[points]]`, AUTH-005), parameters `surface_type` (an index of the table), `mask_mode` (0 none, 1 clip outside, 2 clip inside) and the item's top-level string key `mask_layer` (the layer whose edits the region clips; an optional key of schema 2, in AUTH-005 since `line-items`). A region changes no height itself.
- **Mask stage** (GEN-029): the stage of `mask_layer` multiplies each of its items' deltas at a point by 0 where the point is on the clipped side of a mask region and by 1 elsewhere (a hard edge); a tile whose footprint meets a mask region of that layer keys with the region's hash.
- **Overlay** (VIEW-030): the debug overlay `Regions` tints each fragment by the surface type of the region over it, analytic in tile-local arithmetic from a short polygon list in the frame plan.
- **Not in this change**: a feathered mask edge, regions larger than one reach (40 km), the editor's region tools (crusader builds them against this item shape), splitting a region.

## Impact
- Specs: WDEF-012 (added), GEN-029 (added), VIEW-030 (added). Code: `world-def` (table, kind, mask lookup in the footprint), `generators` (`stamp.rs` mask), `frame` (`look::Overlay::Regions`, plan polygon list, plan text), `render` (lit shader overlay), `app` (`F3` cycle).
- Goldens: none moves with the overlay off; a tier C image of an overlay and of a masked edit is a candidate in `target/review/regions/`, not a golden.
- numerics-reviewer (generators, shader), spec-guardian before archive; owner confirms ADR 0069.
