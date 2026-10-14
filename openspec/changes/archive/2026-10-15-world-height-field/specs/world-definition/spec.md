## MODIFIED Requirements

### Requirement: WDEF-002 Schema version and migrations
The WorldDefinition SHALL be stored as TOML with a top-level `schema_version`, independent of the generator version. Loading SHALL require the version, reject a newer one and a gap in the migration chain with an error that names what to do, apply the migrations from the file's version to the current one in order, and fail with the step's message when a migration fails. Files of every older schema version in the fixture set SHALL load through tested migrations and equal the current-layout definition of the same content. Values SHALL be non-negative integers (a `u64` above `i64::MAX`, and only that, as a canonical decimal string) and finite floats, typed by their literal except for the paths the program reads (`world.seed` an integer, `body.<n>.radius_m` a float, where an exact integer literal is accepted for the float and a float for the integer is an error); the one string value is `world.height_field`, the implementation id of the height field (letters, digits, `_` and `-`), which every other path rejects as a string and which is rejected as a number; any other value, a negative integer, a key outside letters, digits, `_` and `-`, or a reserved name (`derived`, `schema_version`) SHALL be an error naming the path. Writing SHALL be canonical (version first, sorted tables, shortest round-trip floats) and reading it back SHALL give the same nodes and height-field id bit for bit.

Verify: A
Status: active
Source: CON-10

#### Scenario: Old fixture
- **WHEN** a fixture written by an older schema version is loaded
- **THEN** it migrates and equals the current-version fixture of the same content (until a second schema version exists the engine is tested with a synthetic three-version chain)

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build, or has no migration path, or a value cannot be stored exactly
- **THEN** loading fails with an error that names the version or the path

#### Scenario: Height field id
- **WHEN** a file sets `[world] height_field = "cpu-terrain-v1"`, and when it sets the same path to a number or any other path to a string
- **THEN** the first loads, equals the definition built in code and round-trips; the others fail naming the path

### Requirement: WDEF-003 Canonical hashing
Every definition node SHALL have a content hash over a canonical byte encoding (path, value type and exact value bits) that does not depend on field order, platform or insertion history, and the definition hash SHALL be a function of the set of nodes only, taken from the parsed nodes of a TOML file and never from its bytes, so comments, key order, whitespace and table style never change it. The height-field id of the definition (`world.height_field`, GEN-018), when set, SHALL enter the base hash as one more entry under that path, and a definition that names none SHALL hash exactly as it did before the id existed.

Verify: A
Status: active
Source: ADR 0006

#### Scenario: Formatting
- **WHEN** the same definition is written with other comments, key order, spacing, table style or numeric spelling of an exact value (`6.371e6`, `6371000`)
- **THEN** it loads to the same nodes and the same hash

#### Scenario: Reordered fields
- **WHEN** the same definition is built with its entries inserted in a different order
- **THEN** the node hashes and the definition hash are equal

#### Scenario: Height field in the hash
- **WHEN** a definition names a height field, another names a different one, and a third names none
- **THEN** the three hashes differ, and the third equals the hash of the same nodes without the id
