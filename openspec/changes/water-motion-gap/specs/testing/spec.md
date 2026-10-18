## MODIFIED Requirements

### Requirement: TEST-008 Repro bundles
Every visual bug SHALL be reproducible from a bundle containing camera pose or path, the definition hash, generator version, settings (the LOD parameters among them, with the finest level `max_level` and the render floor `min_level`) and adapter information, replayed by `cargo xtask repro <bundle>` (tile scenes and terrain scenes) in every run mode that renders (`test-render` and `editor --offscreen`); a terrain bundle of a run that named a world (`--world`) SHALL also record that world (the canonical definition text, the height field id and the bake size in cells per face edge; format version 2) and replay SHALL rebuild the field from the bundle alone, refusing a recorded world that does not hash to the recorded definition hash, a field id the recorded world no longer resolves to, and a radius that differs from the request's; a bundle without a world (every bundle of a seed-only run, and every version 1 bundle) is the reference field of its seed and still replays; replay SHALL refuse a version mismatch, SHALL refuse a definition hash this build cannot rebuild (without a recorded world the single planet of the recorded seed and radius is the only definition it can) and a missing (zero) hash, each with the recorded and rebuilt hashes, and SHALL report any difference between the rebuilt and the recorded plan in numbers. A bundle of a viewer frame (format version 3, written by the F9 key of VIEW-003) SHALL record, beside the request, the plan as it was drawn (the frame of the pipelined loop: its nodes, which may be resident ancestors standing in for tiles that were not resident, and the lit and water parts the viewer put on it) and how it was decided (`drawn exact|stale|cover`, the substituted and the blank node counts); its replay SHALL rebuild the world, regenerate the tiles of the drawn nodes, draw the recorded plan on the lit path, and report the drawn nodes beside the plan the request alone would make, because a frame plan is not the plan of its request. A bundle that is not a frame bundle is written as version 2 and replays as before.

Verify: A
Status: active
Source: CON-19, ADR 0006, M4 item 7d, LOD-011, ADR 0062

#### Scenario: Replay
- **WHEN** a bundle is replayed
- **THEN** the same FramePlan is produced and the run reports any difference in numbers

#### Scenario: Foreign definition
- **WHEN** a bundle records a definition hash this build cannot rebuild, or none
- **THEN** replay refuses it with the hashes and what to do, before replanning

#### Scenario: Terrain replay
- **WHEN** a terrain bundle recorded with a seed is replayed with another seed, another generator or mesh version, or a tampered plan
- **THEN** the replay refuses a version mismatch, rejects a corrupt bundle by its plan hash, and reports the differing plan lines (for example mesh keys) in numbers; an unchanged bundle replays to the recorded pixels

#### Scenario: World bundle
- **WHEN** a terrain run with `--world` writes a bundle and it is replayed with the world file gone
- **THEN** the replay rebuilds the field from the bundle, plans the recorded plan and draws the recorded pixels; an edited definition, a changed field id or another bake size is refused or reported in numbers

#### Scenario: Version 1 bundle
- **WHEN** a bundle of format version 1 is replayed
- **THEN** it reads as the seed-only reference field and replays as before

#### Scenario: Frame bundle
- **WHEN** the frame of a viewer with a stand-in node, a lit part and a water part is written as a bundle, read back, and replayed with the world rebuilt from it
- **THEN** the bundle reads back equal (plan hash, the stand-in nodes, the water nodes and the frame kind), a corrupted plan is refused by its hash, the replay reports the drawn nodes and the nodes of the request's own plan, and draws the recorded lit frame with its water

#### Scenario: Floor in the bundle
- **WHEN** a terrain bundle is recorded with floor 3 and replayed, and a bundle without `min_level` is replayed
- **THEN** the replayed plan equals the recorded one, and the bundle without `min_level` replays as floor 0 (the plan it recorded)

