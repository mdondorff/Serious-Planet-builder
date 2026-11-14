## ADDED Requirements

### Requirement: BUILD-051 The determinism manifest covers the region bake at several thread counts
`generate --manifest` SHALL write, after the existing lines, the lines `regionbake t1`, `regionbake t1again` and `regionbake t8`: a 64-bit hash of every file of the region bake fixture (`planet_cache::region_bake::determinism_fixture`), baked at one thread twice and at eight threads. The three hashes of one run SHALL be equal. The CI determinism job's manifest diff SHALL compare them across Windows and Linux.

Verify: A
Status: active
Source: Architect 2026-11-11 (ADR 0114; external review condition 3), owner 2026-11-12 (approved lists get a queue row), GEN-003, GEN-177

#### Scenario: A thread count or a repeat that changes a byte
- **WHEN** the fixture is baked at one thread twice and at eight threads
- **THEN** the three hashes are equal; a bake whose bytes differ at another thread count, in a repeat, or on the other operating system fails the tier A test or the manifest diff naming the line
