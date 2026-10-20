## Why
The first stamp of a world adds a stamp stage. A stage's key carries its own implementation id and chains the stage below, so the first stamp re-keyed every tile of the world, in the 128-bit persistent keys and in the 64-bit keys of the plan: an edit refreshed all resident tiles (3,039 of them in the smoke run of the edit-latency scene) instead of the ~100 its footprint touches, and the pre-filled coarse pack of the plain world stopped serving the world. A tile no item touches is not changed by the stage at all.

## What Changes
- **Stage contract** (GEN-027): `HeightField::passes_through(tile, map)`, default no. A stage whose footprint for the tile is empty is the identity on its source there: heights, climate and bounds are the source's bit for bit. `StampStage` says yes exactly when `footprint_items` with its halo is empty; the wrappers forward it.
- **Keys** (STRM-029, departs from ADR 0066 point 4 for such tiles; ADR 0068): a tile a stage passes through is keyed as its source, in both key domains: `cache::key::{height_key, mesh_key_inputs}` and `generators::{cache_key_field, mesh_key_field, MeshKeyer}` take `key_source(field, tile, map)`, the field below the stages that pass the tile through. A tile an item touches is keyed by the stage as before.
- Untouched tiles keep the plain world's keys under any number of layers; the first stamp re-keys only the tiles its footprint touches; the pre-filled coarse pack serves editing sessions; a stage-version bump re-keys the tiles the stage acts on.

## Impact
- Specs: GEN-027 (generator-pipeline), STRM-029 (streaming-cache), added; STRM-027 still holds for every tile a stage acts on.
- Code: `generators` (`HeightField::passes_through`, `key_source`, `MeshKeyer`, `StampStage`), `cache` (key), the wrappers.
- Goldens: none (no world has stamps); the plain world's keys are unchanged.
- numerics-reviewer: the identity claim (bit for bit) and the key paths.
