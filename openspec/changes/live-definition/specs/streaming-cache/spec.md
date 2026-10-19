## ADDED Requirements

### Requirement: STRM-028 The terrain reads the current definition
A world's terrain SHALL hold its definition as a replaceable shared value and SHALL build every field from a snapshot of it taken when the field is built; a field built before a replacement SHALL keep the definition it was built from (a plan or tile job in flight finishes on the definition it started with, and its keys name it through the item revision and the footprint entries). A replacement SHALL be refused, naming both hashes, when the new definition's base hash differs from the current one's, because the bake the fields read was made from the base. The field of a definition that holds stamps SHALL be the base field with one stamp stage (GEN-026) per layer that holds a stamp, lowest layer first (the layer stack of ADR 0007, then other layer names by name), each at its index in that order, below the water bound; the field of a definition without stamps SHALL be what it was. The plan worker's residency set SHALL be selected again when the item revision of the field it plans over changes, whatever the camera did. A caller that edits items SHALL replace the definition first and then name the edited tiles to the managed loop.

Verify: A
Status: active
Source: ADR 0066 points 4 and 5, M6 plan (live definition)

#### Scenario: Swap
- **WHEN** the definition of a shared terrain is replaced by one with a stamp, then by one with a stamp in a second layer, then by one without items
- **THEN** each next field has the stamps of its definition, a field built before the replacement keeps its heights, and the last field equals the first

#### Scenario: Another base
- **WHEN** a definition with another radius is offered
- **THEN** the call is refused and the definition and its revision are unchanged
