# Change: turn-cover

## Why
- Owner (2026-10-21, P1-4 tile flashing): every tile turns "stand-in" in the F3 overlay while the camera only turns. The frame draws the resident cover when the plan in hand no longer serves the view, and labelled all of it stand-in.
- The answers to the two questions: (1) the cover is not coarser than the plan: with R resident it is the plan of the frame's camera node for node (`cover.rs`, already tested); only the label was wrong. (2) A turn invalidates the plan because the plan culls to the exact frustum of its own camera: any turn that shows an area outside it fails `plan_covers_view`; a margin round the plan's frustum would keep small turns on the plan (a follow-up; it costs tiles).

## What changes
- `cover_selection_exact`: the cover with, per tile, whether it is exact (drawn whole, not standing in for a missing child of the plan, not made by balancing a standing tile). `cover_selection_masked` is it without the flag.
- `draw_plan` labels cover tiles exact or stand-in by it (STRM-033, new).
- No change to which tiles are drawn.

## Tests
- `cover.rs`: a turn over a complete residency draws the cover with no stand-in; over a partial one the tiles of the plan are exact and the others stand-in. (Failed before: 431 of 431 tiles stand-in.)

Expected golden changes: none (no golden draws a cover frame; the state colours only show in the F3 overlay).
