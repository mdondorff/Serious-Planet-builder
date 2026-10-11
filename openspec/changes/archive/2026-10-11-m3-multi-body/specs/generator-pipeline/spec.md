## ADDED Requirements

### Requirement: GEN-012 Body identity in keys
Every height key and mesh key SHALL cover the identity and parameters of the body it belongs to (ADR 0001, 0007): the body id, the id of its reference surface and, for a mesh, the body parameter hash (id, seed, radius and reference surface). Two bodies SHALL never share a key for the same tile, even with equal seeds, inputs and radii, a field built directly from a seed SHALL stand for the single planet (`BodyId(0)`), and a key composition that gains an input SHALL bump `KEY_FORMAT_VERSION` and re-record the known answers, which the determinism manifest compares between operating systems.

Verify: A
Status: active
Source: ADR 0001, ADR 0006, ADR 0007, M3 queue item 4

#### Scenario: Body alone changes the key
- **WHEN** two fields differ only in the body they describe
- **THEN** their height keys and mesh keys differ, and the planet through the default equals the field built directly from the seed

#### Scenario: Body parameters
- **WHEN** the body id, seed, radius or reference surface changes
- **THEN** the body parameter hash, and with it the mesh key, changes
