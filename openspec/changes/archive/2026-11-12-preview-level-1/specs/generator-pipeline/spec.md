## ADDED Requirements
### Requirement: GEN-157 The H0 re-route over an edit equals a fresh route of the edited heights
The edited heights SHALL be the base heights with the stamps added at the cell centres (`apply_stamps`), cells outside every item's reach keeping the base bits; the re-route SHALL be `GlobalGrid::route` on them with the sea as outlets, and SHALL equal bit for bit a route of the same heights made without the preview. The preview SHALL not depend on the thread count.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #95), A3 5.1 and 11.1, GEN-139

#### Scenario: Equality
- **WHEN** a height edit is previewed on a small planet and its heights are routed directly
- **THEN** the receivers, areas and outlets are equal, and cells outside the item's reach have the base's height bits
