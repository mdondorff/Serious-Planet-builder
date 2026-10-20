## Context
- ADR 0070 (proposed): strokes inline, capped. ADR 0066 point 6: item files; ADR 0069 and the line items (`line-items`): `[[points]]`, `StampKind::{Ridge, Channel, Region}`, the stamp carries its points.
- Owner (ADR 0066 point 6): never one file per stroke; ADR 0070 departs from "blobs" and goes to the owner. This change is storage-agnostic below the item loader: a blob spelling would change `item.rs` only.

## Decisions
1. **Data.** `Stroke { kind: BrushKind, radius_m, strength_m, target_m: Option<f64>, points: Vec<(lat, lon)> }`; `BrushKind` is `Raise`, `Lower`, `Smooth`, `Flatten` (codes 0 to 3). The item's `strokes: Vec<Stroke>` is in file order = application order.
2. **Anchor and points.** The anchor is the first point of the first stroke. An item has `strokes` or `points`, never both. `group` and `part` are allowed with strokes (the editor's grouping by area).
3. **Caps.** 1 to 64 strokes, 1 to 64 points per stroke (a one-point stroke is a dab), 4,096 points per item. Two consecutive points of a stroke are distinct. A cap breach is refused naming the cap.
4. **Numbers.** `radius_m` finite, at least 1 and at most one cell edge; `strength_m` finite, above 0 and at most `max_delta_m`; `target_m` finite, required on a flatten and refused elsewhere. `max_delta_m` must be at least the sum of the strengths (the stage's bound) and at most 100,000 m.
5. **Reach.** `reach_m` is at least the largest radius plus the largest distance of a point from the anchor on the sphere, and at most one cell edge.
6. **Stamp.** `StampKind::Brush` (code 7) with parameters `[0, 0, 0]`; the stamp holds `Arc<[StrokeBits]>`; the content hash under `planet-builder stamp v1` extends the line form: after the point count and points (zero for a brush) comes the stroke count `u32` and per stroke the kind code `u8`, radius, strength and target bits (`u64` each, the target 0 bits when absent) and the point count `u32` and the point direction bits. A stamp without strokes hashes as before.
7. **Item hash (AUTH-005).** Schema 3 appends, after the schema-2 tail (group, part, mask layer, point count and points, all with a zero point count for a brush), the stroke count `u32` and per stroke the kind code `u8`, radius, strength and target as `u64` bit patterns (target bits 0 when absent) and the point count `u32` with each point's latitude and longitude bits. Names and notes stay out.
8. **File form.** `[[strokes]]` with the keys `kind`, `radius_m`, `strength_m`, `target_m` (flatten only) in that order, then its `[[strokes.points]]` tables (`latitude`, `longitude`). The writer is hand-written like the points writer.
9. **Not here.** Grouping commands (editor), the preview, the blob spelling.

## Risks
- The file of a full item is up to about 350 KB; the loader reads it in one pass. Merge conflicts on one item are a TOML hunk (ADR 0070).
- `max_delta_m ≥ Σ strength` makes an item with many strokes declare a large bound; the stage's bounds use the per-tile range, not the declared maximum (GEN-026), so tile planning is not affected.
