## MODIFIED Requirements

### Requirement: WDEF-002 Schema version and migrations
The WorldDefinition SHALL be stored as TOML with a top-level `schema_version`, independent of the generator version. Loading SHALL require the version, reject a newer one and a gap in the migration chain with an error that names what to do, apply the migrations from the file's version to the current one in order, and fail with the step's message when a migration fails. Files of every older schema version in the fixture set SHALL load through tested migrations and equal the current-layout definition of the same content. Values SHALL be non-negative integers (a `u64` beyond TOML's integer range as a decimal string) and finite floats; any other value, a negative integer, a key outside letters, digits, `_` and `-`, or a path in the reserved `derived` namespace SHALL be an error naming the path. Writing SHALL be canonical (version first, sorted tables, shortest round-trip floats) and reading it back SHALL give the same nodes bit for bit.

Verify: A
Status: active
Source: CON-10

#### Scenario: Old fixture
- **WHEN** a fixture written by an older schema version is loaded
- **THEN** it migrates and equals the current-version fixture of the same content (until a second schema version exists the engine is tested with a synthetic three-version chain)

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build, or has no migration path, or a value cannot be stored exactly
- **THEN** loading fails with an error that names the version or the path

### Requirement: WDEF-003 Canonical hashing
Every definition node SHALL have a content hash over a canonical byte encoding (path, value type and exact value bits) that does not depend on field order, platform or insertion history, and the definition hash SHALL be a function of the set of nodes only, taken from the parsed nodes of a TOML file and never from its bytes, so comments, key order, whitespace and table style never change it.

Verify: A
Status: active
Source: ADR 0006

#### Scenario: Reordered fields
- **WHEN** the same definition is built with its entries inserted in a different order
- **THEN** the node hashes and the definition hash are equal
