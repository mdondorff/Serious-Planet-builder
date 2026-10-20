## ADDED Requirements

### Requirement: WDEF-013 Brush items
The stamp kinds SHALL include `brush` (ADR 0070): an ordered list of 1 to 64 strokes (`[[strokes]]` of the item file, AUTH-005), each with a kind (`raise`, `lower`, `smooth` or `flatten`), `radius_m` (finite, at least 1, at most one nominal cell edge), `strength_m` (finite, above 0: the largest height change of the stroke at its centre line), for a flatten `target_m` (finite: the height it levels towards, metres above the reference surface) and 1 to 64 points of its own (4,096 in all), no two consecutive points of a stroke equal. The item has no `[params]` and no `[[points]]`; its anchor is the first point of the first stroke. The item's `reach_m` SHALL be at least the largest `radius_m` plus the largest distance, on the body's sphere, of a point from the anchor, and stay within one nominal cell edge (WDEF-007). The item's `max_delta_m` SHALL be at least the sum of the `strength_m` of its strokes and at most 100,000 m. The stamp of a brush item SHALL hold the parameters `[0, 0, 0]` and the strokes (kind code `u8`: 0 raise, 1 lower, 2 smooth, 3 flatten; radius, strength and target as bit patterns with `-0.0` made `0.0`, the target 0 bits when absent; the points as unit directions), shared and immutable, and its content hash SHALL be derived under `planet-builder stamp v1` (WDEF-010) from the kind, the three numbers, the point count (0) and then the stroke count (`u32`) and per stroke the kind code, the three numbers as bits, the point count (`u32`) and the point bit patterns, so a change of a point, a radius, a strength, a target, a kind or the order of the strokes re-keys the tiles the item touches; a stamp without strokes hashes as before. A put of a brush item SHALL be refused, naming the item, the stroke and the reason, when a number is not finite, a radius is under 1 m or above a cell edge, a strength is not above 0, a flatten has no target or another kind has one, there are no strokes, more than 64 strokes, a stroke without points or with more than 64, more than 4,096 points in all, two consecutive points of a stroke coincide, a point lies farther from the anchor than `reach_m` less the largest radius, a point is not a unit direction, the first point is not the anchor (to 1e-9 rad), or `max_delta_m` is under the sum of the strengths; the definition is then unchanged. The footprint of a tile (WDEF-007) SHALL return the brush item with its strokes, shared and immutable. An item file's `kind` `brush` and its strokes SHALL map to the stamp and back and round-trip.

Verify: A
Status: planned
Source: ADR 0070, M6 plan row E6, WDEF-010, WDEF-011

#### Scenario: Refusal
- **WHEN** a brush is put with a radius of 0.5 m, a strength of 0, a flatten without a target, a raise with a target, 65 strokes, a stroke of 65 points, 4,097 points in all, a repeated consecutive point, a point beyond `reach_m` less the largest radius, a `max_delta_m` under the sum of the strengths, or a non-finite number
- **THEN** the call is refused naming the item, the stroke and the reason and the definition is unchanged

#### Scenario: The stamp
- **WHEN** a brush item is put and its footprint is read, and each stroke field, a point and the order of two strokes change in turn
- **THEN** the footprint returns the item with its strokes, shared, the stamp's content hash equals the hash of the layout above computed independently in the test, every change gives another hash, and a ridge or a hill hashes as it did before

#### Scenario: Item file
- **WHEN** a brush item is converted to a file and back
- **THEN** the strokes round-trip bit for bit and the definition hash after putting it back is the one before
