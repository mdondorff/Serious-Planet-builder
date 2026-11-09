## ADDED Requirements
### Requirement: TEST-103 The numbers of every stage keep their invariants
The fixture's run SHALL satisfy, on every run of the manifest test: the landmass has land and sea and as many values as cells; the rounds do not raise the sum of the land heights; thermal and the pit spreading keep the height volume (relative change at most 1e-9); the deposition's eroded column equals deposited plus exported (relative error at most 1e-9); H1 has channel cells and segments and at least as many basin cells as basins.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #100), BUILD-047

#### Scenario: The invariants
- **WHEN** the fixture runs
- **THEN** each invariant holds, and a change that breaks one fails the test with the values

### Requirement: TEST-104 Two definitions of one quantity agree
The pit cells of a surface (the land cells whose fill depth reaches H1's pit limit, from the sediment pass's flood and outlets) SHALL equal the cells of H1's basins on the same surface, on the fixture's landmass, on what the rounds left and on what the sediment pass left, and at least one of these surfaces SHALL have pits, so the equality is not 0 = 0.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #100: pit cells = H1 basin cells), BUILD-047

#### Scenario: Pits and basins
- **WHEN** both are computed on the three surfaces
- **THEN** the counts are equal on each, and the landmass has pits
