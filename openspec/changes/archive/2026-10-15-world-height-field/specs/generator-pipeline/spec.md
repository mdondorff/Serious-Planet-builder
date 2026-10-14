## ADDED Requirements

### Requirement: GEN-018 The world names its height field
A world definition SHALL name its height field by implementation id (`world.height_field`). Resolution SHALL give the eroded field (`cpu-erosion-v1`, GEN-017) when the definition names none, the named field when the id is known (`cpu-erosion-v1`, `cpu-terrain-v1`, `cpu-ref-height-v2`), and otherwise an error that names the unknown id and lists every known id. The field built for a resolved id SHALL report that id as its `implementation_id`. `test-render --scene terrain` (a single view and `--flight`) SHALL build its field from the world's resolution (`--world`, an unknown id being refused before any GPU work); a run that names no world file is the seed-only world and SHALL name `cpu-ref-height-v2` explicitly, so no existing golden, plan snapshot or repro bundle changes. The `--stream`, `--bundle`, `--repro` and `--perf` paths remain seed-only reference paths and refuse `--world`; the interactive editor has no window yet.

Verify: A
Status: active
Source: owner's decision of 2026-10-13 (M4 item 0), ADR 0028, ADR 0030, CON-06, CON-07

#### Scenario: Default
- **WHEN** a definition names no height field
- **THEN** it resolves to `cpu-erosion-v1`

#### Scenario: Named
- **WHEN** a definition names `cpu-terrain-v1`
- **THEN** it resolves to `cpu-terrain-v1`, and the built field reports that id

#### Scenario: Unknown id
- **WHEN** a definition names an id no build knows
- **THEN** resolution fails with an error that contains the id and the list of known ids
