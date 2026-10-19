## Why
M6 plan row E5 asks for splines (ridge, carved channel). Owner answers (2026-10-19): a line is one item with a points list, not a chain of segment items; a line beyond one item's reach splits into a few linked items. The item model (schema 2 points, the kinds, the stamp's points) is the change `line-items`; this change gives the line kinds their height output in the stamp stage (GEN-026). Regions are the third change (`region-stage`).

## What Changes
- **Line profiles** (GEN-028): monotone in the distance to the polyline; a ridge adds a smooth crest, a channel cuts a bed with sloped banks. After the stamps, the line items add `max(0, largest ridge delta) + min(0, smallest channel delta)` (order-independent).
- **Bounds** (GEN-028): the delta range over a tile's cap from the nearest and farthest distance of the cap to the polyline (ADR 0067 point 4).
- **Contract tests**: output unchanged beyond `reach_m` and within `max_delta_m` (ADR 0021 point 7), pass-through on tiles away from the line, determinism.
- **Not in this change**: regions (`region-stage`), the split of a long line into linked items (the command layer, E4), preview of lines (E3 previews stamps only; a line shows after its exact tiles arrive), the editor's drawing of lines (crusader builds the editor tools against the item shape).

## Impact
- Spec: GEN-028 (added, generator-pipeline).
- Code: `generators` (`stamp.rs`: polyline distance, line profiles, bounds, the tile stage), `cache` key tests.
- Goldens: none moves (no line exists in any golden scene); a tier C image of a ridge and a channel is a candidate in `target/review/lines/`, not a golden.
- Fidelity: indicative; profiles are authored shapes, not physical.
- numerics-reviewer (generators), spec-guardian before archive; owner confirms ADR 0069.
