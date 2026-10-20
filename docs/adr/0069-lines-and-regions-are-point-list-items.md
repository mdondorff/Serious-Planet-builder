# 0069 Lines and regions are point-list items; a long line splits into linked items
Status: accepted (opus-planner, 2026-10-20: points 1 to 7 follow the owner's E5 answers of 2026-10-19, which approved the points list in the item file; mask mode 1, clip outside, is refused at put until the owner decides openspec/changes/region-mode1; drafted by sonnet-explorer). Extends ADR 0066 point 6 (item files) and ADR 0067 (monotone profiles) to the M6 plan row E5. Replaces the first draft (chains of segment items).
Date: 2026-10-19
Source: M6 plan row E5; docs/status.md "Editor MVP requirements" (lines, regions; owner answers 2026-10-19); ADR 0066; ADR 0067; WDEF-007; WDEF-010; GEN-026

## Context
- Owner: a line is one item with a points list; a line beyond one item's reach limit splits automatically into a few linked items. A region is a closed points list; its mask clips a chosen layer's edits, inside or outside, like a layer mask. Surface types are a data table: rock, sand, grass, snow, forest, ice, desert, wetland, urban.
- An item's reach is limited to one cell edge (40 km, WDEF-007); the item file has scalar parameters only (AUTH-005, schema 1).
- The stamp item carries a kind, three parameter slots and a derived content hash (WDEF-010, ADR 0067); its stage keys by chain with pass-through (GEN-026, STRM-027, ADR 0068).

## Decision
1. **Item file schema 2** adds an optional ordered list `[[points]]` of `latitude`, `longitude` (same ranges and spelling as the anchor), and the optional keys `group` (a ULID: the line or region the item is a part of) and `part` (integer index in the group). Schema 1 files load unchanged (no points, no group). The writer emits schema 2 only for an item that has points; other items stay schema 1, so existing files and hashes do not change. The loader refuses points on a kind that takes none, fewer points than the kind needs (a line 2, a region 3), a repeated consecutive point, and a point list that does not fit the item's reach.
2. **Kinds.** `ridge` and `channel` (open polyline, parameters `amount_m` at least 0 and `width_m` at least 1, the half-width), `region` (closed polygon, parameters `surface_type` as the index of a data-table entry, `mask_mode` 0 none, 1 clip outside, 2 clip inside, and the optional string key `mask_layer` as a top-level key). The anchor is the first point; `reach_m` covers every point plus `width_m` (a region: every point), at most one cell edge.
3. **A long line splits into linked items.** The command layer (E4) splits a line whose extent plus width exceeds the reach limit into parts of at most a limit fraction, sharing their boundary point, all with one `group` and consecutive `part`; deleting or editing a line is one grouped command. The world definition sees independent items; nothing below depends on the group. A region larger than one reach cannot split (a mask of pieces is not the mask): it is refused with the limit, and the owner is told the limit (40 km across).
4. **The points travel with the footprint item** (`FootprintItem.points`, shared and immutable) and enter the item's content hash: BLAKE3-128 under `planet-builder stamp v1` over the kind, the parameter bits and the points' direction bits in order (a stamp without points hashes as before).
5. **Line output.** The distance `d` from a point to the polyline (the minimum over its great-circle segments) gives `x = d / width_m`; a ridge adds `amount_m (1 - x^2)^2`, a channel `-amount_m` out to `x = 0.3` and a smooth rise to 0 at `x = 1`. Within a stage, stamps apply in application order, then the line items add `max(0, largest ridge delta) + min(0, smallest channel delta)`: commutative, so overlapping parts and crossing lines do not depend on order and do not double. The profiles are monotone in `d`, so tile bounds come from the nearest and farthest distance of the cap to the polyline (ADR 0067 point 4).
6. **Region output.** A region changes no height itself. Its surface type is stored (the data table is in the world definition, as liquid kinds are; entries: `rock`, `sand`, `grass`, `snow`, `forest`, `ice`, `desert`, `wetland`, `urban`, each with a debug tint). Its mask: the stage of `mask_layer` multiplies each item's delta at a point by 0 where the point is inside (mode 2) or outside (mode 1) the polygon, 1 elsewhere (a hard edge; a feathered edge is later). A tile whose footprint meets a mask region of its layer keys with the region's hash, so an edit of the region re-keys the masked layer's tiles it touches, and no others.
7. **Overlay.** The debug overlay tints a region by its surface type from a short list of polygons in the frame plan, tested per fragment in tile-local arithmetic (analytic, like the preview stamps), up to a fixed count, nearest first; a mask texture per tile is the fallback if polygons are too many.

## Consequences
- Specs: AUTH-005 (schema 2), WDEF-011/012, GEN-028/029, REND (overlay), plus an item-store round trip; the item file hash of a points item covers the points.
- The item file schema change is hard to reverse once the editor writes line files; until then it is cheap, and schema 1 stays valid.
- A smooth curve is a polyline: the editor chooses the point density (arcs later, additive).
- Regions are limited to one reach (40 km across) in the MVP; larger regions need a different mask representation and a new ADR.
- Easy to reverse: the profile numbers, the debug tints, the mask edge.

## Licences of adopted code or techniques
None.
