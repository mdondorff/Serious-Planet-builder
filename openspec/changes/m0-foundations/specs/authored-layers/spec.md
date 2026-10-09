## ADDED Requirements

### Requirement: AUTH-001 Separate, geo-anchored storage
Authored content SHALL be stored separately from the procedural base, anchored in geo-coordinates (latitude, longitude, height, heading), as text files that merge cleanly in git, with binary payloads in content-addressed blobs.

Verify: A
Status: planned
Source: CON-09, ADR 0007

#### Scenario: Anchor round trip
- **WHEN** an authored item is saved and loaded
- **THEN** its anchor position is unchanged within 1 mm

### Requirement: AUTH-002 Recorded versions
Every authored item SHALL record the `generator_version` and definition hash it was authored against, and loading an item whose versions do not match the active ones SHALL fail with a message naming both versions unless the item is pinned or frozen.

Verify: A
Status: planned
Source: CON-09, ADR 0007

#### Scenario: Version mismatch
- **WHEN** an unpinned item authored against version 3 is loaded under version 4
- **THEN** loading reports both versions and the options (pin, freeze or re-author)

### Requirement: AUTH-003 Version pinning and freeze/bake
A region SHALL be freezable (baked to authoritative tiles at all levels with a blending ring at the border), and a definition SHALL be pinnable to a generator version.

Verify: A
Status: planned
Source: CON-09, ADR 0007

#### Scenario: Frozen region
- **WHEN** the base generator version changes
- **THEN** tiles inside a frozen region are unchanged and tiles in the blending ring are continuous with both sides

### Requirement: AUTH-004 Commands and undo
Edits SHALL be commands over the WorldDefinition recorded in an append-only log; undo and redo SHALL restore the exact previous definition hash for at least 1,000 operations.

Verify: A
Status: planned
Source: report §6

#### Scenario: Undo all
- **WHEN** 1,000 random edits are applied and then undone
- **THEN** the definition hash equals the starting hash
