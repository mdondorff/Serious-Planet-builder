## Why
M6 plan row E5 asks for splines (ridge, carved channel) and regions (mask, surface type stored, name). The stamp stage (GEN-026) and the item files (AUTH-005) exist; a line is long, an item's reach is limited to 40 km, and the item file carries scalar parameters only. ADR 0069 (proposed) decides the representation: a line is a chain of straight segment items of the stamp family. Regions are a second change (E5b) once the owner answers the open questions in `design.md`.

## What Changes
- **Kinds `ridge` and `channel`** (WDEF-011): a straight great-circle segment centred on the item's anchor, along the anchor's heading, with `half_length_m` and `amount_m` (height or depth, at least 0) in `[params]`; the half-width is `reach_m - half_length_m` (at least 1 m). Item file, content hash (ADR 0067), footprint row and chained stage key follow the stamp rules; no file schema change.
- **Line profiles** (GEN-028): monotone in the distance to the segment; a ridge adds a smooth crest, a channel cuts a bed with sloped banks. Within a stage, stamps apply in application order, then the line items add one delta: the largest ridge plus the smallest channel at the point (order-independent for lines).
- **Bounds** (GEN-028): the delta range over a tile's cap comes from the nearest and farthest distance of the cap to the segment (ADR 0067 point 4); a segment whose parameters are refused is not put.
- **Contract tests**: output unchanged beyond `reach_m` and within `max_delta_m` (ADR 0021 point 7), pass-through on tiles away from the chain (GEN-027), determinism.
- **Not in this change**: regions (E5b), preview of lines (E3 previews stamps only; a line shows after the exact tiles arrive), the editor's drawing of chains (U3/E4).

## Impact
- Specs: WDEF-011 (added, world-definition), GEN-028 (added, generators).
- Code: `world-def` (kinds, file mapping, hash), `generators` (`stamp.rs`: segment distance, line profiles, bounds), `app` (`worldfield`: unchanged stage selection), `cache` key tests.
- Goldens: none moves (no line exists in any golden scene); a tier C image of a ridge and a channel is a candidate in `target/review/lines/`, not a golden.
- Fidelity: indicative; profiles are authored shapes, not physical.
- numerics-reviewer (core, generators), spec-guardian before archive; owner confirms ADR 0069.
