## MODIFIED Requirements

### Requirement: GEN-007 Staged generation
Generation SHALL be staged global-coarse, regional (per tile with a halo), local (per point), and a stage SHALL read only outputs of earlier stages: a field graph SHALL be refused when a field reads a field of its own stage or a later one, or an unknown or duplicate field, or has a halo that is not a finite fraction in [0, 1] (regional fields only), with an error naming the fields and stages. The key of a field's output SHALL cover its stage, id, version, implementation id, halo, scope (tile or region), the recorded read set and the keys of the outputs it reads, so that bumping one field's version re-keys exactly the fields downstream of it, and a key built without one of its inputs' keys SHALL be a bug that panics rather than a stale tile.

Verify: A
Status: active
Source: CON-08, ADR 0006, ADR 0021 point 6

#### Scenario: Illegal read
- **WHEN** a stage requests a later stage's output
- **THEN** the request is rejected with the stage names

#### Scenario: Chained keys
- **WHEN** one field's version, implementation id or halo changes
- **THEN** that field's key and the keys of every field that reads it, directly or not, change, and the keys of all other fields do not
