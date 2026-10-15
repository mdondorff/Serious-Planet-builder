## Why

M4 item 7d. #110 made the world name its height field, but only `test-render --scene terrain` (a view and `--flight`) followed it. `--stream`, `--bundle`, `--repro` and `--perf` still built the seed-only reference field, so the terrain perf scene, the pinned per-view counts (TEST-011) and the streaming path never exercised the world a user gets (the eroded field). Every later M4 measurement (frame pipelining, indirect draw, residency, the perf scenes) would have measured the wrong terrain.

## What Changes

- app: `--stream`, `--bundle`, `--repro` and `--perf`/`--perf-smoke` of the terrain scene build their field through `WorldTerrain`. `--world FILE` and `--global-res N` apply to all of them (`--repro` reads the world from the bundle).
- The perf scene without `--world` runs the default world of `--seed`: the eroded field over an in-memory bake of `--global-res` cells per face edge (default 256, at most 1024). A view, stream or bundle without `--world` is still the seed-only reference field, so the goldens, plan snapshots and every existing bundle keep their field.
- Streaming: the mesh source owns the world and builds the field per load (cheap), so streamed meshes equal the synchronous ones for any field.
- Repro bundles: format version 2 adds an optional world section (`height_field`, `global_res`, `world_toml`: the canonical definition text). Replay rebuilds the field from the bundle alone and refuses a world whose hash, field id or radius differs from the recorded values. Version 1 bundles (no world) still read and mean the reference field.
- Perf log: `protocol.field` names what the session measured (field id and bake size). A session is not compared with a baseline that measured another field (or none was recorded): `terrain.last-valid.json` is not compared across this change, and the session becomes the new baseline.
- TEST-011: the ten standard views are pinned over the default world (eroded field, 256-cell bake) in `crates/app/tests/world_view_pins.rs`; the reference-field pins in `crates/frame/tests/terrain.rs` stay for the goldens' field.

## Declared change (TEST-011)

Nodes per standard view at cells 32, tau 6, 1440p, reference field (before; still pinned for the goldens) to default world, eroded field over a 256-cell bake (after): orbit-20000km 52 to 52, high-2000km 428 to 430, aerial-200km 630 to 634, aerial-20km 892 to 900, low-2km 1279 to 1299, ground-200m 1694 to 1719, ground-1m 2897 to 2936, north-pole-1km 1180 to 1128, cube-corner-1km 1606 to 1594, face-edge-1km 1574 to 1551. Triangles, resident tiles and upload bytes follow (2,304 per node, one tile per node).

Not in this change: the store as the perf scene's bake (a 16384 store would give other counts; the pins name their bake), any golden (they keep `cpu-ref-height-v2`), the perf measurement itself (the architect runs it).
