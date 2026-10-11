## MODIFIED Requirements

### Requirement: GEN-007 Staged generation
Generation SHALL be staged global-coarse, regional (per tile with a halo), local (per point), and a stage SHALL read only outputs of earlier stages: a field graph SHALL be refused when a field declares a read of a field of its own stage or a later one, of an unknown field, or the same input twice, when a field is declared twice, or when a halo is not a finite fraction in [0, 1] (regional fields only; `-0.0` is `0.0`), with an error naming the fields and stages. The key of a field's output SHALL cover the stage key version, the field's stage, id, version, implementation id and halo, its scope (the whole body for the global bake, a tile for regional and local outputs, the kind part of the key, and a scope of the wrong kind for the stage a bug that panics), the body, reference surface, face mapping and resolution, the recorded read set and the keys of the outputs it reads, so that changing one field's version, implementation id or halo re-keys exactly the fields downstream of it, and a key built without one of its inputs' keys SHALL be a bug that panics rather than a stale tile. The key layout SHALL carry its own version and pinned known answers. The graph checks declared reads; a contract test that an implementation reads no more than it declares belongs to the first field that has one (ADR 0021 point 7).

Verify: A
Status: active
Source: CON-08, ADR 0006, ADR 0021 point 6, ADR 0022

#### Scenario: Illegal read
- **WHEN** a graph declares that a stage reads a later (or its own) stage's output
- **THEN** it is rejected with the field and stage names

#### Scenario: Chained keys
- **WHEN** one field's version, implementation id or halo changes
- **THEN** that field's key and the keys of every field that reads it, directly or not, change, and the keys of all other fields do not

#### Scenario: Scope
- **WHEN** a global field is keyed per tile, or a regional or local field globally
- **THEN** it panics; the tile with raw id 0 never aliases the global scope
