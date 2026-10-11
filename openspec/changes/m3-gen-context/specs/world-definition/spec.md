## MODIFIED Requirements

### Requirement: WDEF-003 Canonical hashing
Every definition node SHALL have a content hash over a canonical byte encoding (path, value type and exact value bits) that does not depend on field order, platform or insertion history, and the definition hash SHALL be a function of the set of nodes only.

Verify: A
Status: active
Source: ADR 0006

#### Scenario: Reordered fields
- **WHEN** the same definition is built with its entries inserted in a different order
- **THEN** the node hashes and the definition hash are equal

### Requirement: WDEF-004 Read recording
Generators SHALL read definition inputs only through a context that records every read with the hash of the node read; the hash of the recorded read set SHALL be what enters cache keys in place of the seed, a body's id SHALL be part of that read set, and a missing node SHALL be an error naming it. A field built directly from a seed SHALL key exactly like one built through a context over a definition with that seed.

Verify: A
Status: active
Source: CON-06, ADR 0006

#### Scenario: Unrecorded read
- **WHEN** a generator is given a definition whose unread node changes
- **THEN** its output and key are unchanged, and when a read node changes both change

#### Scenario: Bodies
- **WHEN** the same generator reads two bodies of one definition
- **THEN** their keys differ

#### Scenario: Bundles
- **WHEN** a repro bundle records a definition hash this build cannot rebuild, or none
- **THEN** replay refuses it with the hashes and what to do
