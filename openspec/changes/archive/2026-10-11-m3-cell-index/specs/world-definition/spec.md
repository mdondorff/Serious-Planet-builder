## MODIFIED Requirements

### Requirement: WDEF-004 Read recording
Generators SHALL read definition inputs only through a context that records every read with the hash of the node read; the hash of the recorded read set SHALL be what enters cache keys in place of the seed, a body's id SHALL be part of that read set, and spatial queries record the hash of each cell read (WDEF-005), and a missing node SHALL be an error naming it. A field built directly from a seed (`Reference(seed)`, the single planet's seed) SHALL key exactly like one built through a context over a definition with that world seed for `BodyId(0)`; other bodies key by their derived seed and id, so their fields come from the generator.

Verify: A
Status: active
Source: CON-06, ADR 0006

#### Scenario: Unrecorded read
- **WHEN** a generator is given a definition whose unread node changes
- **THEN** its output and key are unchanged, and when a read node changes both change

#### Scenario: Bodies
- **WHEN** the same generator reads two bodies of one definition
- **THEN** their keys differ

### Requirement: WDEF-005 Incremental invalidation
Authored items SHALL be indexed by spatial cell, a `TileId` of the tangent-warp mapping at one level per body chosen so a cell is about 40 km wide (level 8 at Earth's radius, computed with `libm`), and a generator SHALL read items only through cell queries that record the hash of every cell read, an empty cell included, with the body's radius that fixes the cell level. A field that queries cells SHALL report the read set of each tile separately (`inputs_hash_for`), so that adding, moving, editing or removing an item changes the keys of exactly the tiles whose footprint contains its cell and no others, and editing a node no generator read changes no key. A tile touching more cells than a limit reads the whole body instead. The definition hash SHALL cover the items, and the base hash (ADR 0007) SHALL cover the nodes only.

Verify: A
Status: active
Source: report §5, ADR 0006, ADR 0007

#### Scenario: Local edit
- **WHEN** an item in one cell is added, moved, edited or removed
- **THEN** only tiles whose footprint contains the old or new cell change height and mesh key, and restoring the item restores every key exactly

#### Scenario: Empty cell
- **WHEN** a generator queried a cell and found nothing, and an item is then added there
- **THEN** its read set and so the keys of the tiles that queried it change; an item in another cell changes nothing

#### Scenario: Hashes
- **WHEN** items are inserted in any order, or all removed
- **THEN** the definition hash is equal for equal content and equals the node hash when there are no items; the base hash never changes with items
