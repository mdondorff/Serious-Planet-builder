## MODIFIED Requirements

### Requirement: GEN-010 Mesh keys and known answers
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, the body parameter hash with the radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever vertex output changes for the same inputs or the compact construction (GEN-019) changes; version 5 introduced the compact tile, which carries the mesh key of its tile, so a tile has one key in either format. Persisted height tiles (STRM-009) are not meshes and are unchanged by it; persistent mesh keys (kind mesh) change with the version, and repro bundles of an older mesh version are refused with the versions named (TEST-008). Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

Verify: A
Status: active
Source: ADR 0006, CON-14

#### Scenario: Key follows inputs
- **WHEN** the seed, tile, grid size or radius of a mesh changes
- **THEN** its key changes, and equal inputs give equal keys

#### Scenario: Known answers
- **WHEN** the committed meshes are generated
- **THEN** their content hashes equal the committed values, and the test fails if the mesh version differs from the one the values belong to
