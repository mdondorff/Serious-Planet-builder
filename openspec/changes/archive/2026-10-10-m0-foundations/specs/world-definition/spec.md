## ADDED Requirements

### Requirement: WDEF-001 Single source of truth
The WorldDefinition SHALL be the only source of world content; every runtime representation (tiles, meshes, textures, instances) SHALL be regenerable from it and from nothing else.

Verify: A
Status: planned
Source: CON-05

#### Scenario: Regenerate
- **WHEN** all cached tiles are deleted and regenerated from the definition
- **THEN** the regenerated tiles are bit-identical to the deleted ones

### Requirement: WDEF-002 Schema version and migrations
The WorldDefinition file format SHALL carry its own schema version, independent of the generator version, and files of every older schema version in the fixture set SHALL load through tested migrations.

Verify: A
Status: planned
Source: CON-10

#### Scenario: Old fixture
- **WHEN** a fixture written by an older schema version is loaded
- **THEN** it migrates and equals the current-version fixture of the same content

### Requirement: WDEF-003 Canonical hashing
Every definition node SHALL have a content hash over a canonical byte encoding that does not depend on field order, platform or insertion history.

Verify: A
Status: planned
Source: ADR 0006

#### Scenario: Reordered fields
- **WHEN** the same definition is built with its entries inserted in a different order
- **THEN** the node hashes are equal

### Requirement: WDEF-004 Read recording
Generators SHALL read definition inputs only through a context that records every read; the recorded read set SHALL be what is hashed into cache keys.

Verify: A
Status: planned
Source: CON-06, ADR 0006

#### Scenario: Unrecorded read
- **WHEN** a generator is given a definition whose unread node changes
- **THEN** its output and key are unchanged, and when a read node changes both change

### Requirement: WDEF-005 Incremental invalidation
Editing a definition node SHALL invalidate only the tiles whose recorded read set contains that node.

Verify: A
Status: planned
Source: report §5, ADR 0006

#### Scenario: Local edit
- **WHEN** a stamp in one region is edited
- **THEN** only tiles whose footprint intersects the stamp change key
