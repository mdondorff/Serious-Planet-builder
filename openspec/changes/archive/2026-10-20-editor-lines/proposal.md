## Why
The editor MVP draws lines (ridge, channel) and regions (surface type, layer mask) as point lists (ADR 0069, owner 2026-10-19). The tools, the commands and the inspector all edit a point list before any item holds it, and the world definition does not hold points yet (line-stage, region-stage: sonnet-explorer). opus-planner (2026-10-19): the editor side of E5.

## What Changes
- Stage 1 (this change): `planet_editor::polyline`, GPU-free and independent of the item type: points as latitude and longitude in degrees; the line rule (2 to 64 points, finite, in range, no repeated neighbour); the region rule (3 to 64, within one cell edge of the first point, outline without a self-crossing); `split_line` (a line beyond one item's reach becomes parts of at most 80 % of the limit that share their boundary point, a long segment cut at great-circle points, at most 64 points a part); picking (`nearest_point`, `nearest_segment` with the foot of the click, open or closed); insert, move and remove a point, each returning a new list.
- Later stages, after explorer's item model (`Item.points`, `group`, `part`, `mask_layer`, ridge, channel and region kinds) is on main: the grouped command (one undo step for a split line), the line and region tools, the inspector (surface type, mask layer, inside or outside), relevance of lines and regions.

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves.

## Out of scope
The item file, the world definition and the generator stages (sonnet-explorer); the overlay (VIEW-025).
