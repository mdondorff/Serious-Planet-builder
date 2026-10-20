## Why
M6 plan row E6, second half: the height output of brush strokes (raise, lower, smooth, flatten; radius and strength in metres), after the item model of `brush-items` (ADR 0070, proposed). The editor's brush tool is the support lane's.

## What Changes
- **Stroke stage** (GEN-030): the stamp stage applies the strokes of a brush item at a point in file order; the distance to the stroke's polyline gives a weight, raise and lower add `±strength * weight`, flatten moves towards `target_m` and smooth towards the mean of the stage's input around the point, each by at most `strength * weight`. The stage reads its input at a ring of points around a sample inside a smooth stroke.
- **Bounds**: a brush item's tile range is the sum over its strokes of `±strength * weight(nearest distance)`; the key is the content hash of the item.
- **Not in this change**: the brush tool and the grouping commands (editor), the instant preview of strokes (a brush shows when its exact tiles arrive, like lines), the blob spelling of ADR 0070 point 4.

## Impact
- Spec: GEN-030 (added, generator-pipeline).
- Code: `generators` (`stamp.rs`: `apply_stamps_with`, `stroke_delta`, `delta_range` for the brush, the stage passes its base as the reader of ring points).
- Goldens: none moves (no brush exists in any golden scene).
- `STAMP_STAGE_VERSION` stays 3: a layer without a brush item has the output it had.
- numerics-reviewer (generators), spec-guardian before archive.
