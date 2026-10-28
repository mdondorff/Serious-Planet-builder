## Why
The owner's fallback of 2026-10-28 (PR #848) puts the chunk stage off; the horizon jobs lived inside it, so no map would bake and plateau shadows would be gone. M8 delivers shadows (criterion 6), so the maps are baked from the base terrain and the stamps.

## What Changes
1. `streaming::chunks::ChunkSource` (the chunk manager, or `BaseTerrain` with no stage) is what `HorizonJobs` asks about chunk readiness; with `BaseTerrain` every chunk is ready with no change, nothing is evicted by the source, and the jobs keep the wanted maps and the most recently wanted others up to `keep` (default 32).
2. The viewer enables the horizon jobs and the stored far-field horizon without a chunk stage (`enable_viewer_horizon`); the maps are of the same format, march, shader and slots. An edit changes the edit word of the chunks it reaches (neighbours within the march's halo included), which re-bakes them after the settle window.
3. The gate judges horizon bakes > 0 at every site (also with the stage off) and the first re-bake within 10 s of its scripted edit.

## Impact
- Spec: STRM-064, TEST-065 (new), TEST-061 (modified). Code: `streaming/chunks.rs`, `app/horizonworld.rs`, `app/anetworld.rs`, `app/window.rs`, `app/gate.rs`. Tests: `streaming/tests/horizon_jobs.rs`, `app/gate.rs`, `app/tests/gpu_window.rs`.
