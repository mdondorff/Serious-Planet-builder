## Why
M5 item 3 (ADR 0042 point 6, ADR 0048 point 2): the lit path's presentation parameters (scene-unit scale, exposure range and speeds, histogram axis and band, tone curve) are code constants in `frame::look`. The owner's M5 rule is that look parameters are data with physically based Earth defaults, reviewed once at the gate. Items 4 (albedo, sun), 5 (atmosphere) and 7 (liquid appearance) add more of them and need a place that is outside the world definition, so that tuning the look never re-keys a cache.

## What Changes
- A schema-versioned TOML look file (`planet_frame::look::file`, GPU-free): `schema_version`, `kind = "look"`, and per body (`body.<id>`) the tables `units`, `exposure`, `histogram`, `tone_map`; every key optional (missing means the Earth default), unknown keys and out-of-range values refused naming the key and the value, canonical writer, a migration engine like the definition's with a version 1 fixture and a synthetic three-version chain (CON-10). Version 1 is the first layout, so the production chain is empty.
- The Earth defaults are exactly the old constants (126,600 lux, EV100 -2..16, speeds 3 and 1, histogram -10..20 with the band 40..90 %, ACES); `frame::look` keeps the constants only as the definition of those defaults.
- `LitPlan` carries `SceneUnits` and `HistogramParams`; `LitPlan::fixed_with` and `adaptive_with` build a plan from a body's look. The text form adds the look's tokens only when they differ from the default, so plan text, hashes, bundles and goldens are unchanged.
- The pre-exposure factor takes the unit from the plan; the histogram and adaptation shaders take the axis and the band from the parameter block (two more `vec4`), with identical numbers at the default.
- The application loads it: `--look F` for `editor` and `test-render`, else `<stem>.look.toml` beside `--world`, else the defaults; `LookSet::params(body)` answers per body.
- New requirements LOOK-001 to LOOK-006; REND-021, REND-022 and REND-024 are modified.
- No new dependency (`toml` is already in the workspace; `frame` now uses it and `planet-world-def` for `BodyId`).

## Out of scope
Albedo tables and the sun (item 4), atmosphere coefficients (item 5), liquid appearance (item 7): their tables are added to the same file when those items land (optional tables, no migration). The viewer still shows the debug path, so the loaded look is validated and reported but not yet drawn; `LookSet::params` is the call the lit viewer will make. The editor UI for the look (M6). The placeholder `ALBEDO_SCALE` and `DEFAULT_SUN` stay code constants until item 4 replaces them. The EV conventions `1.2` and `8` (saturation-based exposure) are formula constants, not look data.
