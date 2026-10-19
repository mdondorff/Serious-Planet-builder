## Why
M6 plan row E5, first half: the item model of lines and regions (owner answers, 2026-10-19; ADR 0069, proposed). A line is one item with a points list, a long line splits into linked items (by the editor's command layer), a region is a closed points list. The editor session (crusader) builds its commands, inspector and tools against this item shape, so the model lands before the line stage that gives it height output (`line-stage`).

## What Changes
- **Item file schema 2** (AUTH-005): optional `[[points]]` (`latitude`, `longitude`), `group`, `part` and `mask_layer`; schema 1 files and their hashes are unchanged; the content hash covers the group, part, mask layer and points of a schema-2 item.
- **Kinds `ridge` and `channel`** (WDEF-011): stamp kinds with the parameters `amount_m` and `width_m` and a points list; the stamp carries the points (shared, immutable) and its content hash is derived from kind, numbers and points; put refusals; the footprint returns the line with its points.
- **Item API**: `Item.points`, `group`, `part`, `mask_layer`; `Stamp::with_points`, `StampKind::{Ridge, Channel}`, `param_keys`.
- **Not in this change**: the height output of lines (`line-stage`: GEN-028; until then a ridge or channel adds no height and its tile bounds are 0), regions as stamps and the surface table (`region-stage`: a `region` item is accepted by the file with its points and `mask_layer`, and has no height output), the long-line split (the editor), preview of lines (the preview list skips line kinds).

## Impact
- Specs: AUTH-005 (modified, authored-layers), WDEF-011 (added, world-definition).
- Code: `world-def` (`stamp.rs`, `item.rs`, `lib.rs`), `generators` (`stamp.rs`: line kinds add nothing yet), `frame` (`preview.rs`: lines skipped), `editor` (`commands.rs`: the new fields default to none).
- Goldens: none moves (no line exists in any golden scene).
- numerics-reviewer: not needed (no numeric output); spec-guardian before archive; owner confirms ADR 0069.
