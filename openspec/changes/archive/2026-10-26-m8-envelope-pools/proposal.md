## Why
The envelope windows of #752 and #763 ran the chunk stage with 18 chunk workers and 18 horizon workers (`chunkstage::enable_chunks`, copied from capture stills, which waits for completeness); the viewer (window.rs, M8-7's measured setting) uses half the generation workers, at most `VIEW_CHUNK_WORKERS` = 4, on one pool shared by the chunk and the horizon jobs. The windows measured an oversubscribed machine (about 3 x 18 threads), so their floors and the horizon-halves-the-floor reading are void (marked superseded, #767). The window and the scenes must not be able to drift apart again.

## What Changes
- `anetworld::viewer_chunk_stage(gen)` is the pool and the `ChunkConfig` of the viewer's stage; `WorldTerrain::enable_viewer_chunks(spawner, clock, cfg, horizon, use_disk)` is its blocking setup (the stage from the stored channel, the horizon jobs on the same spawner, the far-field horizon request). The window's `request_anet` thread and the headless scenes call them.
- `chunkstage::enable_viewer_stage` is the scenes' entry (the envelope scene uses it, one fresh stage per flight, no disk cache); capture stills keep `enable_chunks` (all workers: they wait for completeness, no measurement).
- TEST-054 says the scene runs the stage exactly as the window does.

## Impact
- Spec: testing TEST-054 (one clause). Code: `app/src/anetworld.rs`, `window.rs`, `chunkstage.rs`, `envelopeperf.rs`. Test: `the_viewers_chunk_stage_takes_half_the_generation_workers_at_most_four_and_one_shared_pool`.
- The envelope window has to be run again; crusader's edit-latency scene should use `enable_viewer_stage` too.
