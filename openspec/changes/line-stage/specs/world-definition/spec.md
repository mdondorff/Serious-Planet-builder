## ADDED Requirements

### Requirement: WDEF-011 Line segment items
The stamp kinds SHALL include `ridge` and `channel` (ADR 0069): one straight great-circle segment centred on the item's anchor along the anchor's heading, with the parameters `amount_m` (the crest height of a ridge, the bed depth of a channel; finite, at least 0, at most `max_delta_m`) and `half_length_m` (finite, at least 0). The half-width of the segment is `reach_m - half_length_m` and SHALL be at least 1 m, and `reach_m` stays within one nominal cell edge (WDEF-007), so a segment and its width fit inside it. The stamp of a line item SHALL hold `[amount_m, half_length_m, heading_degrees]` (the heading is the anchor's, read from the file's `[anchor] heading`), and its content hash SHALL be derived from the kind and the three numbers under `planet-builder stamp v1` (WDEF-010), so a change of the heading, the length or the amount re-keys the tiles the segment touches. An item file's `kind` and its two `[params]` keys SHALL map to the stamp and back, round-trip, and a line kind without either key SHALL be refused naming the key. A put of a line item SHALL be refused, naming the item and the reason, when a number is not finite or negative, the amount is above `max_delta_m`, or the half-width is under 1 m; the definition is then unchanged.

Verify: A
Status: planned
Source: ADR 0069, M6 plan row E5, WDEF-010

#### Scenario: Refusal
- **WHEN** a ridge is put with `half_length_m` equal to `reach_m`, a negative amount, an amount over `max_delta_m` or a non-finite number
- **THEN** the call is refused naming the item and the reason and the definition is unchanged

#### Scenario: Item file
- **WHEN** a ridge and a channel are written as files, read back and put
- **THEN** each stamp equals the one it was made from, the heading is the file's anchor heading, a missing `half_length_m` is refused naming the key, and the content hash is the stamp's

#### Scenario: Re-key
- **WHEN** the heading, the half-length or the amount of a segment changes
- **THEN** the footprint hash of a tile under the segment changes and that of a tile out of reach does not
