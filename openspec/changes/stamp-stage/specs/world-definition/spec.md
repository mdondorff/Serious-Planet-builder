## ADDED Requirements

### Requirement: WDEF-009 Stamp items
An authored item SHALL carry a kind and its parameters; the stamp kinds are `hill`, `crater` and `plateau` (GEN-026), with `amount_m` (finite; for a hill, a crater's depth or a plateau's offset) and parameters reserved at 0. `amount_m` SHALL be at most the item's `max_delta_m` in absolute value, or the item SHALL be refused naming the item and the bound. The kind and the parameters SHALL be part of the item's hash (WDEF-007), so changing either is a change of the item; the hash of an item of kind none SHALL be what it was before kinds existed.

Verify: A
Status: planned
Source: ADR 0066 point 6, M6 plan row E2

#### Scenario: Refusal
- **WHEN** a stamp is put with an amount above its bound, or a non-finite parameter
- **THEN** the call is refused naming the item and the bound, and the definition is unchanged

#### Scenario: Hash
- **WHEN** the kind or a parameter of a stamp changes, and when an item of kind none is hashed
- **THEN** the item hash changes in the first case, and equals the pinned known answers of the footprint tests in the second
