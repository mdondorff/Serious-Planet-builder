## ADDED Requirements

### Requirement: WDEF-010 Stamp items
An authored item SHALL carry a kind and its parameters; the kinds are none, `hill`, `crater` and `plateau` (GEN-026), with `height_m` for a hill (signed), `depth_m` for a crater (at least 0) and `target_m` for a plateau (an absolute height) in an item file, one parameter in the stamp and two reserved at 0. A stamp SHALL be refused, naming the item and the reason, when `reach_m` is not above 0, a parameter is not finite, the hill's height or the crater's depth is above its `max_delta_m` in absolute value, or a crater's depth is negative. The content hash of a stamp SHALL be derived from its kind and parameters (BLAKE3-128 under `planet-builder stamp v1`; a given hash that disagrees is refused), so the kind and the parameters are part of every hash that covers the content: the item hash (WDEF-007), the footprint result, the region summary leaves (WDEF-008) and the definition hash; no hash format changes, and an item with no stamp hashes as it did before kinds existed. The footprint query SHALL return the stamp with each item, in the cap form (`footprint_items_cap`) as well as the tile form. An item file's `kind` and that one parameter SHALL map to the stamp (the stamp's reach is the file's `reach_m`, its bound the file's `max_delta_m`), the map SHALL round-trip, a stamp kind without its parameter SHALL be refused by the file's validation naming the key, other keys SHALL be ignored (they are in no hash), and for a stamp kind the item's content hash SHALL be the stamp's (the name, the notes and the recorded versions re-key nothing); a kind that is none of the three is a plain item that keeps the file hash.

Verify: A
Status: active
Source: ADR 0066 point 6, ADR 0067, M6 plan row E2

#### Scenario: Refusal
- **WHEN** a stamp is put with no reach, an amount above its bound, a negative crater depth or a non-finite parameter
- **THEN** the call is refused naming the item and the reason, and the definition is unchanged

#### Scenario: Hash
- **WHEN** the kind or a parameter of a stamp changes, and when an item of kind none is hashed
- **THEN** the content hash, the item hash, the footprint result, the summary leaf and the definition hash change in the first case, and the pinned known answers of the footprint and summary tests are unchanged in the second

#### Scenario: One source of truth
- **WHEN** a stamp is put with a content hash that is not the one its kind and parameters give
- **THEN** it is refused

#### Scenario: Item file
- **WHEN** a hill, a crater and a plateau item are written as files with their parameter, read back and put into a definition
- **THEN** each stamp equals the one it was made from, its content hash is the stamp's and stable, a stamp kind without its parameter is refused naming the key, and a kind that is none of the three keeps the file hash
