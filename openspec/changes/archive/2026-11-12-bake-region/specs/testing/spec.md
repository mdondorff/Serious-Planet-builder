## ADDED Requirements
### Requirement: TEST-125 The region bake test
A tier-A test SHALL bake a fixture region and check that the commit names a graph, the reader gives the channels of the fixture's known H1 segments at their local cells, the L tiles of the land tiles are present, two bakes give the same bytes and one and eight threads give the same bytes, and a region near a face edge is refused.

Verify: A
Status: planned
Source: GEN-177

#### Scenario: Determinism
- **WHEN** the fixture region is baked twice and at one and eight threads
- **THEN** every stored record is byte-equal
