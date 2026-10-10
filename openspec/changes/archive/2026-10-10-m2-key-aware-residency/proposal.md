## Why

The owner pointed out that the editor will switch seeds on a live renderer. Residency was decided by tile id alone (`has_tile`), so a caller could skip regenerating a mesh whose inputs had changed, and a stale buffer would be drawn silently.

## What Changes

- `frame`: every `TerrainDraw` carries `mesh_key`, the key of the mesh it expects.
- `render`: `TerrainRenderer::render` fails with `ExecError::StaleMesh` (tile, resident key, planned key) when a resident buffer's key differs; `has_tile` is replaced by `has_mesh(tile, key)`.
- `app`: the perf workload decides what to generate with `has_mesh`.
- Modified: REND-013 (scenario "Stale mesh").
