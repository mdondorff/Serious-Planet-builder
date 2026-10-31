## Why
B6 PR 4 of `remove-chunk-stage` (owner, 2026-10-29; ADR 0086): after PRs 2 and 3 nothing in the app, the frame or the renderer calls the chunk manager, so the manager and the A-net chunk pack go.

## What changes
- Removed in `streaming`: the manager half of `chunks.rs` (`ChunkManager`, `ChunkConfig`, `ChunkStats`, `ChunkUpdate`, `ChunkDisk`, `ChunkVersions`, the stale hold, prefetch, `VIEW_CHUNK_WORKERS`, `VIEW_BASE_THREADS`). The horizon half moved unchanged to `horizonjobs.rs` (`HorizonJobs`, `HorizonConfig`, `HorizonBaker`, `HorizonDisk`, `ChunkSource`, `BaseTerrain`); the module path is `planet_streaming::horizonjobs`.
- Removed in `cache`: `chunk_store.rs` (`ChunkSpec`, `PackChunkDisk`).
- Requirements removed: STRM-038, STRM-039, STRM-041, STRM-042, STRM-056, GEN-081, GEN-082. Reworded: STRM-046 (no chunk manager), STRM-048 (the source, not the manager), STRM-057 (source), STRM-060 (no chunk pool), GEN-039 (no read set or version).
- Tests removed with the code: `streaming/tests/chunks.rs`, `cache/tests/chunk_keys.rs`. `streaming/tests/horizon_jobs.rs` keeps its cases on a fake `ChunkSource` in place of the manager; its one case on the pyramid being hashed once (a property of the manager's job) goes. `m8-editor-stalls` (STRM-063, chunk-only) is archived without syncing.

## Kept (not chunk-manager work)
- WDEF-018 and `CHAIN_ORDER_ERA`: the definition hash must not change (decision 3); PR 5 with the setting.
- `ReadRecord::Chunk`, `QUERY_CHUNK`, `Kind::Chunk`, `PARAM_CHUNK` and the `Scope::Chunk` stage key: produced by the generators' chunk field and `ChunkId` (GEN-034); PR 5.
- Citations of GEN-081 and STRM-041 in generators tests are re-cited to GEN-078 and GEN-037; those tests go with their modules in PR 5.

## Default path
No tile key, golden or hash changes: no default node reads a chunk, and the horizon code is moved, not edited.
