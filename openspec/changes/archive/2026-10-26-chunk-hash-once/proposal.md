## Why
The owner's editor stalls intermittently while looking around and flying, since one edit. Reading main's code (opus-planner, 2026-10-25): `HorizonJobs::update` calls `word(k)` for every wanted chunk every frame, `word` reads `ChunkManager::result_hash` of each of the up to nine chunks a horizon reads, and `result_hash` hashed the whole pyramid of the result (about 2 MB, byte by byte, FNV) while holding the chunk-state lock, on the main thread, with the horizon lock held too; the chunk workers landing their results wait for that lock. The cost grows with the chunks in view and shows after an edit (every map is re-baked).

## What Changes
- `ChunkResult::hash` is made once (`OnceLock`), by the chunk job on its worker before it takes the lock; `hash_runs` counts it (a test aid).
- `ChunkManager::result_hash` takes the result under the lock and hashes it outside it (O(1) after the first).
- `HorizonJobs::update` makes the state words of the wanted chunks before it takes the horizon lock.

## Impact
- Spec: STRM-048 (the horizon job's per-frame cost); tier A test `the_state_word_of_a_horizon_hashes_a_chunks_pyramid_once_however_many_frames_ask`.
- Code: `generators/chunkfield.rs`, `streaming/chunks.rs`. No result changes: the hash is the same value.
