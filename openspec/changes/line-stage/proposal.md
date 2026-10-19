## Why
M6 plan row E5 asks for splines (ridge, carved channel). Owner answers (2026-10-19): a line is one item with a points list, not a chain of segment items; a line beyond one item's reach splits into a few linked items. ADR 0069 (proposed) decides the representation. The item file needs a points list (schema 2); the stamp stage (GEN-026) takes the new line kinds. Regions are the second change (`region-stage`).

## What Changes
- **Item file schema 2** (AUTH-005): optional `[[points]]` (`latitude`, `longitude`), `group` and `part`; schema 1 files load unchanged, the writer emits schema 2 only for an item with points; the content hash covers the points.
- **Kinds `ridge` and `channel`** (WDEF-011): an open polyline of 2 to 64 points, parameters `amount_m` (at least 0) and `width_m` (the half-width, at least 1); the anchor is the first point; `reach_m` covers every point plus `width_m`.
- **Line profiles** (GEN-028): monotone in the distance to the polyline; a ridge adds a smooth crest, a channel cuts a bed with sloped banks. After the stamps, the line items add `max(0, largest ridge delta) + min(0, smallest channel delta)` (order-independent).
- **Bounds** (GEN-028): the delta range over a tile's cap from the nearest and farthest distance of the cap to the polyline (ADR 0067 point 4).
- **Item put** (WDEF-011): a put refuses a line that does not fit its reach; the footprint item carries the points (shared, immutable).
- **Contract tests**: output unchanged beyond `reach_m` and within `max_delta_m` (ADR 0021 point 7), pass-through on tiles away from the line, determinism.
- **Not in this change**: regions (`region-stage`), the split of a long line into linked items (the command layer, E4; this change only reads `group` and `part`), preview of lines (E3 previews stamps only; a line shows after its exact tiles arrive), the editor's drawing of lines (crusader builds the editor tools against this item shape).

## Impact
- Specs: AUTH-005 (modified), WDEF-011 (added, world-definition), GEN-028 (added, generator-pipeline).
- Code: item model and item file (schema 2), `world-def` (kinds, points, hash), `generators` (`stamp.rs`: polyline distance, line profiles, bounds), `app` (`worldfield`: stage selection unchanged), `cache` key tests.
- Goldens: none moves (no line exists in any golden scene); a tier C image of a ridge and a channel is a candidate in `target/review/lines/`, not a golden.
- Fidelity: indicative; profiles are authored shapes, not physical.
- numerics-reviewer (core, generators), spec-guardian before archive; owner confirms ADR 0069.
