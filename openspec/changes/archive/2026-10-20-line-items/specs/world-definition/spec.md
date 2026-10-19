## ADDED Requirements

### Requirement: WDEF-011 Line items
The stamp kinds SHALL include `ridge` and `channel` (ADR 0069): an open polyline of 2 to 64 points (`[[points]]` of the item file, AUTH-005), the first of which is the item's anchor, with the parameters `amount_m` (the crest height of a ridge, the bed depth of a channel; finite, at least 0, at most `max_delta_m`) and `width_m` (the half-width of the line; finite, at least 1). The item's `reach_m` SHALL be at least `width_m` plus the largest distance, on the body's sphere, of a point from the anchor, and stays within one nominal cell edge (WDEF-007), so the polyline and its width fit inside the reach. The stamp of a line item SHALL hold `[amount_m, width_m, 0]` and the points as unit directions, and its content hash SHALL be derived from the kind, the three numbers and, for a stamp with points, their count (`u32`) and bit patterns in order under `planet-builder stamp v1` (WDEF-010), so a change of a point, the width or the amount re-keys the tiles the line touches; a stamp without points hashes as before. An item file's `kind` and its two `[params]` keys SHALL map to the stamp and back, round-trip, and a line kind without either key SHALL be refused naming the key. A put of a line item SHALL be refused, naming the item and the reason, when a number is not finite or negative, the amount is above `max_delta_m`, the width is under 1 m, there are fewer than 2 or more than 64 points, two consecutive points coincide, a point lies farther from the anchor than `reach_m - width_m`, a point is not a unit direction, or the first point is not the anchor (to 1e-9 rad); the definition is then unchanged. The footprint of a tile (WDEF-007) SHALL return the line item with its points, shared and immutable. The `group` and `part` of an item file (a long line split into linked items by the command layer) SHALL not enter the stamp: the world definition sees independent items.

Verify: A
Status: active
Source: ADR 0069, M6 plan row E5, owner (2026-10-19), WDEF-010

#### Scenario: Refusal
- **WHEN** a ridge is put with a negative amount, an amount over `max_delta_m`, a width under 1 m, one point, 65 points, a repeated consecutive point, a point beyond `reach_m - width_m` of the anchor, or a non-finite number
- **THEN** the call is refused naming the item and the reason and the definition is unchanged

#### Scenario: Item file
- **WHEN** a ridge and a channel are written as files, read back and put
- **THEN** each stamp equals the one it was made from, the anchor is the first point, a missing `width_m` is refused naming the key, and the content hash is the stamp's

#### Scenario: Re-key
- **WHEN** a point, the width or the amount of a line changes, and when only its `group` or `part` changes
- **THEN** the footprint hash of a tile under the line changes in the first case and that of a tile out of reach does not, and the second case changes no hash
