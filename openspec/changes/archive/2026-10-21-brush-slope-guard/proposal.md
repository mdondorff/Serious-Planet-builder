# Change: brush-slope-guard

## Why
- SPIKE-26: 37 of 45 refused plan stops sit at the owner's raise-stroke clusters (44 strokes of 5 m on 100 m, a 220 m spike, about 73 degrees). The bounds are exact; the ground is steeper than a tile can morph (LOD-008), so no bound can fix it. ADR 0074 (proposed) would draw such ground coarsely; until it is built, and whenever it is, the editor can keep new strokes from making ground that cannot be drawn fully (opus-planner, 2026-10-21: option B).

## What changes
- `LodParams::drawable_slope`: the steepest neighbourhood slope that leaves a morph interval, from the plan's own rule (about 2.5 at the defaults).
- The brush guard (`planet_editor::slope`): a raise or lower is filed with the strength that keeps the summed largest gradient of the strokes it overlaps under `drawable_slope - BASE_SLOPE_SHARE` (1.0; about 1.5), or refused where there is no room, with a status line; a flatten and a smooth are never held back.
- Existing items are untouched: an item (or group) already over the limit shows "too steep to draw fully" in the inspector and the layer list.
- No item file, schema, key or generator change.

## Tests
- `editor/tests/slope.rs` (VIEW-043): the limit, a stack that passes up to the limit then is clamped then refused, a lower counted by size and a flatten and smooth never clamped, the warning of an item already over; `frame` lod test for `drawable_slope`; `app` panels test for the badges.

Expected golden changes: none.
