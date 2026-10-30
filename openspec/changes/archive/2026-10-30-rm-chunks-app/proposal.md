## Why
B6 PR 2 of `remove-chunk-stage` (owner, 2026-10-29; ADR 0086): the app has no chunk stage any more. The app is the only caller of the stage, so it goes before the frame, streaming and generators crates (planner's order of 2026-10-30).

## What changes
- Removed: `anetworld.rs` (the stage, `WorldBase`, the chunk disk), `regionheights.rs`, `editresolve.rs` with `generate --edit-resolve`, `--stale-workers`, `--brush`, `--base-threads`; the stage in `WorldTerrain` (`enable_chunks*`, `chunk_manager`, `chunk_view`, `chunk_frame*`, the field layer); the scene `anet-chunk` with `xtask perf anet-chunk` and its log; the chunk trace, the late-chunk log and the prefetch of the envelope scene; the steady trial's chunk-edge start; the horizon-off comparison (it compared against the stage-on run); the chunk table of the edit-latency scene; the stills' waiting for chunks, `snap_to_channel_m` and `sections.txt`.
- Kept and made horizon-only: `chunkstage.rs` is now `horizonstep.rs` (`frame_step`, `FrameFeatures`, `FeatureCounters`, `announce_item_change`, `stage_gen`, `chunks_busy`); `horizon_workers`, `cells_per_chunk_for` and `enable_viewer_horizon` moved to `horizonworld.rs`. The envelope log keeps `chunk_stage` (always false) for `accept M8`.
- Reworded: TEST-048, TEST-050, TEST-052, TEST-054, TEST-055, TEST-057, TEST-058, VIEW-054. Removed: TEST-049, GEN-052, GEN-055. The changes `m8-edit-latency-chunks` (TEST-056), `m8-envelope-late-chunks` (TEST-059) and `m8-edits-reuse` (GEN-087) are archived without syncing.
- Tests: the stage cases of `anet_world.rs`, `oracle_edit_resolve.rs`, `horizon_world.rs`, `world_field.rs`, `gpu_horizon_rig.rs` are removed or rebuilt on the base terrain (the horizon lattice of a test is set with `set_horizon_cells`); the coarse-horizon, warm-up and hover cases moved to `coarse_horizon_world.rs` and `oracle_world_warmup.rs`.

## Moved to later PRs
The frame, render, streaming and generators sides (`chunkshare.rs`, the fade, `ChunkManager`, `anet.rs`, the chunk field, STRM-038 keys, GEN-081, GEN-082, STRM-056, STRM-063).

## Default path
No golden, no pixel and no hash changes: the chunk stage was off by default. The title of a window without horizon jobs no longer says "no chunk stage" (VIEW-054).
- STRM-042 reworded (the app makes no prefetch request); goes with the frame side.
