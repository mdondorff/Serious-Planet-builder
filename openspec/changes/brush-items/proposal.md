## Why
M6 plan row E6, first half: the item model of the brush (raise, lower, smooth, flatten). ADR 0070 (proposed): a brush item holds its strokes inline in its TOML file, capped, grouped by area by the editor's commands; the blob store is specified there and deferred. The height output of strokes is the change `brush-stage`; the editor's brush tool is the support lane's.

## What Changes
- **Item file schema 3** (AUTH-005): optional ordered `[[strokes]]` (`kind`, `radius_m`, `strength_m`, `target_m` for a flatten, and the stroke's own `[[strokes.points]]`); schema 1 and 2 files and their hashes are unchanged; the content hash of a schema-3 item covers the strokes.
- **Kind `brush`** (WDEF-013): a stamp kind with no `[params]`, holding its strokes (shared, immutable) and a content hash derived from them; caps (64 points per stroke, 64 strokes and 4,096 points per item); reach and `max_delta_m` rules; put refusals; the footprint returns the item with its strokes.
- **Item API**: `Item.strokes`, `Stroke`, `BrushKind`, `Stamp::with_strokes`, `StampKind::Brush`.
- **Not in this change**: the height output of strokes (`brush-stage`: until then a brush item adds no height and its tile bounds are 0), the editor's tool and grouping commands, the preview of strokes, the blob store.

## Impact
- Spec: AUTH-005 (modified, authored-layers), WDEF-013 (added, world-definition).
- Code: `world-def` (`stamp.rs`, `item.rs`, `lib.rs`), `generators` (`stamp.rs`: the brush kind adds nothing yet), `frame` (`preview.rs`: brushes skipped), `editor` (`commands.rs`: the new field defaults to none).
- Goldens: none moves.
- numerics-reviewer: not needed (no numeric output); spec-guardian before archive.
