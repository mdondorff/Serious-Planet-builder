## Why

ADR 0009 review follow-up 2 (docs/status.md on change/adr-0009-review, owner asked): `TerrainPlan` had only `summary()`, which omitted colour, mesh key, height range, clear colour, cells, size, view and the f64 camera; and `render_terrain` ignored `--bundle`, so terrain bugs could not be replayed.

## What Changes

- `frame`: `TerrainPlan::to_text/from_text/plan_hash/diff` (every field, floats as hex bit patterns); `TerrainBundle` (request incl. LOD parameters, versions, recorded plan, plan hash).
- Committed snapshots: `terrain_plans.txt` (hash of the full text of all ten scripted plans) and `terrain_plan_orbit.txt` (one plan in full, reviewable line by line).
- `app`: `test-render --scene terrain --bundle <path>` writes a terrain bundle; `--repro` replays terrain bundles (also in `editor --offscreen`): refuse on generator, implementation or mesh version mismatch, replan from the recorded request, report any differing plan line, then render with the recorded seed.
- Modified: TEST-008 (terrain bundles), TEST-009 (terrain plans).
