## MODIFIED Requirements

### Requirement: TEST-008 Repro bundles
Every visual bug SHALL be reproducible from a bundle containing camera pose or path, the definition hash, generator version, settings and adapter information, replayed by `cargo xtask repro <bundle>` (tile scenes and terrain scenes) in every run mode that renders (`test-render` and `editor --offscreen`); replay SHALL refuse a version mismatch, SHALL refuse a definition hash this build cannot rebuild (the single planet of the recorded seed and radius is the only definition it can) and a missing (zero) hash, each with the recorded and rebuilt hashes, and SHALL report any difference between the rebuilt and the recorded plan in numbers.

Verify: A
Status: active
Source: CON-19, ADR 0006

#### Scenario: Replay
- **WHEN** a bundle is replayed
- **THEN** the same FramePlan is produced and the run reports any difference in numbers

#### Scenario: Foreign definition
- **WHEN** a bundle records a definition hash this build cannot rebuild, or none
- **THEN** replay refuses it with the hashes and what to do, before replanning

#### Scenario: Terrain replay
- **WHEN** a terrain bundle recorded with a seed is replayed with another seed, another generator or mesh version, or a tampered plan
- **THEN** the replay refuses a version mismatch, rejects a corrupt bundle by its plan hash, and reports the differing plan lines (for example mesh keys) in numbers; an unchanged bundle replays to the recorded pixels
