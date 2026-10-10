## MODIFIED Requirements

### Requirement: TEST-008 Repro bundles
Every visual bug SHALL be reproducible from a bundle containing camera pose or path, definition hash (zero until the WorldDefinition exists in M3), generator version, settings and adapter information, replayed by `cargo xtask repro <bundle>` (tile scenes and terrain scenes) in every run mode that renders (`test-render` and `editor --offscreen`); replay SHALL refuse a version mismatch and report any difference between the rebuilt and the recorded plan in numbers.

Verify: A
Status: active
Source: CON-19

#### Scenario: Replay
- **WHEN** a bundle is replayed
- **THEN** the same FramePlan is produced and the run reports any difference in numbers

#### Scenario: Terrain replay
- **WHEN** a terrain bundle recorded with a seed is replayed with another seed, another generator or mesh version, or a tampered plan
- **THEN** the replay refuses a version mismatch, rejects a corrupt bundle by its plan hash, and reports the differing plan lines (for example mesh keys) in numbers; an unchanged bundle replays to the recorded pixels


### Requirement: TEST-009 FramePlan snapshots
A FramePlan and a TerrainPlan SHALL each be serialisable to a stable text form that can be snapshot-tested, diffed between versions and stored in a repro bundle; the terrain text form SHALL carry every field of the plan (size, view, cells, the f64 camera, view-projection, clear colour, height range and per node tile, level, origin, camera-relative origin, morph values, colour and mesh key) so that a change to any one of them changes the text and its hash.

Verify: A
Status: active
Source: CON-03, ADR 0009

#### Scenario: Round trip
- **WHEN** a FramePlan is serialised and read back
- **THEN** it is equal to the original

#### Scenario: Every terrain plan field
- **WHEN** any single field of a terrain plan is changed (down to one ulp of an f64 or f32 value)
- **THEN** its text form and plan hash change, and the diff names the difference

