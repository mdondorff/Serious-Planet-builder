## Why

Owner's decision (a), 2026-10-13, M4 item 0: a world definition names its height field by implementation id. The eroded field (GEN-017, `cpu-erosion-v1`) is the default, `cpu-terrain-v1` stays selectable. Until now the field was chosen by the code that happened to build it (the app and `test-render` build the seed-only reference field `cpu-ref-height-v2` directly).

## What Changes

- world-def: a definition may carry `world.height_field`, an implementation id (a string, TOML `[world] height_field = "..."`). It is part of the base hash when set; an unset definition hashes exactly as before. The TOML reader and writer carry it (WDEF-002); the id is syntax-checked only (world-def cannot know the generators, CON-02).
- generators: `height_fields` registry: the known ids (`cpu-erosion-v1` default, `cpu-terrain-v1`, `cpu-ref-height-v2`), `resolve(def)` (default, or the named id, or an error listing the known ids), and `build_field` over a bake (GEN-018).
- frame: `flight_path_field` (the flight over any field).
- app: `test-render --scene terrain` (single view and `--flight`) accepts `--world FILE` and `--global-res N` and uses the world's field over an in-memory bake; an unknown id is refused before any GPU work. Without `--world` the invocation is the legacy seed-only world and keeps the reference field explicitly, so no golden, plan snapshot or repro bundle changes.

Not in this change: the `--stream`, `--bundle`/`--repro` and `--perf` terrain paths with a world field (they refuse `--world` with a message), and the interactive editor (no window yet).
