## MODIFIED Requirements

### Requirement: GEN-018 The world names its height field
A world definition SHALL name its height field by implementation id (`world.height_field`). Resolution SHALL give the eroded field (`cpu-erosion-v1`, GEN-017) when the definition names none, the named field when the id is known (`cpu-erosion-v1`, `cpu-terrain-v1`, `cpu-ref-height-v2`), and otherwise an error that names the unknown id and lists every known id. The field built for a resolved id SHALL report that id as its `implementation_id`. Every path of `test-render --scene terrain` (a single view, `--flight`, `--stream`, `--bundle`, `--perf` and `--perf-smoke`) SHALL build its field from the world's resolution (`--world`, an unknown id being refused before any GPU work) over an in-memory bake of `--global-res` cells per face edge (default 256, at most 1024); `--repro` SHALL rebuild the field from the world the bundle records (TEST-008) and refuse `--world`. A view, stream, flight or bundle that names no world file is the seed-only world and SHALL name `cpu-ref-height-v2` explicitly, so no existing golden, plan snapshot or repro bundle changes; the perf scene that names no world file SHALL run the default world of `--seed` (the eroded field), because it measures the world a user gets, and SHALL report which field and bake it measured. A streamed mesh SHALL equal the synchronously generated mesh of the same field. The interactive editor has no window yet.

Verify: A
Status: active
Source: owner's decision of 2026-10-13 (M4 item 0) and of 2026-10-15 (M4 item 7d), ADR 0028, ADR 0030, CON-06, CON-07

#### Scenario: Default
- **WHEN** a definition names no height field
- **THEN** it resolves to `cpu-erosion-v1`

#### Scenario: Named
- **WHEN** a definition names `cpu-terrain-v1`
- **THEN** it resolves to `cpu-terrain-v1`, and the built field reports that id

#### Scenario: Unknown id
- **WHEN** a definition names an id no build knows
- **THEN** resolution fails with an error that contains the id and the list of known ids

#### Scenario: Every terrain path takes the world
- **WHEN** `--stream`, `--bundle`, `--perf` or `--perf-smoke` is given with `--world`, and when the perf scene is given none
- **THEN** the options are accepted, the field is the world's (the perf scene without a world: the eroded field over the `--global-res` bake), and a view with `--global-res` and no world is refused

#### Scenario: Streamed equals synchronous
- **WHEN** a terrain view with a world is rendered with and without `--stream`
- **THEN** the two frames are identical
