## MODIFIED Requirements

### Requirement: TEST-008 Repro bundles
Every visual bug SHALL be reproducible from a bundle containing camera pose or path, definition hash, generator version, settings and adapter information, replayed by `cargo xtask repro <bundle>` in every run mode.

Verify: A
Status: active
Source: CON-19

#### Scenario: Replay
- **WHEN** a bundle is replayed
- **THEN** the same FramePlan is produced and the run reports any difference in numbers

### Requirement: TEST-009 FramePlan snapshots
A FramePlan SHALL be serialisable to a stable text form that can be snapshot-tested, diffed between versions and stored in a repro bundle.

Verify: A
Status: active
Source: CON-03, ADR 0009

#### Scenario: Round trip
- **WHEN** a FramePlan is serialised and read back
- **THEN** it is equal to the original

### Requirement: TEST-010 Debug view per visual feature
Every visual feature SHALL ship with a flat-colour debug view and at least one tier-C test; debug views SHALL be independent of lighting and compare exactly.

Verify: C
Status: active
Source: CON-18

#### Scenario: Face view
- **WHEN** the face debug view of two generated tiles is rendered
- **THEN** each planned rectangle is exactly the colour of its face, with no background pixels (the whole-planet version arrives with the LOD change in M2)

#### Scenario: Tile views
- **WHEN** the face, tile-id and height views of two generated tiles are rendered
- **THEN** no clear-colour pixel appears inside a planned rectangle, the face view is exactly the face colour, and the images match the adapter's goldens (or are written as candidates)

