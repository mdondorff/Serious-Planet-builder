## Why
Owner test 2026-10-27 (`docs/milestones/M8/owner-test-2026-10-27/README.md`, group A): the editor stalled 37 times in about an hour (1 to 15 s, 34 in the phase "chunk frame, refusals and editor update"). Cause (probe `chunk_frame_cost_report`, one thread): `ChunkManager::update_with` asked `ChunkVersions::version(key)` for every newly wanted chunk on the main thread; with an item in a stamp layer under the chunk stage a version needs the chunk's read set (`stage.read_discs`), 0.2 to 0.85 s per chunk (3.5 s the first), where the footprint queries after it take 0.2 ms. Without items the version is 0 at once, so the edit-latency scene (7 chunks, cached after the first frame) never showed it.

## What Changes
1. `ChunkManager::update_with` asks for the versions of the chunks that are ready (their read set is cached) and for their near-edit flags only; the job of a new chunk makes its own version on its worker before it reads the disk cache (STRM-063).
2. The stalled phase of the window frame is three marks: chunk frame, plan refusals / sweep / repro / preview end, editor update.
3. `PLANET_STALL_MS` sets the stall limit of a run (default 1 s); `chunk_frame_cost_report` (ignored) times the chunk step over new chunks.

## Impact
- Spec: STRM-063 (new). Code: `streaming/chunks.rs`, `app/window.rs`, `app/stall.rs`, `app/globalperf.rs`. Tests: tier A `chunks.rs` (version asked off the caller's thread), tier B `oracle_edit_resolve.rs` (a chunk step over new chunks with an item takes under 100 ms on the calling thread; 1195 ms before).
