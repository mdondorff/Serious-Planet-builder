## Why
- Owner test 2026-11-03 (inbox #52): on `global16384-m9r2` the scripted descent's default site (20 N, 30 E) is open sea, so the descent ends in the water.
- Planner decision 2026-11-03 (inbox #53): a per-world site in the definition, and a default that snaps to land when none is named.

## What Changes
- `WorldDefinition::site()` / `set_site` and the TOML key `[world] site = [lat, lon]` (GEN-108): optional, validated (within 90 and 180 degrees), written canonically. Presentation: it enters no hash (`hash`, `base_hash` and every key are the same with and without it), so the store of the world opens unchanged.
- The viewer's `descent_site`: `--site` wins; else the definition's site; else 20 N 30 E where that is land; else the first land of `litscenes::find_coast` from there (the shore walk of the lit scenes, 1 km inland of the shore it finds); else the old constants. A pure function of the world.
- `crates/app/data/m9r2.world.toml`: seed 1 with the hills site 10.0946 N, 26.2573 W (ground 1,353 m, 948 m of relief within 3 km); it hashes as the default seed-1 world.

## Impact
- Code: `world-def` (lib, toml_io), `app::window` (`descent_site`, the usage text), a data file. No GPU or pixel change; no golden moves (goldens use explicit poses).
- Requirement: GEN-108 added.
- On `global16384-m9r2` without `--world`, the snap puts the descent on a shore at 20 N 121.09 W (ground 6 m, beach); with `--world crates/app/data/m9r2.world.toml` it descends over the hills.
