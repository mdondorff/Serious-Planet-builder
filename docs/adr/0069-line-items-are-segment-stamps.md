# 0069 Lines are chains of straight segment items; the stamp machinery carries them
Status: proposed (sonnet-explorer, 2026-10-19; for opus-planner and the owner; easy to reverse until the editor writes line items). Extends ADR 0066 point 6 (item files) and ADR 0067 (monotone profiles) to the M6 plan row E5.
Date: 2026-10-19
Source: M6 plan row E5; docs/status.md "Editor MVP requirements" (lines: ridge, carved channel, region outline); ADR 0066; ADR 0067; WDEF-007; WDEF-010; GEN-026

## Context
- A line (ridge, carved channel) is long; an item's reach is limited to one cell edge (40 km, WDEF-007).
- The item file carries scalar parameters only (`[params]`, finite floats by name; AUTH-005); an array of control points needs a new schema version.
- The stamp item already has a kind, three parameter slots (two reserved at 0), a footprint, a derived content hash (ADR 0067) and a stage that keys by chain (GEN-026, STRM-027, ADR 0068).
- Region masks and the editor's "mask other edits" have no decided meaning; a polygon outline needs arrays or blobs.

## Decision
1. **A line is a chain of segment items.** One item is one straight great-circle segment centred on its anchor: kinds `ridge` and `channel` of the stamp family, with the anchor's heading as the direction, `half_length_m` and `amount_m` (height or depth, at least 0) as parameters, and a half-width `reach_m - half_length_m` (at least 1 m). The editor lays the chain; a chain is a set of items with one `name`.
2. **The stamp stage carries them.** The kinds add to `StampKind`; the item file keys, the item content hash, the footprint row and the chained stage key stay as ADR 0066 and ADR 0067 define them.
3. **Combination.** In one stage, the stamps apply in application order, then the line items of the layer add one delta: the largest ridge delta plus the smallest (most negative) channel delta at the point. This is order-independent for lines, so overlapping segments of a chain do not double at joints.
4. **Profiles are monotone in the distance to the segment** (ridge and channel), so the delta range over a tile's cap comes from the nearest and farthest distance of the cap to the segment (ADR 0067 point 4).
5. **Regions are not decided here.** Surface type, name, the mask meaning and the outline representation go to a second change (E5b) after the owner answers the questions in the line-stage design notes.

## Consequences
- No file schema change: a line item is a TOML item with `kind = "ridge"` or `"channel"` and `half_length_m`, `amount_m` in `[params]`.
- A curve is a polyline of short segments; a smooth curve costs many items. Arcs (a curvature parameter) are a later, additive kind.
- A segment longer than about 40 km minus its width is refused; the editor splits longer lines.
- Easy to reverse: nothing is persisted until the editor writes line items; the kinds can be replaced by a spline item with control points in a schema version 2.

## Licences of adopted code or techniques
None.
