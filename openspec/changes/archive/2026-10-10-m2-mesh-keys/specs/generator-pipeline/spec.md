## ADDED Requirements

### Requirement: GEN-010 Mesh keys and known answers
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever vertex output changes for the same inputs. Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

Verify: A
Status: active
Source: ADR 0006, CON-14

#### Scenario: Key follows inputs
- **WHEN** the seed, tile, grid size or radius of a mesh changes
- **THEN** its key changes, and equal inputs give equal keys

#### Scenario: Known answers
- **WHEN** the committed meshes are generated
- **THEN** their content hashes equal the committed values, and the test fails if the mesh version differs from the one the values belong to
